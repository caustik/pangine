use super::{
    choice::Choice,
    completion::projection_strength,
    concept_answer::{ConceptAnswer, LiveConceptAnswer},
    interpolation::Probability,
    CompletionGrade, CompletionResult, ConceptId, Pangine,
};
use crate::Relevance;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// An immutable proof-bearing answer snapshot.
///
/// The shape identifies the output Percepts that belong to the answer. The
/// completion result retains its correlated rows and source evidence. Derived
/// answers share neither live mutation nor implicit dependencies with later
/// engine state.
#[derive(Clone)]
pub struct Answer {
    pub(super) result: Rc<CompletionResult>,
    pub(super) shape: ConceptId,
    pub(super) origin: Option<ConceptId>,
}

impl Answer {
    /// Returns the complete correlated result retained by this snapshot.
    pub fn result(&self) -> &CompletionResult {
        &self.result
    }

    /// Returns the visible output shape retained by this snapshot.
    pub fn shape(&self) -> &ConceptId {
        &self.shape
    }

    /// Creates a view over one output projection contained by this answer.
    pub fn view(&self, pangine: &Pangine, projection: ConceptId) -> Option<AnswerView> {
        if !pangine.owns(&self.shape) || !pangine.owns(&projection) {
            return None;
        }

        let mut answer_outputs = BTreeSet::new();
        pangine.collect_output_percepts(&self.shape, &mut answer_outputs);

        let mut projected_outputs = BTreeSet::new();
        pangine.collect_output_percepts(&projection, &mut projected_outputs);
        if projected_outputs.is_empty() || !projected_outputs.is_subset(&answer_outputs) {
            return None;
        }

        Some(AnswerView { answer: self.clone(), projection })
    }

    /// Replaces the live answer Concept from which this snapshot was derived
    /// and returns the newly current snapshot.
    ///
    /// Publication fails when another operation has already changed or
    /// detached any output linked to that Concept.
    pub(super) fn publish(&self, pangine: &mut Pangine) -> Result<Answer, AnswerPublicationError> {
        pangine.publish_answer(self)
    }

    fn derived(&self, result: CompletionResult) -> Self {
        Self { result: Rc::new(result), shape: self.shape.clone(), origin: self.origin.clone() }
    }
}

/// One explicit projection of an immutable [`Answer`].
#[derive(Clone)]
pub struct AnswerView {
    pub(super) answer: Answer,
    pub(super) projection: ConceptId,
}

impl AnswerView {
    /// Returns the immutable answer behind this view.
    pub fn answer(&self) -> &Answer {
        &self.answer
    }

    /// Returns the Concept shape instantiated by this view.
    pub fn projection(&self) -> &ConceptId {
        &self.projection
    }

    /// Creates another projection over the same immutable answer.
    pub fn projecting(&self, pangine: &Pangine, projection: ConceptId) -> Option<Self> {
        self.answer.view(pangine, projection)
    }

    /// Materializes this projection as an ordinary Concept whose members carry
    /// their current counts, without changing the answer or any live Percept.
    pub fn materialize(&self, pangine: &mut Pangine) -> Option<ConceptId> {
        if !pangine.owns_answer(&self.answer) {
            return None;
        }
        pangine.materialize_completion_projection(&self.answer.result, &self.projection)
    }

