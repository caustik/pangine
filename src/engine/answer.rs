use super::{Completion, CompletionResult, ConceptId, Pangine};
use crate::Relevance;
use std::collections::BTreeSet;
use std::rc::Rc;

type SourceContributionKey = (ConceptId, ConceptId, Relevance, Relevance);

#[derive(Clone)]
pub(super) struct StoredAnswer {
    pub(super) result: Rc<CompletionResult>,
    pub(super) outputs: BTreeSet<ConceptId>,
    pub(super) questions: BTreeSet<ConceptId>,
}

#[derive(Clone)]
pub(super) struct AnswerOrigin {
    pub(super) state_id: usize,
    pub(super) outputs: BTreeSet<ConceptId>,
    pub(super) questions: BTreeSet<ConceptId>,
}

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
    pub(super) origin: Option<Rc<AnswerOrigin>>,
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

    /// Replaces the live answer revision from which this snapshot was derived.
    ///
    /// Publication fails when another operation has already changed or
    /// detached any output in that revision.
    pub fn publish(&self, pangine: &mut Pangine) -> Result<AnswerPublication, AnswerPublicationError> {
        pangine.publish_answer(self)
    }

    fn derived(&self, result: CompletionResult) -> Self {
        Self { result: Rc::new(result), shape: self.shape.clone(), origin: self.origin.clone() }
    }

    #[cfg(test)]
    pub(super) fn detached(result: CompletionResult) -> Self {
        let shape = result.question().clone();
        Self { result: Rc::new(result), shape, origin: None }
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

    /// Materializes the current strengths of this projection without changing
    /// the answer or any live Percept.
    pub fn materialize(&self, pangine: &mut Pangine) -> Option<ConceptId> {
        if !pangine.owns_answer(&self.answer) {
            return None;
        }
        pangine.materialize_completion_projection(&self.answer.result, &self.projection)
    }

    /// Chooses this projection and returns the selected Concept together with
    /// a new answer containing only compatible complete rows.
    pub fn choose(&self, pangine: &mut Pangine) -> Option<AnswerChoice> {
        if !pangine.owns_answer(&self.answer) {
            return None;
        }

        let witnesses = pangine.completion_projection_witnesses(&self.answer.result, &self.projection)?;
        let selected = pangine.select_projection_candidate(&witnesses)?;
        let mut result = self.answer.result.as_ref().clone();
        result.completions.retain(|completion| pangine.instantiate_completion(&self.projection, completion).as_ref() == Some(&selected));
        if result.completions.is_empty() {
            return None;
        }

        let answer = self.answer.derived(result);
        Some(AnswerChoice { selected, answer: answer.view(pangine, self.projection.clone())? })
    }

    /// Adds signed source evidence from matching rows of another answer view.
    ///
    /// The returned answer keeps this view's answer shape. `factor` multiplies
    /// each imported source contribution; it does not change the source's raw
    /// relevance. An empty factor can still report matching rows, but leaves
    /// the answer unchanged.
    pub fn adjust(&self, pangine: &mut Pangine, adjustment: &AnswerView, factor: Relevance) -> Option<AnswerAdjustment> {
        if !pangine.owns_answer(&self.answer) || !pangine.owns_answer(&adjustment.answer) {
            return None;
        }

        let adjustment_candidates = adjustment
            .answer
            .result
            .completions
            .iter()
            .map(|completion| pangine.instantiate_completion(&adjustment.projection, completion))
            .collect::<Option<Vec<_>>>()?;
        let mut matched_adjustments = BTreeSet::new();
        let mut matched_target_rows = 0;
        let mut matched_pairs = 0;
        for completion in &self.answer.result.completions {
            let candidate = pangine.instantiate_completion(&self.projection, completion)?;
            let matches =
                adjustment_candidates.iter().enumerate().filter_map(|(index, adjustment)| (adjustment == &candidate).then_some(index)).collect::<Vec<_>>();
            if !matches.is_empty() {
                matched_target_rows += 1;
                matched_pairs += matches.len();
                matched_adjustments.extend(matches);
            }
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
        let changed_target_rows = result.completions.iter().filter(|completion| !self.answer.result.completions.contains(completion)).count();
        let added_source_occurrences = result
            .completions
            .iter()
            .map(|adjusted| {
                let before = self
                    .answer
                    .result
                    .completions
                    .iter()
                    .find(|target| target.bindings().eq(adjusted.bindings()))
                    .map(source_contribution_keys)
                    .unwrap_or_default();
                source_contribution_keys(adjusted).difference(&before).count()
            })
            .sum();
        let answer = self.answer.derived(result);
        Some(AnswerAdjustment {
            answer: answer.view(pangine, self.projection.clone())?,
            target_rows: self.answer.result.completions.len(),
            adjustment_rows: adjustment.answer.result.completions.len(),
            matched_target_rows,
            matched_adjustment_rows: matched_adjustments.len(),
            matched_pairs,
            changed_target_rows,
            added_source_occurrences,
        })
    }

    /// Returns only the adjusted view when receipt measurements are not needed.
    pub fn adjusted_by(&self, pangine: &mut Pangine, adjustment: &AnswerView, factor: Relevance) -> Option<AnswerView> {
        Some(self.adjust(pangine, adjustment, factor)?.answer)
    }

    #[cfg(test)]
    pub(super) fn from_result(pangine: &Pangine, result: CompletionResult, projection: ConceptId) -> Option<Self> {
        Answer::detached(result).view(pangine, projection)
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

/// Measurements from one functional answer adjustment.
pub struct AnswerAdjustment {
    pub(super) answer: AnswerView,
    pub(super) target_rows: usize,
    pub(super) adjustment_rows: usize,
    pub(super) matched_target_rows: usize,
    pub(super) matched_adjustment_rows: usize,
    pub(super) matched_pairs: usize,
    pub(super) changed_target_rows: usize,
    pub(super) added_source_occurrences: usize,
}

impl AnswerAdjustment {
    /// Returns the adjusted answer view.
    pub fn view(&self) -> &AnswerView {
        &self.answer
    }

    /// Consumes this result and returns the adjusted answer view.
    pub fn into_view(self) -> AnswerView {
        self.answer
    }

    /// Returns the number of complete rows in the target answer.
    pub fn target_rows(&self) -> usize {
        self.target_rows
    }

    /// Returns the number of complete rows in the adjustment answer.
    pub fn adjustment_rows(&self) -> usize {
        self.adjustment_rows
    }

    /// Returns the number of target rows matched at least once.
    pub fn matched_target_rows(&self) -> usize {
        self.matched_target_rows
    }

    /// Returns the number of adjustment rows matched at least once.
    pub fn matched_adjustment_rows(&self) -> usize {
        self.matched_adjustment_rows
    }

    /// Returns the number of matching target-adjustment row pairs.
    pub fn matched_pairs(&self) -> usize {
        self.matched_pairs
    }

    /// Returns the number of target rows whose complete retained evidence changed.
    pub fn changed_target_rows(&self) -> usize {
        self.changed_target_rows
    }

    /// Returns the number of new signed source occurrences across target rows.
    pub fn added_source_occurrences(&self) -> usize {
        self.added_source_occurrences
    }
}

/// The result of replacing one current live answer revision.
pub struct AnswerPublication {
    pub(super) answer: Answer,
    pub(super) prior_state_id: usize,
    pub(super) state_id: usize,
}

impl AnswerPublication {
    /// Returns the newly current immutable answer snapshot.
    pub fn answer(&self) -> &Answer {
        &self.answer
    }

    /// Consumes this receipt and returns the newly current answer snapshot.
    pub fn into_answer(self) -> Answer {
        self.answer
    }

    /// Returns the live revision replaced by this publication.
    pub fn prior_revision(&self) -> usize {
        self.prior_state_id
    }

    /// Returns the new live revision created by this publication.
    pub fn revision(&self) -> usize {
        self.state_id
    }
}

/// An error produced while publishing a derived answer into live Percepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnswerPublicationError {
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
        let state_id = self.shared_answer_state(concept)?;
        self.answer_from_state(state_id)
    }

    /// Returns an immutable live-answer snapshot viewed through `projection`.
    pub fn answer_view(&mut self, projection: &ConceptId) -> Option<AnswerView> {
        self.answer_snapshot(projection)?.view(self, projection.clone())
    }

    pub(super) fn insert_answer_state(&mut self, state: StoredAnswer) -> usize {
        let state_id = self.next_answer_state_id;
        self.next_answer_state_id += 1;
        for output in &state.outputs {
            self.percept_answer_states.insert(output.index(), state_id);
        }
        self.answer_states.insert(state_id, state);
        state_id
    }

    pub(super) fn answer_from_state(&mut self, state_id: usize) -> Option<Answer> {
        let state = self.answer_states.get(&state_id)?.clone();
        let components = self.visible_answer_components(&state);
        let shape = self.answer_shape(&components)?;
        let origin = AnswerOrigin { state_id, outputs: state.outputs, questions: state.questions };
        Some(Answer { result: state.result, shape, origin: Some(Rc::new(origin)) })
    }

    fn owns_answer(&self, answer: &Answer) -> bool {
        self.owns(&answer.shape)
            && self.owns(answer.result.question())
            && answer.result.completions().iter().all(|completion| completion.bindings().all(|(percept, value)| self.owns(percept) && self.owns(value)))
    }

    fn publish_answer(&mut self, answer: &Answer) -> Result<AnswerPublication, AnswerPublicationError> {
        if !self.owns_answer(answer) {
            return Err(AnswerPublicationError::ForeignAnswer);
        }

        let origin = answer.origin.as_ref().ok_or(AnswerPublicationError::Detached)?;
        if origin.outputs.iter().chain(&origin.questions).any(|concept| !self.owns(concept)) {
            return Err(AnswerPublicationError::ForeignAnswer);
        }

        let state = self.answer_states.get(&origin.state_id).ok_or(AnswerPublicationError::Stale)?;
        if state.outputs != origin.outputs || state.questions != origin.questions {
            return Err(AnswerPublicationError::Stale);
        }
        if origin.outputs.iter().any(|output| self.percept_answer_states.get(&output.index()) != Some(&origin.state_id)) {
            return Err(AnswerPublicationError::Stale);
        }

        let projections = self.materialize_answer_projections(&answer.result, &origin.outputs).ok_or(AnswerPublicationError::InvalidAnswer)?;
        self.answer_states.remove(&origin.state_id);
        for output in &origin.outputs {
            self.percept_answer_states.remove(&output.index());
        }
        self.write_answer_projections(projections);

        let state_id =
            self.insert_answer_state(StoredAnswer { result: answer.result.clone(), outputs: origin.outputs.clone(), questions: origin.questions.clone() });
        let answer = self.answer_from_state(state_id).ok_or(AnswerPublicationError::InvalidAnswer)?;
        Ok(AnswerPublication { answer, prior_state_id: origin.state_id, state_id })
    }
}

fn source_contribution_keys(completion: &Completion) -> BTreeSet<SourceContributionKey> {
    completion
        .evidence()
        .iter()
        .map(|evidence| (evidence.source_subject().clone(), evidence.source_concept().clone(), evidence.source_relevance(), evidence.source_contribution()))
        .collect()
}
