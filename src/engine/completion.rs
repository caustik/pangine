use super::{CompletionProjectionWitnesses, ConceptId, ConceptKind, ConceptMap, Pangine, ProjectionAssignment, QuestionSourceView};
use crate::Relevance;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// Identifies one proper contiguous ordered window projected from a complete
/// ordered source Concept.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompletionOrderedWindow {
    pub(super) parent: ConceptId,
    pub(super) parent_occurrence: Vec<CompletionOrderedStep>,
    pub(super) start: usize,
    pub(super) width: usize,
}

impl CompletionOrderedWindow {
    /// Returns the complete ordered Concept containing the projected window.
    pub fn parent(&self) -> &ConceptId {
        &self.parent
    }

    /// Iterates over the ordered component steps locating `parent` inside the
    /// complete source Concept.
    pub fn parent_occurrence(&self) -> impl Iterator<Item = &CompletionOrderedStep> {
        self.parent_occurrence.iter()
    }

    /// Returns the zero-based component at which the window begins.
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the number of ordered components in the window.
    pub fn width(&self) -> usize {
        self.width
    }
}

/// Identifies one ordered-component descent on the way to a projected window.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompletionOrderedStep {
    pub(super) parent: ConceptId,
    pub(super) position: usize,
}

impl CompletionOrderedStep {
    /// Returns the ordered Concept containing this component occurrence.
    pub fn parent(&self) -> &ConceptId {
        &self.parent
    }

    /// Returns the zero-based component position selected from `parent`.
    pub fn position(&self) -> usize {
        self.position
    }
}

/// Identifies the represented source occurrence that supplied one Percept
/// binding along a route.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompletionBindingOrigin {
    pub(super) parent: ConceptId,
    pub(super) parent_occurrence: Vec<CompletionOrderedStep>,
    pub(super) span_start: usize,
    pub(super) span_width: usize,
    pub(super) nested_path: Vec<usize>,
}

impl CompletionBindingOrigin {
    /// Returns the complete ordered Concept containing the bound occurrence.
    pub fn parent(&self) -> &ConceptId {
        &self.parent
    }

    /// Iterates over the ordered component steps locating `parent` inside the
    /// complete source Concept.
    pub fn parent_occurrence(&self) -> impl Iterator<Item = &CompletionOrderedStep> {
        self.parent_occurrence.iter()
    }

    /// Returns the zero-based start of the bound span inside `parent`.
    pub fn span_start(&self) -> usize {
        self.span_start
    }

    /// Returns the width of the bound span inside `parent`.
    pub fn span_width(&self) -> usize {
        self.span_width
    }

    /// Returns any ordered-component path nested beneath the selected span.
    pub fn nested_path(&self) -> &[usize] {
        &self.nested_path
    }
}

/// One correlated route by which a source view can participate in a
/// completion.
///
/// Selected entries and binding origins constrain same-source joins.
/// Coefficient ancestors and ordered-window descriptions remain annotations;
/// none is interpreted as a count, score, or new Concept identity.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompletionRoute {
    pub(super) coefficient_ancestors: BTreeSet<ConceptId>,
    pub(super) selected_entries: BTreeMap<ConceptId, ConceptId>,
    pub(super) ordered_windows: BTreeSet<CompletionOrderedWindow>,
    pub(super) binding_origins: BTreeMap<ConceptId, BTreeSet<CompletionBindingOrigin>>,
}

impl CompletionRoute {
    /// Iterates over coefficient-bearing Concepts crossed along this route.
    pub fn coefficient_ancestors(&self) -> impl Iterator<Item = &ConceptId> {
        self.coefficient_ancestors.iter()
    }

    /// Iterates over `(container, selected entry)` commitments on this route.
    pub fn selected_entries(&self) -> impl Iterator<Item = (&ConceptId, &ConceptId)> {
        self.selected_entries.iter()
    }

    /// Iterates over proper ordered windows projected along this route.
    pub fn ordered_windows(&self) -> impl Iterator<Item = &CompletionOrderedWindow> {
        self.ordered_windows.iter()
    }

    /// Iterates over the source occurrences associated with each constrained
    /// Percept binding on this route.
    ///
    /// A missing Percept has no occurrence constraint. Several origins for one
    /// Percept are alternatives; same-source route joins retain their
    /// intersection.
    pub fn binding_origins(&self) -> impl Iterator<Item = (&ConceptId, &BTreeSet<CompletionBindingOrigin>)> {
        self.binding_origins.iter()
    }
}

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

