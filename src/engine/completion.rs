use super::{ConceptId, ConceptKind, ConceptMap, Pangine, ProjectionAssignment, QuestionCandidateWitnesses, QuestionSourceView};
use crate::Relevance;
use std::collections::{BTreeMap, BTreeSet};

/// Identifies which side of a structural match supplied an unmatched remainder.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompletionRemainderSide {
    /// The selected source contained structure not consumed by the question.
    Source,
    /// The question contained context not supplied by the selected source.
    Question,
}

/// Retains unmatched structure rather than silently treating it as irrelevant.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompletionRemainder {
    side: CompletionRemainderSide,
    ordered_path: Vec<usize>,
    concept: ConceptId,
}

impl CompletionRemainder {
    /// Returns the side that supplied the unmatched structure.
    pub fn side(&self) -> CompletionRemainderSide {
        self.side
    }

    /// Returns the ordered-component path containing the unmatched structure.
    pub fn ordered_path(&self) -> &[usize] {
        &self.ordered_path
    }

    /// Returns the unmatched structure.
    pub fn concept(&self) -> &ConceptId {
        &self.concept
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct StructuralCompletion {
    assignment: ProjectionAssignment,
    remainders: BTreeSet<CompletionRemainder>,
}

/// Describes one selected source fragment participating in a completion.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompletionEvidence {
    clause: ConceptId,
    source_view: QuestionSourceView,
    assignment: ProjectionAssignment,
    remainders: BTreeSet<CompletionRemainder>,
}

impl CompletionEvidence {
    /// Returns the question clause matched by this evidence.
    pub fn clause(&self) -> &ConceptId {
        &self.clause
    }

    /// Returns the selected Percept owning the source Concept, when a retained
    /// Percept supplied this evidence.
    pub fn source_percept(&self) -> Option<&ConceptId> {
        self.source_view.source.percept()
    }

    /// Returns the source subject supplying this evidence.
    ///
    /// For retained experience this is the individual selected Percept that
    /// owns the Concept. For a direct question it is the complete ordinary
    /// Concept supplied on the left.
    pub fn source_subject(&self) -> &ConceptId {
        self.source_view.source.subject()
    }

    /// Returns the complete source Concept supplying this evidence.
    ///
    /// A retained Percept supplies one of its direct subconcepts. A direct
    /// ordinary Concept supplies itself.
    pub fn source_concept(&self) -> &ConceptId {
        &self.source_view.source.concept
    }

    /// Returns the relevance attached to the source Concept.
    ///
    /// Direct ordinary Concepts have default relevance.
    pub fn source_relevance(&self) -> Relevance {
        self.source_view.source.relevance
    }

    /// Returns the recursive source view matched by the clause.
    pub fn matched(&self) -> &ConceptId {
        &self.source_view.matched
    }

    /// Returns the value assigned to `percept` by this evidence fragment.
    pub fn binding(&self, percept: &ConceptId) -> Option<&ConceptId> {
        self.assignment.get(percept)
    }

    /// Returns unmatched structure retained by this evidence fragment.
    pub fn remainders(&self) -> impl Iterator<Item = &CompletionRemainder> {
        self.remainders.iter()
    }
}

/// One correlated grounding of every Percept hole in a question.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Completion {
    assignment: ProjectionAssignment,
    evidence: Vec<CompletionEvidence>,
}

impl Completion {
    /// Returns the value assigned to `percept` in this completion.
    pub fn binding(&self, percept: &ConceptId) -> Option<&ConceptId> {
        self.assignment.get(percept)
    }

    /// Iterates over the complete correlated assignment.
    pub fn bindings(&self) -> impl Iterator<Item = (&ConceptId, &ConceptId)> {
        self.assignment.iter()
    }

    /// Returns the source fragments participating in this completion.
    pub fn evidence(&self) -> &[CompletionEvidence] {
        &self.evidence
    }
}

/// A structural question and all of its complete correlated groundings.
#[derive(Clone)]
pub struct CompletionResult {
    question: ConceptId,
    completions: Vec<Completion>,
}

impl CompletionResult {
    /// Returns the structural question supplied to the evaluator.
    pub fn question(&self) -> &ConceptId {
        &self.question
    }

    /// Returns every complete correlated grounding in canonical order.
    pub fn completions(&self) -> &[Completion] {
        &self.completions
    }
}

#[derive(Clone)]
struct Injection {
    completion: StructuralCompletion,
    used_candidates: BTreeSet<usize>,
}