    /// Returns every projected possibility, most probable first, with its
    /// evidence count, probability, and the support behind that count.
    ///
    /// A possibility's probability is its share of the positive evidence among
    /// these possibilities. A graded answer interpolates its complete rows from
    /// the exact grade toward the more general ones, and each possibility adds
    /// the rows that give it, so every view of one answer agrees. A possibility whose
    /// evidence is zero or negative stays in the list with probability zero,
    /// and equal probabilities keep the larger count first, then canonical
    /// spelling order. `is_top_tie` identifies every most probable possibility;
    /// it is false for every possibility when none has positive evidence and
    /// choice abstains.
    pub fn possibilities(&self, pangine: &mut Pangine) -> Option<Vec<AnswerPossibility>> {
        if !pangine.owns_answer(&self.answer) {
            return None;
        }

        let support = pangine.completion_projection_support(&self.answer.result, &self.projection)?;
        let probabilities = pangine.projection_probabilities(&self.answer.result, &self.projection)?;
        let mut complete_rows = BTreeMap::new();
        for completion in &self.answer.result.completions {
            let value = pangine.instantiate_completion(&self.projection, completion)?;
            *complete_rows.entry(value).or_insert(0) += 1;
        }

        let mut possibilities = support
            .into_iter()
            .map(|(value, derivations)| {
                let strength = projection_strength(&derivations)?;
                let probability = probabilities.get(&value).copied().unwrap_or(Probability::ZERO);
                let support = derivations
                    .into_iter()
                    .map(|((grade, _, sources), weight)| AnswerSupport {
                        grade,
                        weight,
                        sources: sources
                            .into_iter()
                            .map(|source| AnswerSource { subject: source.subject().clone(), concept: source.concept, relevance: source.relevance })
                            .collect(),
                    })
                    .collect();
                Some(AnswerPossibility {
                    complete_rows: complete_rows.remove(&value).unwrap_or_default(),
                    value,
                    strength,
                    support,
                    probability,
                    is_top_tie: false,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        possibilities.sort_by_cached_key(|possibility| {
            (Reverse(possibility.probability), Reverse(possibility.strength), pangine.format_concept(&possibility.value, false))
        });

        let greatest = possibilities.first().map(|possibility| possibility.probability).filter(|probability| !probability.is_zero());
        for possibility in &mut possibilities {
            possibility.is_top_tie = Some(possibility.probability) == greatest;
        }
        Some(possibilities)
    }

    /// Chooses this projection and returns the selected Concept together with
    /// a new answer containing only compatible complete rows.
    pub fn choose(&self, pangine: &mut Pangine) -> Option<AnswerChoice> {
        self.pick(pangine, Choice::MostProbable)
    }

    /// Draws one value of this projection with probability equal to its
    /// share, as `^~` does, and returns it together with a new answer
    /// containing only compatible complete rows.
    ///
    /// A graded answer draws by its interpolated probabilities. Each draw
    /// advances the engine's generator, which
    /// [`Pangine::set_sample_seed`] seeds.
    pub fn sample(&self, pangine: &mut Pangine) -> Option<AnswerChoice> {
        self.pick(pangine, Choice::Sampled)
    }

    fn pick(&self, pangine: &mut Pangine, choice: Choice) -> Option<AnswerChoice> {
        if !pangine.owns_answer(&self.answer) {
            return None;
        }

        let (selected, result) = pangine.choose_completion_result(&self.answer.result, &self.projection, choice)?;
        let answer = self.answer.derived(result);
        Some(AnswerChoice { selected, answer: answer.view(pangine, self.projection.clone())? })
    }

    /// Imports the evidence of matching rows from another answer view.
    ///
    /// The returned answer keeps this view's answer shape. Every imported
    /// derivation keeps its sources and is weighed by `factor`, which does not
    /// change any source's own count. An empty factor leaves the answer
    /// unchanged.
    pub fn adjusted_by(&self, pangine: &mut Pangine, adjustment: &AnswerView, factor: Relevance) -> Option<AnswerView> {
        if !pangine.owns_answer(&self.answer) || !pangine.owns_answer(&adjustment.answer) {
            return None;
        }

        let result = pangine.adjust_completion_result(&self.answer.result, &self.projection, &adjustment.answer.result, &adjustment.projection, factor)?;
        self.answer.derived(result).view(pangine, self.projection.clone())
    }
}

/// One projected value retained by an [`AnswerView`].
#[derive(Clone)]
pub struct AnswerPossibility {
    value: ConceptId,
    strength: Relevance,
    complete_rows: usize,
    support: Vec<AnswerSupport>,
    probability: Probability,
    is_top_tie: bool,
}

impl AnswerPossibility {
    /// Returns the projected value.
    pub fn value(&self) -> &ConceptId {
        &self.value
    }

    /// Returns the signed evidence count behind this value, the sum of its
    /// support weights.
    pub fn strength(&self) -> Relevance {
        self.strength
    }

    /// Returns this value's share of the positive evidence among the
    /// possibilities of the same view, interpolated across grades for a graded
    /// answer, or zero when its own evidence is zero or negative.
    pub fn probability(&self) -> Probability {
        self.probability
    }

    /// Returns the number of complete proof-bearing rows projecting this value.
    pub fn complete_rows(&self) -> usize {
        self.complete_rows
    }

    /// Returns the combinations of sources whose weights add up to this
    /// value's strength.
    pub fn support(&self) -> &[AnswerSupport] {
        &self.support
    }

    /// Returns whether this value is among the most probable, which choice
    /// separates by canonical spelling.
    pub fn is_top_tie(&self) -> bool {
        self.is_top_tie
    }
}

/// One combination of sources behind an [`AnswerPossibility`].
///
/// A complete row's own proof weighs the product of its distinct sources'
/// counts, so sources joined from separate experiences multiply while one
/// source proving several clauses counts once. Evidence imported by `@+=` or
/// `@-=` keeps its signed factor. The weight adds up every complete row that
/// the same combination supports at the same grade.
#[derive(Clone)]
pub struct AnswerSupport {
    grade: CompletionGrade,
    weight: Relevance,
    sources: Vec<AnswerSource>,
}

impl AnswerSupport {
    /// Returns how closely the supported rows answer the question as asked.
    pub fn grade(&self) -> CompletionGrade {
        self.grade
    }

    /// Returns the signed evidence this combination supplies.
    pub fn weight(&self) -> Relevance {
        self.weight
    }

    /// Returns the distinct sources whose counts multiply into the weight.
    pub fn sources(&self) -> &[AnswerSource] {
        &self.sources
    }
}

/// One source behind an [`AnswerSupport`].
#[derive(Clone)]
pub struct AnswerSource {
    subject: ConceptId,
    concept: ConceptId,
    relevance: Relevance,
}

impl AnswerSource {
    /// Returns the owning Percept for remembered experience, or the complete
    /// subject Concept for a direct question.
    pub fn subject(&self) -> &ConceptId {
        &self.subject
    }

    /// Returns the complete source Concept.
    pub fn concept(&self) -> &ConceptId {
        &self.concept
    }

    /// Returns the evidence count stored on the source Concept.
    pub fn relevance(&self) -> Relevance {
        self.relevance
    }
}

/// The result of functionally choosing or sampling an answer view.
pub struct AnswerChoice {
    pub(super) selected: ConceptId,
    pub(super) answer: AnswerView,
}

impl AnswerChoice {
    /// Returns the selected projected Concept.
    pub fn selected(&self) -> &ConceptId {
        &self.selected
    }

    /// Returns the conditioned answer view.
    pub fn view(&self) -> &AnswerView {
        &self.answer
    }

    /// Consumes this result and returns the conditioned answer view.
    pub fn into_view(self) -> AnswerView {
        self.answer
    }
}

/// An error produced while publishing a derived answer into live Percepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub(super) enum AnswerPublicationError {
    /// The answer has no live origin revision.
    Detached,
    /// The answer or its origin belongs to another engine.
    ForeignAnswer,
    /// The answer can no longer be materialized as its complete live output group.
    InvalidAnswer,
    /// The live answer has changed since this snapshot was created.
    Stale,
}

impl std::fmt::Display for AnswerPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Detached => formatter.write_str("answer has no live origin"),
            Self::ForeignAnswer => formatter.write_str("answer belongs to another Pangine engine"),
            Self::InvalidAnswer => formatter.write_str("answer cannot be published to its complete output group"),
            Self::Stale => formatter.write_str("live answer changed after this snapshot was created"),
        }
    }
}