#[derive(Clone)]
struct StructuralCompletion {
    assignment: ProjectionAssignment,
    binding_paths: BTreeMap<ConceptId, BTreeSet<Vec<usize>>>,
    remainders: BTreeSet<CompletionRemainder>,
}

impl PartialEq for StructuralCompletion {
    fn eq(&self, other: &Self) -> bool {
        self.assignment == other.assignment && self.remainders == other.remainders
    }
}

impl Eq for StructuralCompletion {}

impl PartialOrd for StructuralCompletion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StructuralCompletion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.assignment.cmp(&other.assignment).then_with(|| self.remainders.cmp(&other.remainders))
    }
}

/// Describes one selected source fragment participating in a completion.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompletionEvidence {
    clause: ConceptId,
    source_view: QuestionSourceView,
    source_route_products: BTreeSet<CompletionRoute>,
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

    /// Iterates over the alternative correlated routes to this matched source
    /// view before it joins the other clauses in the completion.
    pub fn routes(&self) -> impl Iterator<Item = &CompletionRoute> {
        self.source_view.routes.iter()
    }

    /// Iterates over the surviving route constraints for this complete source
    /// participation after all same-source clauses have joined.
    ///
    /// Every evidence fragment from the same source in one completion exposes
    /// the same set. These compact products retain selected entries and the
    /// binding origins shared across clauses. Full coefficient and window
    /// annotations remain on each fragment's [`Self::routes`] and can be kept
    /// factorized by fragment identity rather than expanded as a Cartesian
    /// product during recognition.
    pub fn source_route_products(&self) -> impl Iterator<Item = &CompletionRoute> {
        self.source_route_products.iter()
    }

    /// Iterates over the alternative coefficient-ancestor routes that reached
    /// this canonical matched source view.
    ///
    /// Each inner set contains ordinary Concepts such as `x2(F)` crossed along
    /// one route. Different inner sets are alternatives, not simultaneous
    /// factors. A route for an exact wrapper match is empty because it crosses
    /// no coefficient boundary.
    pub fn coefficient_ancestor_routes(&self) -> impl Iterator<Item = &BTreeSet<ConceptId>> {
        self.source_view.routes.iter().map(|route| &route.coefficient_ancestors)
    }

    /// Iterates over coefficient-bearing ancestors across every alternative
    /// route to this matched view.
    ///
    /// An ancestor shared by several routes can occur more than once. Use
    /// [`Self::coefficient_ancestor_routes`] when the distinction between
    /// alternatives matters. Equal ancestors under distinct source owners are
    /// distinguishable only when paired with this evidence's source fields.
    /// Nothing interprets a coefficient as occurrences, support, or a score.
    pub fn coefficient_ancestors(&self) -> impl Iterator<Item = &ConceptId> {
        self.source_view.routes.iter().flat_map(|route| route.coefficient_ancestors.iter())
    }

    /// Iterates over complete source entries selected while reaching this view.
    ///
    /// Each pair is `(containing Concept, selected entry)`. Evidence fragments
    /// from the same source can join only when they select the same entry for a
    /// shared container. Direct grouped unordered entries are selected
    /// immediately. A complete ordered entry is selected when one of its proper
    /// windows participates, and a coefficient around either entry preserves
    /// that boundary. Flat relation atoms, including atoms that merely contain
    /// a nested group, add no selection. This flattened compatibility view can
    /// repeat or discard correlations across alternative routes; use
    /// [`Self::routes`] for exact route-local selections. Both values are
    /// ordinary Concepts, so this is derived provenance rather than a new
    /// stored value type.
    pub fn selected_entries(&self) -> impl Iterator<Item = (&ConceptId, &ConceptId)> {
        self.source_view.routes.iter().flat_map(|route| route.selected_entries.iter())
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

/// One proof-bearing correlated grounding of every Percept hole in a question.
/// Distinct clause-to-source proofs can therefore produce distinct completions
/// with the same grounded assignment.
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

/// A structural question and all of its proof-bearing correlated groundings.
#[derive(Clone)]
pub struct CompletionResult {
    question: ConceptId,
    pub(super) completions: Vec<Completion>,
}

impl CompletionResult {
    /// Returns the structural question supplied to the evaluator.
    pub fn question(&self) -> &ConceptId {
        &self.question
    }

    /// Returns every proof-bearing correlated grounding in canonical order.
    pub fn completions(&self) -> &[Completion] {
        &self.completions
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
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
        let shared_percepts = self.shared_clause_percepts(&clauses);
        let mut products = BTreeSet::from([Completion { assignment: ProjectionAssignment::new(), evidence: Vec::new() }]);

        for clause in clauses {
            let mut clause_evidence = BTreeSet::new();
            for ((source, matched, _), routes) in &snapshot.source_views {
                for completion in self.source_view_completions(matched, &clause) {
                    let source_view = QuestionSourceView {
                        source: source.clone(),
                        matched: matched.clone(),
                        routes: routes_with_binding_origins(routes, &completion.binding_paths),
                    };
                    let source_route_products = source_route_constraints(&source_view.routes, &shared_percepts);
                    clause_evidence.insert(CompletionEvidence {
                        clause: clause.clone(),
                        source_route_products,
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
                    let existing_routes = joined_evidence
                        .iter()
                        .find(|joined| joined.source_view.source == evidence.source_view.source)
                        .map(|joined| &joined.source_route_products);
                    let source_route_products = match existing_routes {
                        Some(existing_routes) => join_source_route_relations(existing_routes, &evidence.source_route_products),
                        None => evidence.source_route_products.clone(),
                    };
                    if source_route_products.is_empty() {
                        continue;
                    }
                    for joined in &mut joined_evidence {
                        if joined.source_view.source == evidence.source_view.source {
                            joined.source_route_products = source_route_products.clone();
                        }
                    }
                    let mut evidence = evidence.clone();
                    evidence.source_route_products = source_route_products;
                    joined_evidence.push(evidence);
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

    pub(super) fn join_completion_results(
        &self,
        left: &CompletionResult,
        left_outputs: &BTreeSet<ConceptId>,
        right: &CompletionResult,
        right_outputs: &BTreeSet<ConceptId>,
        question: &ConceptId,
    ) -> CompletionResult {
        let active_outputs = left_outputs.union(right_outputs).cloned().collect::<BTreeSet<_>>();
        let mut completions = BTreeSet::new();
        for left_completion in &left.completions {
            for right_completion in &right.completions {
                let left_assignment = active_completion_assignment(left_completion, left_outputs);
                let right_assignment = active_completion_assignment(right_completion, right_outputs);
                let Some(assignment) = Self::merge_projection_assignments(&left_assignment, &right_assignment) else {
                    continue;
                };
                let mut evidence = left_completion.evidence.clone();
                evidence.extend(right_completion.evidence.iter().cloned());
                evidence.sort();
                evidence.dedup();
                if !refresh_completion_evidence_routes(&mut evidence, &active_outputs) {
                    continue;
                }
                evidence.sort();
                evidence.dedup();
                completions.insert(Completion { assignment, evidence });
            }
        }
        CompletionResult { question: question.clone(), completions: completions.into_iter().collect() }
    }

    fn shared_clause_percepts(&self, clauses: &[ConceptId]) -> BTreeSet<ConceptId> {
        let mut counts = BTreeMap::new();
        for clause in clauses {
            let mut percepts = BTreeSet::new();
            self.collect_output_percepts(clause, &mut percepts);
            for percept in percepts {
                *counts.entry(percept).or_insert(0_usize) += 1;
            }
        }
        counts.into_iter().filter_map(|(percept, count)| (count > 1).then_some(percept)).collect()
    }

    /// Instantiates any ordinary Concept template from one correlated completion.
    pub fn instantiate_completion(&mut self, template: &ConceptId, completion: &Completion) -> Option<ConceptId> {
        if !self.owns(template) || completion.assignment.iter().any(|(percept, candidate)| !self.owns(percept) || !self.owns(candidate)) {
            return None;
        }
        self.instantiate_completion_inner(template, &completion.assignment)
    }

    pub(super) fn completion_projection_witnesses(&mut self, result: &CompletionResult, template: &ConceptId) -> Option<CompletionProjectionWitnesses> {
        let mut outputs = BTreeSet::new();
        self.collect_output_percepts(template, &mut outputs);
        let mut witnesses = CompletionProjectionWitnesses::new();
        for completion in &result.completions {
            let candidate = self.instantiate_completion_inner(template, &completion.assignment)?;
            let candidate_witnesses = witnesses.entry(candidate).or_default();
            candidate_witnesses.extend(
                completion
                    .evidence
                    .iter()
                    .filter(|evidence| {
                        outputs.iter().any(|output| {
                            let Some(binding) = evidence.assignment.get(output) else {
                                return false;
                            };
                            completion.assignment.get(output) == Some(binding)
                        })
                    })
                    .map(|evidence| evidence.source_view.source.clone()),
            );
        }
        Some(witnesses)
    }

    pub(super) fn materialize_completion_rows(&mut self, result: &CompletionResult) -> Option<ConceptId> {
        self.materialize_completion_rows_for(result, &result.question)
    }

    pub(super) fn materialize_completion_rows_for(&mut self, result: &CompletionResult, template: &ConceptId) -> Option<ConceptId> {
        let mut rows = ConceptMap::new();
        for completion in &result.completions {
            let row = self.instantiate_completion_inner(template, &completion.assignment)?;
            self.add_relevance(&mut rows, row, false, Relevance::DEFAULT)?;
        }
        self.reference_map(&rows)
    }

    pub(super) fn materialize_completion_projection(&mut self, result: &CompletionResult, template: &ConceptId) -> Option<ConceptId> {
        self.try_materialize_completion_projection(result, template).flatten()
    }

    pub(super) fn try_materialize_completion_projection(&mut self, result: &CompletionResult, template: &ConceptId) -> Option<Option<ConceptId>> {
        let witnesses = self.completion_projection_witnesses(result, template)?;
        let mut candidates = ConceptMap::new();
        for (candidate, sources) in witnesses {
            // The current integer rule adds distinct source witnesses. Keeping
            // those witnesses in the answer state leaves room for a different
            // Relevance combination rule later.
            let support = self.question_source_support(&sources)?;
            self.add_relevance(&mut candidates, candidate, false, support)?;
        }
        Some(self.reference_map(&candidates))
    }

    fn source_view_completions(&mut self, source: &ConceptId, question: &ConceptId) -> BTreeSet<StructuralCompletion> {
        let mut ordered_path = Vec::new();
        self.structural_completions(source, question, &mut ordered_path)
    }

    fn structural_completions(&mut self, source: &ConceptId, question: &ConceptId, ordered_path: &mut Vec<usize>) -> BTreeSet<StructuralCompletion> {
        if self.is_percept(question) {
            return BTreeSet::from([StructuralCompletion {
                assignment: ProjectionAssignment::from([(question.clone(), source.clone())]),
                binding_paths: BTreeMap::from([(question.clone(), BTreeSet::from([ordered_path.clone()]))]),
                remainders: BTreeSet::new(),
            }]);
        }
        if source == question {
            return exact_structural_completion();
        }

        // A coefficient-bearing Concept can contain an ordinary structural
        // question, but the wrapper itself must match exactly. Do not turn a
        // coefficient difference into a remainder, occurrence count, or score.
        if let (Some((source_relevance, source_operand)), Some((question_relevance, question_operand))) =
            (source.0.coefficient_operand(), question.0.coefficient_operand())
        {
            if source_relevance != question_relevance {
                return BTreeSet::new();
            }
            return self.structural_completions(source_operand, question_operand, ordered_path);
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
                    insert_structural_completion(&mut completions, completion);
                } else if question_outputs.len() == 1 {
                    let Some(remainder) = self.reference_completion_members(&remaining) else {
                        continue;
                    };
                    let output = &question_outputs[0];
                    let output_assignment = ProjectionAssignment::from([(output.clone(), remainder)]);
                    let Some(assignment) = Self::merge_projection_assignments(&injection.completion.assignment, &output_assignment) else {
                        continue;
                    };
                    let mut binding_paths = injection.completion.binding_paths;
                    binding_paths.entry(output.clone()).or_default().insert(ordered_path.to_vec());
                    insert_structural_completion(
                        &mut completions,
                        StructuralCompletion { assignment, binding_paths, remainders: injection.completion.remainders },
                    );
                } else if remaining.len() == question_outputs.len() {
                    for outputs in self.inject_members(&question_outputs, &remaining, true, ordered_path) {
                        let Some(assignment) = Self::merge_projection_assignments(&injection.completion.assignment, &outputs.completion.assignment) else {
                            continue;
                        };
                        let binding_paths = merge_binding_paths(&injection.completion.binding_paths, &outputs.completion.binding_paths);
                        let mut remainders = injection.completion.remainders.clone();
                        remainders.extend(outputs.completion.remainders);
                        insert_structural_completion(&mut completions, StructuralCompletion { assignment, binding_paths, remainders });
                    }
                }
            }
        }

        if question_outputs.is_empty() && source_members.len() <= question_fixed.len() {
            for injection in self.inject_members(&source_members, &question_fixed, false, ordered_path) {
                let remaining = unused_members(&question_fixed, &injection.used_candidates);
                let mut completion = injection.completion;
                self.add_completion_remainder(&mut completion, CompletionRemainderSide::Question, ordered_path, &remaining);
                insert_structural_completion(&mut completions, completion);
            }
        }

        completions
    }

    fn inject_members(
        &mut self,
        patterns: &[ConceptId],
        candidates: &[ConceptId],
        patterns_are_questions: bool,
        ordered_path: &[usize],
    ) -> BTreeSet<Injection> {
        let mut injections = BTreeSet::from([Injection {
            completion: StructuralCompletion { assignment: ProjectionAssignment::new(), binding_paths: BTreeMap::new(), remainders: BTreeSet::new() },
            used_candidates: BTreeSet::new(),
        }]);
        for pattern in patterns {
            let mut next = BTreeSet::new();
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
                        let binding_paths = merge_binding_paths(&injection.completion.binding_paths, &matched.binding_paths);
                        let mut remainders = injection.completion.remainders.clone();
                        remainders.extend(matched.remainders);
                        let mut used_candidates = injection.used_candidates.clone();
                        used_candidates.insert(candidate_index);
                        insert_injection(&mut next, Injection { completion: StructuralCompletion { assignment, binding_paths, remainders }, used_candidates });
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

fn active_completion_assignment(completion: &Completion, outputs: &BTreeSet<ConceptId>) -> ProjectionAssignment {
    completion.assignment.iter().filter(|(percept, _)| outputs.contains(*percept)).map(|(percept, value)| (percept.clone(), value.clone())).collect()
}

fn refresh_completion_evidence_routes(evidence: &mut [CompletionEvidence], active_outputs: &BTreeSet<ConceptId>) -> bool {
    let mut percept_counts = BTreeMap::new();
    for fragment in evidence.iter() {
        for percept in fragment.assignment.keys().filter(|percept| active_outputs.contains(*percept)) {
            *percept_counts.entry(percept.clone()).or_insert(0_usize) += 1;
        }
    }
    let shared_percepts = percept_counts.into_iter().filter_map(|(percept, count)| (count > 1).then_some(percept)).collect::<BTreeSet<_>>();
    let sources = evidence.iter().map(|fragment| fragment.source_view.source.clone()).collect::<BTreeSet<_>>();

    for source in sources {
        let mut source_route_products = BTreeSet::from([CompletionRoute::default()]);
        for fragment in evidence.iter().filter(|fragment| fragment.source_view.source == source) {
            let constraints = source_route_constraints(&fragment.source_view.routes, &shared_percepts);
            source_route_products = join_source_route_relations(&source_route_products, &constraints);
        }
        if source_route_products.is_empty() {
            return false;
        }
        for fragment in evidence.iter_mut().filter(|fragment| fragment.source_view.source == source) {
            fragment.source_route_products = source_route_products.clone();
        }
    }
    true
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

fn join_source_route_relations(left: &BTreeSet<CompletionRoute>, right: &BTreeSet<CompletionRoute>) -> BTreeSet<CompletionRoute> {
    let mut products = BTreeSet::new();
    for left in left {
        for right in right {
            if left.selected_entries.iter().any(|(container, entry)| right.selected_entries.get(container).is_some_and(|other| other != entry)) {
                continue;
            }
            let mut product = left.clone();
            let mut compatible = true;
            for (percept, right_origins) in &right.binding_origins {
                if let Some(left_origins) = product.binding_origins.get_mut(percept) {
                    left_origins.retain(|origin| right_origins.contains(origin));
                    if left_origins.is_empty() {
                        compatible = false;
                        break;
                    }
                } else {
                    product.binding_origins.insert(percept.clone(), right_origins.clone());
                }
            }
            if !compatible {
                continue;
            }
            product.coefficient_ancestors.extend(right.coefficient_ancestors.iter().cloned());
            product.selected_entries.extend(right.selected_entries.iter().map(|(container, entry)| (container.clone(), entry.clone())));
            product.ordered_windows.extend(right.ordered_windows.iter().cloned());
            products.insert(product);
        }
    }
    products
}

fn exact_structural_completion() -> BTreeSet<StructuralCompletion> {
    BTreeSet::from([StructuralCompletion { assignment: ProjectionAssignment::new(), binding_paths: BTreeMap::new(), remainders: BTreeSet::new() }])
}

fn multiply_structural_completions(left: &BTreeSet<StructuralCompletion>, right: &BTreeSet<StructuralCompletion>) -> BTreeSet<StructuralCompletion> {
    let mut products = BTreeSet::new();
    for left in left {
        for right in right {
            let Some(assignment) = Pangine::merge_projection_assignments(&left.assignment, &right.assignment) else {
                continue;
            };
            let binding_paths = merge_binding_paths(&left.binding_paths, &right.binding_paths);
            let mut remainders = left.remainders.clone();
            remainders.extend(right.remainders.iter().cloned());
            insert_structural_completion(&mut products, StructuralCompletion { assignment, binding_paths, remainders });
        }
    }
    products
}

fn merge_binding_paths(
    left: &BTreeMap<ConceptId, BTreeSet<Vec<usize>>>,
    right: &BTreeMap<ConceptId, BTreeSet<Vec<usize>>>,
) -> BTreeMap<ConceptId, BTreeSet<Vec<usize>>> {
    let mut merged = left.clone();
    for (percept, paths) in right {
        merged.entry(percept.clone()).or_default().extend(paths.iter().cloned());
    }
    merged
}

fn insert_structural_completion(completions: &mut BTreeSet<StructuralCompletion>, mut completion: StructuralCompletion) {
    if let Some(previous) = completions.take(&completion) {
        completion.binding_paths = merge_binding_paths(&previous.binding_paths, &completion.binding_paths);
    }
    completions.insert(completion);
}

fn insert_injection(injections: &mut BTreeSet<Injection>, mut injection: Injection) {
    if let Some(previous) = injections.take(&injection) {
        injection.completion.binding_paths = merge_binding_paths(&previous.completion.binding_paths, &injection.completion.binding_paths);
    }
    injections.insert(injection);
}

fn routes_with_binding_origins(routes: &BTreeSet<CompletionRoute>, binding_paths: &BTreeMap<ConceptId, BTreeSet<Vec<usize>>>) -> BTreeSet<CompletionRoute> {
    routes
        .iter()
        .map(|route| {
            let mut route = route.clone();
            for window in route.ordered_windows.clone() {
                for (percept, paths) in binding_paths {
                    for path in paths {
                        let (span_start, span_width, nested_path) = match path.split_first() {
                            Some((position, nested_path)) => (window.start + position, 1, nested_path.to_vec()),
                            None => (window.start, window.width, Vec::new()),
                        };
                        route.binding_origins.entry(percept.clone()).or_default().insert(CompletionBindingOrigin {
                            parent: window.parent.clone(),
                            parent_occurrence: window.parent_occurrence.clone(),
                            span_start,
                            span_width,
                            nested_path,
                        });
                    }
                }
            }
            route
        })
        .collect()
}

fn source_route_constraints(routes: &BTreeSet<CompletionRoute>, shared_percepts: &BTreeSet<ConceptId>) -> BTreeSet<CompletionRoute> {
    routes
        .iter()
        .map(|route| CompletionRoute {
            selected_entries: route.selected_entries.clone(),
            binding_origins: route
                .binding_origins
                .iter()
                .filter(|(percept, _)| shared_percepts.contains(*percept))
                .map(|(percept, origins)| (percept.clone(), origins.clone()))
                .collect(),
            ..CompletionRoute::default()
        })
        .collect()
}

fn unused_members(members: &[ConceptId], used: &BTreeSet<usize>) -> Vec<ConceptId> {
    members.iter().enumerate().filter(|(index, _)| !used.contains(index)).map(|(_, concept)| concept.clone()).collect()
}

#[cfg(test)]
mod route_tests {
    use super::*;

    #[test]
    fn route_relation_join_requires_one_globally_consistent_selection() {
        let mut pangine = Pangine::new();
        let container = pangine.reference_concept("[container]").unwrap().unwrap();
        let a = pangine.reference_concept("[A]").unwrap().unwrap();
        let b = pangine.reference_concept("[B]").unwrap().unwrap();
        let c = pangine.reference_concept("[C]").unwrap().unwrap();
        let relation = |entries: &[ConceptId]| {
            entries
                .iter()
                .map(|entry| CompletionRoute { selected_entries: BTreeMap::from([(container.clone(), entry.clone())]), ..CompletionRoute::default() })
                .collect::<BTreeSet<_>>()
        };
        let ab = relation(&[a.clone(), b.clone()]);
        let bc = relation(&[b.clone(), c.clone()]);
        let ac = relation(&[a, c]);

        assert!(!join_source_route_relations(&ab, &bc).is_empty());
        assert!(!join_source_route_relations(&bc, &ac).is_empty());
        assert!(!join_source_route_relations(&ab, &ac).is_empty());
        let first_two = join_source_route_relations(&ab, &bc);
        assert!(join_source_route_relations(&first_two, &ac).is_empty(), "pairwise-compatible fragments need not have one global source route");
    }

    #[test]
    fn singleton_route_relation_join_is_associative_commutative_and_idempotent() {
        let mut pangine = Pangine::new();
        let container = pangine.reference_concept("[container]").unwrap().unwrap();
        let entry = pangine.reference_concept("[entry]").unwrap().unwrap();
        let left_coefficient = pangine.reference_concept("x2[left]").unwrap().unwrap();
        let right_coefficient = pangine.reference_concept("x3[right]").unwrap().unwrap();
        let parent = pangine.reference_concept("[A]->[r]->[B]->[s]->[C]").unwrap().unwrap();
        let left = BTreeSet::from([CompletionRoute {
            coefficient_ancestors: BTreeSet::from([left_coefficient]),
            selected_entries: BTreeMap::from([(container.clone(), entry.clone())]),
            ordered_windows: BTreeSet::from([CompletionOrderedWindow { parent: parent.clone(), parent_occurrence: Vec::new(), start: 0, width: 3 }]),
            binding_origins: BTreeMap::new(),
        }]);
        let right = BTreeSet::from([CompletionRoute {
            coefficient_ancestors: BTreeSet::from([right_coefficient]),
            selected_entries: BTreeMap::from([(container, entry)]),
            ordered_windows: BTreeSet::from([CompletionOrderedWindow { parent, parent_occurrence: Vec::new(), start: 2, width: 3 }]),
            binding_origins: BTreeMap::new(),
        }]);

        let forward = join_source_route_relations(&left, &right);
        assert_eq!(forward, join_source_route_relations(&right, &left));
        assert_eq!(join_source_route_relations(&forward, &left), forward, "a singleton constraint relation is idempotent");
        assert_eq!(
            join_source_route_relations(&join_source_route_relations(&left, &right), &left),
            join_source_route_relations(&left, &join_source_route_relations(&right, &left))
        );
    }

    #[test]
    fn binding_origin_join_requires_one_globally_shared_occurrence() {
        let mut pangine = Pangine::new();
        let percept = pangine.reference_percept("shared-origin");
        let parent = pangine.reference_concept("[A]->[r]->[B]->[s]->[C]").unwrap().unwrap();
        let origin =
            |span_start| CompletionBindingOrigin { parent: parent.clone(), parent_occurrence: Vec::new(), span_start, span_width: 1, nested_path: Vec::new() };
        let relation = |positions: &[usize]| {
            BTreeSet::from([CompletionRoute {
                binding_origins: BTreeMap::from([(percept.clone(), positions.iter().copied().map(origin).collect())]),
                ..CompletionRoute::default()
            }])
        };
        let ab = relation(&[0, 1]);
        let bc = relation(&[1, 2]);
        let ac = relation(&[0, 2]);

        assert!(!join_source_route_relations(&ab, &bc).is_empty());
        assert!(!join_source_route_relations(&bc, &ac).is_empty());
        assert!(!join_source_route_relations(&ab, &ac).is_empty());
        assert!(join_source_route_relations(&join_source_route_relations(&ab, &bc), &ac).is_empty());
        assert!(join_source_route_relations(&ab, &join_source_route_relations(&bc, &ac)).is_empty());
    }
}