impl Pangine {
    /// Completes a structural question against Concepts retained by Percepts.
    ///
    /// Top-level unordered collections of ordered clauses form one conjunction.
    /// Repeated Percept holes are shared variables, and each returned completion
    /// retains a correlated assignment, source evidence, and unmatched context.
    pub fn complete_question(&mut self, sources: &[ConceptId], question: &ConceptId) -> Option<CompletionResult> {
        if sources.is_empty()
            || !self.owns(question)
            || sources.iter().any(|source| !self.is_mutable_percept(source))
            || sources.iter().collect::<BTreeSet<_>>().len() != sources.len()
        {
            return None;
        }

        let snapshot = self.question_snapshot(sources, question);
        Some(self.complete_question_snapshot(question, &snapshot))
    }

    /// Completes a structural question against one ordinary Concept.
    ///
    /// The complete subject is treated as one source Concept with default
    /// relevance. It is not split into synthetic experiences, and the
    /// returned evidence therefore has no source Percept.
    pub fn complete_subject(&mut self, subject: &ConceptId, question: &ConceptId) -> Option<CompletionResult> {
        let mut contains_percept_cache = BTreeMap::new();
        if !self.owns(subject) || !self.owns(question) || self.contains_percept(subject, &mut contains_percept_cache) {
            return None;
        }

        let snapshot = self.subject_question_snapshot(subject, question);
        Some(self.complete_question_snapshot(question, &snapshot))
    }

    pub(super) fn complete_question_snapshot(&mut self, question: &ConceptId, snapshot: &super::QuestionSnapshot) -> CompletionResult {
        let clauses = question_clauses(question);
        let mut products = BTreeSet::from([Completion { assignment: ProjectionAssignment::new(), evidence: Vec::new() }]);

        for clause in clauses {
            let mut clause_evidence = BTreeSet::new();
            for source_view in &snapshot.source_views {
                for completion in self.structural_completions(&source_view.matched, &clause, &mut Vec::new()) {
                    clause_evidence.insert(CompletionEvidence {
                        clause: clause.clone(),
                        source_view: source_view.clone(),
                        assignment: completion.assignment,
                        remainders: completion.remainders,
                    });
                }
            }

            let mut next = BTreeSet::new();
            for product in products {
                for evidence in &clause_evidence {
                    let Some(assignment) = Self::merge_projection_assignments(&product.assignment, &evidence.assignment) else {
                        continue;
                    };
                    let mut joined_evidence = product.evidence.clone();
                    joined_evidence.push(evidence.clone());
                    next.insert(Completion { assignment, evidence: joined_evidence });
                }
            }
            products = next;
            if products.is_empty() {
                break;
            }
        }

        let mut outputs = BTreeSet::new();
        self.collect_output_percepts(question, &mut outputs);
        let completions = products.into_iter().filter(|completion| outputs.iter().all(|output| completion.assignment.contains_key(output))).collect();
        CompletionResult { question: question.clone(), completions }
    }

    /// Instantiates any ordinary Concept template from one correlated completion.
    pub fn instantiate_completion(&mut self, template: &ConceptId, completion: &Completion) -> Option<ConceptId> {
        if !self.owns(template) || completion.assignment.iter().any(|(percept, candidate)| !self.owns(percept) || !self.owns(candidate)) {
            return None;
        }
        self.instantiate_completion_inner(template, &completion.assignment)
    }

    pub(super) fn materialize_completion_bindings(&mut self, result: &CompletionResult) -> Option<BTreeMap<ConceptId, Option<ConceptId>>> {
        let mut outputs = BTreeSet::new();
        self.collect_output_percepts(&result.question, &mut outputs);
        let mut witnesses = outputs.iter().cloned().map(|percept| (percept, BTreeMap::new())).collect::<QuestionCandidateWitnesses>();

        for completion in &result.completions {
            for evidence in &completion.evidence {
                for output in &outputs {
                    let Some(candidate) = evidence.assignment.get(output) else {
                        continue;
                    };
                    if completion.assignment.get(output) != Some(candidate) {
                        continue;
                    }
                    witnesses.entry(output.clone()).or_default().entry(candidate.clone()).or_default().insert(evidence.source_view.source.clone());
                }
            }
        }

        self.materialize_question_witnesses(outputs, witnesses)
    }

    pub(super) fn materialize_completion_rows(&mut self, result: &CompletionResult) -> Option<ConceptId> {
        let mut rows = ConceptMap::new();
        for completion in &result.completions {
            let row = self.instantiate_completion_inner(&result.question, &completion.assignment)?;
            self.add_relevance(&mut rows, row, false, Relevance::DEFAULT)?;
        }
        self.reference_map(&rows)
    }

