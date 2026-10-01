use super::{concept_answer::ConceptAnswer, concept_answer::LiveConceptAnswer, CompletionResult, ConceptId, Pangine};
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
    /// evidence count, probability, and distinct source contributions.
    ///
    /// A possibility's probability is its share of the positive evidence among
    /// these possibilities. A possibility whose evidence is zero or negative
    /// stays in the list with probability zero, and equal counts keep canonical
    /// spelling order. `is_top_tie` identifies every most probable possibility;
    /// it is false for every possibility when none has positive evidence and
    /// choice abstains.
    pub fn possibilities(&self, pangine: &mut Pangine) -> Option<Vec<AnswerPossibility>> {
        if !pangine.owns_answer(&self.answer) {
            return None;
        }

        let witnesses = pangine.completion_projection_witnesses(&self.answer.result, &self.projection)?;
        let mut complete_rows = BTreeMap::new();
        for completion in &self.answer.result.completions {
            let value = pangine.instantiate_completion(&self.projection, completion)?;
            *complete_rows.entry(value).or_insert(0) += 1;
        }

        let mut possibilities = witnesses
            .into_iter()
            .map(|(value, witnesses)| {
                let strength = pangine.question_source_support(&witnesses)?;
                let sources = witnesses
                    .into_iter()
                    .map(|witness| AnswerSourceContribution {
                        subject: witness.source.subject().clone(),
                        concept: witness.source.concept,
                        relevance: witness.source.relevance,
                        contribution: witness.contribution,
                    })
                    .collect();
                Some(AnswerPossibility {
                    complete_rows: complete_rows.remove(&value).unwrap_or_default(),
                    value,
                    strength,
                    sources,
                    positive_total: 0,
                    is_top_tie: false,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        possibilities.sort_by_cached_key(|possibility| (Reverse(possibility.strength), pangine.format_concept(&possibility.value, false)));

        let positive_total = possibilities.iter().map(|possibility| i128::from(possibility.strength.count().max(0))).sum();
        let greatest_positive = possibilities.first().map(|possibility| possibility.strength).filter(|strength| strength.count() > 0);
        for possibility in &mut possibilities {
            possibility.positive_total = positive_total;
            possibility.is_top_tie = Some(possibility.strength) == greatest_positive;
        }
        Some(possibilities)
    }

    /// Chooses this projection and returns the selected Concept together with
    /// a new answer containing only compatible complete rows.
    pub fn choose(&self, pangine: &mut Pangine) -> Option<AnswerChoice> {
        if !pangine.owns_answer(&self.answer) {
            return None;
        }

        let (selected, result) = pangine.choose_completion_result(&self.answer.result, &self.projection)?;
        let answer = self.answer.derived(result);
        Some(AnswerChoice { selected, answer: answer.view(pangine, self.projection.clone())? })
    }

    /// Adds signed source evidence from matching rows of another answer view.
    ///
    /// The returned answer keeps this view's answer shape. `factor` multiplies
    /// each imported source contribution; it does not change the source's raw
    /// relevance. An empty factor leaves the answer unchanged.
    pub fn adjusted_by(&self, pangine: &mut Pangine, adjustment: &AnswerView, factor: Relevance) -> Option<AnswerView> {
        if !pangine.owns_answer(&self.answer) || !pangine.owns_answer(&adjustment.answer) {
            return None;
        }

        let mut target_outputs = BTreeSet::new();
        pangine.collect_output_percepts(&self.answer.shape, &mut target_outputs);
        let result = pangine.adjust_completion_result(
            &self.answer.result,
            &self.projection,
            &adjustment.answer.result,
            &adjustment.projection,
            &target_outputs,
            factor,
        )?;
        self.answer.derived(result).view(pangine, self.projection.clone())
    }
}

/// One projected value retained by an [`AnswerView`].
#[derive(Clone)]
pub struct AnswerPossibility {
    value: ConceptId,
    strength: Relevance,
    complete_rows: usize,
    sources: Vec<AnswerSourceContribution>,
    /// The positive evidence of every possibility in the same view.
    positive_total: i128,
    is_top_tie: bool,
}

impl AnswerPossibility {
    /// Returns the projected value.
    pub fn value(&self) -> &ConceptId {
        &self.value
    }

    /// Returns the signed evidence count behind this value, the sum of its
    /// distinct source contributions.
    pub fn strength(&self) -> Relevance {
        self.strength
    }

    /// Returns this value's share of the positive evidence among the
    /// possibilities of the same view, or zero when its own evidence is zero
    /// or negative.
    pub fn probability(&self) -> f64 {
        let (numerator, denominator) = self.probability_fraction();
        if numerator == 0 {
            0.0
        } else {
            numerator as f64 / denominator as f64
        }
    }

    /// Returns the number of complete proof-bearing rows projecting this value.
    pub fn complete_rows(&self) -> usize {
        self.complete_rows
    }

    /// Returns the distinct source contributions used to calculate strength.
    pub fn sources(&self) -> &[AnswerSourceContribution] {
        &self.sources
    }

    /// Returns whether this value is among the most probable, which choice
    /// separates by canonical spelling.
    pub fn is_top_tie(&self) -> bool {
        self.is_top_tie
    }

    /// Returns the probability as this value's positive evidence over the
    /// view's positive total, without reducing the fraction.
    pub(super) fn probability_fraction(&self) -> (i128, i128) {
        (i128::from(self.strength.count().max(0)), self.positive_total)
    }
}

/// One distinct source contribution to an [`AnswerPossibility`].
#[derive(Clone)]
pub struct AnswerSourceContribution {
    subject: ConceptId,
    concept: ConceptId,
    relevance: Relevance,
    contribution: Relevance,
}

impl AnswerSourceContribution {
    /// Returns the owning Percept for remembered experience, or the complete
    /// subject Concept for a direct question.
    pub fn subject(&self) -> &ConceptId {
        &self.subject
    }

    /// Returns the complete source Concept supplying the contribution.
    pub fn concept(&self) -> &ConceptId {
        &self.concept
    }

    /// Returns the evidence count stored on the source Concept.
    pub fn relevance(&self) -> Relevance {
        self.relevance
    }

    /// Returns the signed amount contributed to this answer.
    pub fn contribution(&self) -> Relevance {
        self.contribution
    }
}

/// The result of functionally choosing an answer view.
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