impl std::error::Error for AnswerPublicationError {}

impl Pangine {
    /// Returns an immutable snapshot of the one live answer shared by every
    /// Percept in `concept`.
    pub fn answer_snapshot(&mut self, concept: &ConceptId) -> Option<Answer> {
        if !self.owns(concept) {
            return None;
        }
        let (value, live) = self.shared_live_answer(concept)?;
        self.answer_from_live_value(value, live)
    }

    /// Returns an immutable live-answer snapshot viewed through `projection`.
    pub fn answer_view(&mut self, projection: &ConceptId) -> Option<AnswerView> {
        self.answer_snapshot(projection)?.view(self, projection.clone())
    }

    fn answer_from_live_value(&mut self, value: ConceptId, live: LiveConceptAnswer) -> Option<Answer> {
        let shape = live.answer.shape(self)?;
        let result = Rc::new(live.answer.to_result(self)?);
        Some(Answer { result, shape, origin: Some(value) })
    }

    fn owns_answer(&self, answer: &Answer) -> bool {
        self.owns(&answer.shape)
            && self.owns(answer.result.question())
            && answer.result.completions().iter().all(|completion| completion.bindings().all(|(percept, value)| self.owns(percept) && self.owns(value)))
    }

    fn publish_answer(&mut self, answer: &Answer) -> Result<Answer, AnswerPublicationError> {
        if !self.owns_answer(answer) {
            return Err(AnswerPublicationError::ForeignAnswer);
        }

        let origin = answer.origin.as_ref().ok_or(AnswerPublicationError::Detached)?;
        if !self.owns(origin) {
            return Err(AnswerPublicationError::ForeignAnswer);
        }
        let expected = LiveConceptAnswer::decode(self, origin).ok_or(AnswerPublicationError::InvalidAnswer)?;

        if expected.answer.outputs.iter().any(|output| self.percept_values.get(&output.index()) != Some(origin)) {
            return Err(AnswerPublicationError::Stale);
        }

        let mut concept_answer = ConceptAnswer::from_result(self, &answer.result);
        concept_answer.outputs = expected.answer.outputs;
        concept_answer.questions = expected.answer.questions;
        let live = LiveConceptAnswer::successor(self, expected.revision, concept_answer).ok_or(AnswerPublicationError::InvalidAnswer)?;
        let value = self.install_live_answer(live.clone()).ok_or(AnswerPublicationError::InvalidAnswer)?;
        self.answer_from_live_value(value, live).ok_or(AnswerPublicationError::InvalidAnswer)
    }
}

#[cfg(test)]
mod tests;