    fn structural_completions(&mut self, source: &ConceptId, question: &ConceptId, ordered_path: &mut Vec<usize>) -> BTreeSet<StructuralCompletion> {
        if self.is_percept(question) {
            return BTreeSet::from([StructuralCompletion {
                assignment: ProjectionAssignment::from([(question.clone(), source.clone())]),
                remainders: BTreeSet::new(),
            }]);
        }
        if source == question {
            return exact_structural_completion();
        }

        match (&source.0.kind, &question.0.kind) {
            (ConceptKind::Named(source_name), ConceptKind::Named(question_name)) => {
                if source_name == question_name {
                    exact_structural_completion()
                } else {
                    BTreeSet::new()
                }
            }
            (ConceptKind::Ordered { components: source_components }, ConceptKind::Ordered { components: question_components }) => {
                if source_components.len() != question_components.len() {
                    return BTreeSet::new();
                }

                let pairs = source_components.iter().zip(question_components).map(|(source, question)| (source.clone(), question.clone())).collect::<Vec<_>>();
                let mut products = exact_structural_completion();
                for (position, (source_component, question_component)) in pairs.into_iter().enumerate() {
                    ordered_path.push(position);
                    let component = self.structural_completions(&source_component, &question_component, ordered_path);
                    ordered_path.pop();
                    products = multiply_structural_completions(&products, &component);
                    if products.is_empty() {
                        break;
                    }
                }
                products
            }
            (ConceptKind::Unordered, ConceptKind::Unordered) => self.unordered_completions(source, question, ordered_path),
            _ => BTreeSet::new(),
        }
    }

    fn unordered_completions(&mut self, source: &ConceptId, question: &ConceptId, ordered_path: &[usize]) -> BTreeSet<StructuralCompletion> {
        if source.0.subconcepts.values().any(|relevance| *relevance != Relevance::DEFAULT)
            || question.0.subconcepts.values().any(|relevance| *relevance != Relevance::DEFAULT)
        {
            return BTreeSet::new();
        }

        let source_members = source.0.subconcepts.keys().cloned().collect::<Vec<_>>();
        let question_members = question.0.subconcepts.keys().cloned().collect::<Vec<_>>();
        let question_outputs = question_members.iter().filter(|concept| self.is_percept(concept)).cloned().collect::<Vec<_>>();
        let question_fixed = question_members.iter().filter(|concept| !self.is_percept(concept)).cloned().collect::<Vec<_>>();
        let mut completions = BTreeSet::new();

        if question_fixed.len() <= source_members.len() {
            for injection in self.inject_members(&question_fixed, &source_members, true, ordered_path) {
                let remaining = unused_members(&source_members, &injection.used_candidates);
                if question_outputs.is_empty() {
                    let mut completion = injection.completion;
                    self.add_completion_remainder(&mut completion, CompletionRemainderSide::Source, ordered_path, &remaining);
                    completions.insert(completion);
                } else if question_outputs.len() == 1 {
                    let Some(remainder) = self.reference_completion_members(&remaining) else {
                        continue;
                    };
                    let output = &question_outputs[0];
                    let output_assignment = ProjectionAssignment::from([(output.clone(), remainder)]);
                    let Some(assignment) = Self::merge_projection_assignments(&injection.completion.assignment, &output_assignment) else {
                        continue;
                    };
                    completions.insert(StructuralCompletion { assignment, remainders: injection.completion.remainders });
                } else if remaining.len() == question_outputs.len() {
                    for outputs in self.inject_members(&question_outputs, &remaining, true, ordered_path) {
                        let Some(assignment) = Self::merge_projection_assignments(&injection.completion.assignment, &outputs.completion.assignment) else {
                            continue;
                        };
                        let mut remainders = injection.completion.remainders.clone();
                        remainders.extend(outputs.completion.remainders);
                        completions.insert(StructuralCompletion { assignment, remainders });
                    }
                }
            }
        }

        if question_outputs.is_empty() && source_members.len() <= question_fixed.len() {
            for injection in self.inject_members(&source_members, &question_fixed, false, ordered_path) {
                let remaining = unused_members(&question_fixed, &injection.used_candidates);
                let mut completion = injection.completion;
                self.add_completion_remainder(&mut completion, CompletionRemainderSide::Question, ordered_path, &remaining);
                completions.insert(completion);
            }
        }

        completions
    }

    fn inject_members(&mut self, patterns: &[ConceptId], candidates: &[ConceptId], patterns_are_questions: bool, ordered_path: &[usize]) -> Vec<Injection> {
        let mut injections = vec![Injection {
            completion: StructuralCompletion { assignment: ProjectionAssignment::new(), remainders: BTreeSet::new() },
            used_candidates: BTreeSet::new(),
        }];
        for pattern in patterns {
            let mut next = Vec::new();
            for injection in injections {
                for (candidate_index, candidate) in candidates.iter().enumerate() {
                    if injection.used_candidates.contains(&candidate_index) {
                        continue;
                    }
                    let mut path = ordered_path.to_vec();
                    let matches = if patterns_are_questions {
                        self.structural_completions(candidate, pattern, &mut path)
                    } else {
                        self.structural_completions(pattern, candidate, &mut path)
                    };
                    for matched in matches {
                        let Some(assignment) = Self::merge_projection_assignments(&injection.completion.assignment, &matched.assignment) else {
                            continue;
                        };
                        let mut remainders = injection.completion.remainders.clone();
                        remainders.extend(matched.remainders);
                        let mut used_candidates = injection.used_candidates.clone();
                        used_candidates.insert(candidate_index);
                        next.push(Injection { completion: StructuralCompletion { assignment, remainders }, used_candidates });
                    }
                }
            }
            injections = next;
            if injections.is_empty() {
                break;
            }
        }
        injections
    }

    fn add_completion_remainder(
        &mut self,
        completion: &mut StructuralCompletion,
        side: CompletionRemainderSide,
        ordered_path: &[usize],
        members: &[ConceptId],
    ) {
        if let Some(concept) = self.reference_completion_members(members) {
            completion.remainders.insert(CompletionRemainder { side, ordered_path: ordered_path.to_vec(), concept });
        }
    }

    fn reference_completion_members(&mut self, members: &[ConceptId]) -> Option<ConceptId> {
        let map = members.iter().cloned().map(|concept| (concept, Relevance::DEFAULT)).collect::<ConceptMap>();
        self.reference_map(&map)
    }

    fn instantiate_completion_inner(&mut self, template: &ConceptId, assignment: &ProjectionAssignment) -> Option<ConceptId> {
        if self.is_percept(template) {
            return assignment.get(template).cloned();
        }

        match &template.0.kind {
            ConceptKind::Named(_) => Some(template.clone()),
            ConceptKind::Percept { .. } => None,
            ConceptKind::Ordered { components } => {
                let components = components.clone();
                let instantiated = components.iter().map(|component| self.instantiate_completion_inner(component, assignment)).collect::<Option<Vec<_>>>()?;
                Some(self.reference_ordered(instantiated))
            }
            ConceptKind::Unordered => {
                let entries = template.0.subconcepts.clone();
                let mut map = ConceptMap::new();
                for (concept, relevance) in entries {
                    let concept = self.instantiate_completion_inner(&concept, assignment)?;
                    self.add_union_concept(&mut map, concept, false, relevance)?;
                }
                self.reference_map(&map)
            }
        }
    }
}

fn question_clauses(question: &ConceptId) -> Vec<ConceptId> {
    if matches!(question.0.kind, ConceptKind::Unordered)
        && question.0.subconcepts.len() > 1
        && question.0.subconcepts.values().all(|relevance| *relevance == Relevance::DEFAULT)
        && question.0.subconcepts.keys().all(|concept| matches!(concept.0.kind, ConceptKind::Ordered { .. }))
    {
        return question.0.subconcepts.keys().cloned().collect();
    }
    vec![question.clone()]
}

fn exact_structural_completion() -> BTreeSet<StructuralCompletion> {
    BTreeSet::from([StructuralCompletion { assignment: ProjectionAssignment::new(), remainders: BTreeSet::new() }])
}

fn multiply_structural_completions(left: &BTreeSet<StructuralCompletion>, right: &BTreeSet<StructuralCompletion>) -> BTreeSet<StructuralCompletion> {
    let mut products = BTreeSet::new();
    for left in left {
        for right in right {
            let Some(assignment) = Pangine::merge_projection_assignments(&left.assignment, &right.assignment) else {
                continue;
            };
            let mut remainders = left.remainders.clone();
            remainders.extend(right.remainders.iter().cloned());
            products.insert(StructuralCompletion { assignment, remainders });
        }
    }
    products
}

fn unused_members(members: &[ConceptId], used: &BTreeSet<usize>) -> Vec<ConceptId> {
    members.iter().enumerate().filter(|(index, _)| !used.contains(index)).map(|(_, concept)| concept.clone()).collect()
}
