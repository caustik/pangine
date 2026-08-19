//! Retains a complete answer as one ordinary Concept.
//!
//! The structs in this file are temporary decoded views. The retained value,
//! transport form, and result of every operation are ordinary Concepts. This
//! preserves collapse, adjustment, extension, and reduction without installing
//! a second kind of state behind any Percept.

use super::{
    completion::CompletionEvidenceParts, Completion, CompletionBindingOrigin, CompletionEvidence, CompletionOrderedStep, CompletionOrderedWindow,
    CompletionRemainder, CompletionRemainderSide, CompletionResult, CompletionRoute, ConceptId, ConceptKind, Pangine, ProjectionAssignment, QuestionSource,
};
use crate::Relevance;
use std::collections::{BTreeMap, BTreeSet};

const ANSWER: &str = "pangine-answer-v1";
const QUESTIONS: &str = "pangine-answer-questions";
const OUTPUTS: &str = "pangine-answer-outputs";
const ROWS: &str = "pangine-answer-rows";
const ROW: &str = "pangine-answer-row";
const BINDINGS: &str = "pangine-answer-bindings";
const BINDING: &str = "pangine-answer-binding";
const EVIDENCE_SET: &str = "pangine-answer-evidence-set";
const EVIDENCE: &str = "pangine-answer-evidence";
const SOURCE: &str = "pangine-answer-source";
const PERCEPT_SOURCE: &str = "pangine-answer-percept-source";
const SUBJECT_SOURCE: &str = "pangine-answer-subject-source";
const ROUTES: &str = "pangine-answer-routes";
const SOURCE_ROUTE_PRODUCTS: &str = "pangine-answer-source-route-products";
const ROUTE: &str = "pangine-answer-route";
const COEFFICIENT_ANCESTORS: &str = "pangine-answer-coefficient-ancestors";
const SELECTED_ENTRIES: &str = "pangine-answer-selected-entries";
const SELECTED_ENTRY: &str = "pangine-answer-selected-entry";
const ORDERED_WINDOWS: &str = "pangine-answer-ordered-windows";
const ORDERED_WINDOW: &str = "pangine-answer-ordered-window";
const ORDERED_STEPS: &str = "pangine-answer-ordered-steps";
const ORDERED_STEP: &str = "pangine-answer-ordered-step";
const BINDING_ORIGIN_ENTRIES: &str = "pangine-answer-binding-origin-entries";
const BINDING_ORIGIN_ENTRY: &str = "pangine-answer-binding-origin-entry";
const BINDING_ORIGINS: &str = "pangine-answer-binding-origins";
const BINDING_ORIGIN: &str = "pangine-answer-binding-origin";
const INDEX_PATH: &str = "pangine-answer-index-path";
const REMAINDERS: &str = "pangine-answer-remainders";
const REMAINDER: &str = "pangine-answer-remainder";
const SOURCE_REMAINDER: &str = "pangine-answer-source-remainder";
const QUESTION_REMAINDER: &str = "pangine-answer-question-remainder";
const ADJUSTED_OUTPUTS: &str = "pangine-answer-adjusted-outputs";
const VERSIONED_ANSWER: &str = "pangine-versioned-answer";
const LIVE_PROJECTIONS: &str = "pangine-answer-live-projections";
const LIVE_PROJECTION: &str = "pangine-answer-live-projection";
const LIVE_VALUE: &str = "pangine-answer-live-value";
const LIVE_NULL: &str = "pangine-answer-live-null";
const SIGNED_PREFIX: &str = "pangine-answer-i64-";
const UNSIGNED_PREFIX: &str = "pangine-answer-usize-";

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ConceptAnswer {
    pub(super) questions: BTreeSet<ConceptId>,
    pub(super) outputs: BTreeSet<ConceptId>,
    rows: Vec<Completion>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct LiveConceptAnswer {
    pub(super) revision: usize,
    pub(super) answer: ConceptAnswer,
    projections: BTreeMap<ConceptId, Option<ConceptId>>,
}

impl LiveConceptAnswer {
    pub(super) fn new(pangine: &mut Pangine, revision: usize, answer: ConceptAnswer) -> Option<Self> {
        let result = answer.to_result(pangine)?;
        let projections = answer
            .outputs
            .iter()
            .map(|output| pangine.try_materialize_completion_projection(&result, output).map(|projection| (output.clone(), projection)))
            .collect::<Option<BTreeMap<_, _>>>()?;
        Some(Self { revision, answer, projections })
    }

    pub(super) fn successor(pangine: &mut Pangine, revision: usize, answer: ConceptAnswer) -> Option<Self> {
        Self::new(pangine, revision.checked_add(1)?, answer)
    }

    pub(super) fn encode(&self, pangine: &mut Pangine) -> ConceptId {
        let revision = encode_unsigned(pangine, self.revision);
        let answer = self.answer.encode(pangine);
        let projections = self
            .projections
            .iter()
            .map(|(output, value)| {
                let value = match value {
                    Some(value) => tagged(pangine, LIVE_VALUE, vec![value.clone()]),
                    None => tagged(pangine, LIVE_NULL, Vec::new()),
                };
                tagged(pangine, LIVE_PROJECTION, vec![output.clone(), value])
            })
            .collect::<Vec<_>>();
        let projections = encode_concept_set(pangine, LIVE_PROJECTIONS, projections);
        tagged(pangine, VERSIONED_ANSWER, vec![revision, answer, projections])
    }

    pub(super) fn decode(pangine: &Pangine, concept: &ConceptId) -> Option<Self> {
        let [revision, answer, projections] = fixed_fields(pangine, concept, VERSIONED_ANSWER)?;
        let answer = ConceptAnswer::decode(pangine, answer)?;
        let projections = tagged_fields(pangine, projections, LIVE_PROJECTIONS)?
            .iter()
            .map(|projection| {
                let [output, value] = fixed_fields(pangine, projection, LIVE_PROJECTION)?;
                if !pangine.is_percept(output) {
                    return None;
                }
                let value = if let Some([value]) = fixed_fields(pangine, value, LIVE_VALUE) {
                    Some(value.clone())
                } else if tagged_fields(pangine, value, LIVE_NULL)?.is_empty() {
                    None
                } else {
                    return None;
                };
                Some((output.clone(), value))
            })
            .collect::<Option<BTreeMap<_, _>>>()?;
        if projections.keys().cloned().collect::<BTreeSet<_>>() != answer.outputs {
            return None;
        }
        Some(Self { revision: decode_unsigned(pangine, revision)?, answer, projections })
    }

    pub(super) fn decode_validated(pangine: &mut Pangine, concept: &ConceptId) -> Option<Self> {
        let live = Self::decode(pangine, concept)?;
        (Self::new(pangine, live.revision, live.answer.clone())? == live).then_some(live)
    }

    pub(super) fn projection(&self, output: &ConceptId) -> Option<Option<ConceptId>> {
        self.projections.get(output).cloned()
    }
}

impl ConceptAnswer {
    pub(super) fn from_result(pangine: &Pangine, result: &CompletionResult) -> Self {
        let mut outputs = BTreeSet::new();
        pangine.collect_output_percepts(result.question(), &mut outputs);
        let rows = result.completions().to_vec();

        Self { questions: BTreeSet::from([result.question().clone()]), outputs, rows }
    }

    pub(super) fn to_result(&self, pangine: &mut Pangine) -> Option<CompletionResult> {
        let question = self.shape(pangine)?;
        Some(CompletionResult::from_parts(question, self.rows.clone()))
    }

    pub(super) fn encode(&self, pangine: &mut Pangine) -> ConceptId {
        let questions = encode_concept_set(pangine, QUESTIONS, self.questions.iter().cloned());
        let outputs = encode_concept_set(pangine, OUTPUTS, self.outputs.iter().cloned());
        let rows = self.rows.iter().map(|row| encode_completion(pangine, row)).collect::<Vec<_>>();
        let rows = encode_concept_set(pangine, ROWS, rows);
        tagged(pangine, ANSWER, vec![questions, outputs, rows])
    }

    pub(super) fn decode(pangine: &Pangine, concept: &ConceptId) -> Option<Self> {
        let [questions, outputs, rows] = fixed_fields(pangine, concept, ANSWER)?;
        let questions = decode_concept_set(pangine, questions, QUESTIONS)?;
        let outputs = decode_concept_set(pangine, outputs, OUTPUTS)?;
        if outputs.iter().any(|output| !pangine.is_percept(output)) {
            return None;
        }
        let mut rows = tagged_fields(pangine, rows, ROWS)?.iter().map(|row| decode_completion(pangine, row)).collect::<Option<Vec<_>>>()?;
        rows.sort();
        rows.dedup();
        Some(Self { questions, outputs, rows })
    }

    pub(super) fn shape(&self, pangine: &mut Pangine) -> Option<ConceptId> {
        pangine.answer_shape(&self.visible_components(pangine))
    }

    pub(super) fn visible_components(&self, pangine: &Pangine) -> BTreeSet<ConceptId> {
        self.visible_components_for_outputs(pangine, &self.outputs)
    }

    fn visible_components_for_outputs(&self, pangine: &Pangine, outputs: &BTreeSet<ConceptId>) -> BTreeSet<ConceptId> {
        let mut components = BTreeSet::new();
        let mut represented_outputs = BTreeSet::new();
        for question in &self.questions {
            let mut question_outputs = BTreeSet::new();
            pangine.collect_output_percepts(question, &mut question_outputs);
            if !question_outputs.is_empty() && question_outputs.is_subset(outputs) {
                components.insert(question.clone());
                represented_outputs.extend(question_outputs);
            }
        }
        components.extend(outputs.difference(&represented_outputs).cloned());
        components
    }

    pub(super) fn materialize(&self, pangine: &mut Pangine, template: &ConceptId) -> Option<ConceptId> {
        self.projected_outputs(pangine, template)?;
        let result = self.to_result(pangine)?;
        pangine.materialize_completion_projection(&result, template)
    }

    #[cfg(test)]
    pub(super) fn materialize_rows(&self, pangine: &mut Pangine) -> Option<ConceptId> {
        let result = self.to_result(pangine)?;
        pangine.materialize_completion_rows(&result)
    }

    pub(super) fn choose(&self, pangine: &mut Pangine, template: &ConceptId) -> Option<(ConceptId, Self)> {
        self.projected_outputs(pangine, template)?;
        let result = self.to_result(pangine)?;
        let (selected, result) = pangine.choose_completion_result(&result, template)?;
        let mut answer = Self::from_result(pangine, &result);
        answer.questions = self.questions.clone();
        answer.outputs = self.outputs.clone();
        Some((selected, answer))
    }

    #[cfg(test)]
    pub(super) fn adjust(
        &self,
        pangine: &mut Pangine,
        template: &ConceptId,
        adjustment: &Self,
        adjustment_template: &ConceptId,
        factor: Relevance,
    ) -> Option<Self> {
        self.projected_outputs(pangine, template)?;
        adjustment.projected_outputs(pangine, adjustment_template)?;
        let target = self.to_result(pangine)?;
        let adjustment = adjustment.to_result(pangine)?;
        let result = pangine.adjust_completion_result(&target, template, &adjustment, adjustment_template, &self.outputs, factor)?;
        let mut answer = Self::from_result(pangine, &result);
        answer.questions = self.questions.clone();
        answer.outputs = self.outputs.clone();
        Some(answer)
    }

    #[cfg(test)]
    pub(super) fn join(&self, pangine: &mut Pangine, other: &Self) -> Option<Self> {
        if self.outputs.is_disjoint(&other.outputs) {
            return None;
        }

        let outputs = self.outputs.union(&other.outputs).cloned().collect::<BTreeSet<_>>();
        let questions = self.questions.union(&other.questions).cloned().collect();
        let mut answer = Self { questions, outputs, rows: Vec::new() };
        let question = answer.shape(pangine)?;
        let left = self.to_result(pangine)?;
        let right = other.to_result(pangine)?;
        let result = pangine.join_completion_results(&left, &self.outputs, &right, &other.outputs, &question);
        if result.completions().is_empty() {
            return None;
        }
        answer.rows = Self::from_result(pangine, &result).rows;
        Some(answer)
    }

    #[cfg(test)]
    fn merge_partitions(&self, other: &Self) -> Option<Self> {
        if self.questions != other.questions || self.outputs != other.outputs {
            return None;
        }
        let rows = self.rows.iter().chain(&other.rows).cloned().collect::<BTreeSet<_>>().into_iter().collect();
        Some(Self { questions: self.questions.clone(), outputs: self.outputs.clone(), rows })
    }

    pub(super) fn detach(&self, pangine: &Pangine, output: &ConceptId) -> Option<Self> {
        let mut outputs = self.outputs.clone();
        outputs.remove(output);
        if outputs.is_empty() {
            return None;
        }

        let questions = self.visible_components_for_outputs(pangine, &outputs);
        Some(Self { questions, outputs, rows: self.rows.clone() })
    }

    fn projected_outputs(&self, pangine: &Pangine, template: &ConceptId) -> Option<BTreeSet<ConceptId>> {
        if !pangine.owns(template) {
            return None;
        }
        let mut projected = BTreeSet::new();
        pangine.collect_output_percepts(template, &mut projected);
        (!projected.is_empty() && projected.is_subset(&self.outputs)).then_some(projected)
    }
}

fn encode_completion(pangine: &mut Pangine, completion: &Completion) -> ConceptId {
    let bindings = encode_bindings(pangine, BINDINGS, completion.bindings());
    let evidence = completion.evidence().iter().map(|evidence| encode_evidence(pangine, evidence)).collect::<Vec<_>>();
    let evidence = encode_concept_set(pangine, EVIDENCE_SET, evidence);
    tagged(pangine, ROW, vec![bindings, evidence])
}

fn decode_completion(pangine: &Pangine, concept: &ConceptId) -> Option<Completion> {
    let [bindings, evidence] = fixed_fields(pangine, concept, ROW)?;
    let bindings = decode_bindings(pangine, bindings, BINDINGS)?;
    let mut evidence = tagged_fields(pangine, evidence, EVIDENCE_SET)?.iter().map(|evidence| decode_evidence(pangine, evidence)).collect::<Option<Vec<_>>>()?;
    evidence.sort();
    evidence.dedup();
    Some(Completion::from_parts(bindings, evidence))
}

fn encode_evidence(pangine: &mut Pangine, evidence: &CompletionEvidence) -> ConceptId {
    let source = encode_source(pangine, evidence);
    let routes = encode_route_set(pangine, ROUTES, evidence.routes());
    let products = encode_route_set(pangine, SOURCE_ROUTE_PRODUCTS, evidence.source_route_products());
    let assignment = encode_bindings(pangine, BINDINGS, evidence.bindings());
    let remainders = evidence.remainders().map(|remainder| encode_remainder(pangine, remainder)).collect::<Vec<_>>();
    let remainders = encode_concept_set(pangine, REMAINDERS, remainders);
    let contribution = encode_signed(pangine, evidence.source_contribution().weight());
    let adjusted_outputs = encode_concept_set(pangine, ADJUSTED_OUTPUTS, evidence.adjusted_outputs().cloned());
    tagged(
        pangine,
        EVIDENCE,
        vec![source, evidence.clause().clone(), evidence.matched().clone(), routes, products, assignment, remainders, contribution, adjusted_outputs],
    )
}

fn decode_evidence(pangine: &Pangine, concept: &ConceptId) -> Option<CompletionEvidence> {
    let [source, clause, matched, routes, products, assignment, remainders, contribution, adjusted_outputs] = fixed_fields(pangine, concept, EVIDENCE)?;
    Some(CompletionEvidence::from_parts(CompletionEvidenceParts {
        source: decode_source(pangine, source)?,
        clause: clause.clone(),
        matched: matched.clone(),
        routes: decode_route_set(pangine, routes, ROUTES)?,
        source_route_products: decode_route_set(pangine, products, SOURCE_ROUTE_PRODUCTS)?,
        assignment: decode_bindings(pangine, assignment, BINDINGS)?,
        remainders: tagged_fields(pangine, remainders, REMAINDERS)?
            .iter()
            .map(|remainder| decode_remainder(pangine, remainder))
            .collect::<Option<BTreeSet<_>>>()?,
        contribution: Relevance::new(decode_signed(pangine, contribution)?),
        adjusted_outputs: decode_concept_set(pangine, adjusted_outputs, ADJUSTED_OUTPUTS)?,
    }))
}

fn encode_source(pangine: &mut Pangine, evidence: &CompletionEvidence) -> ConceptId {
    let origin = evidence
        .source_percept()
        .map(|percept| tagged(pangine, PERCEPT_SOURCE, vec![percept.clone()]))
        .unwrap_or_else(|| tagged(pangine, SUBJECT_SOURCE, Vec::new()));
    let relevance = encode_signed(pangine, evidence.source_relevance().weight());
    tagged(pangine, SOURCE, vec![origin, evidence.source_concept().clone(), relevance])
}

fn decode_source(pangine: &Pangine, concept: &ConceptId) -> Option<QuestionSource> {
    let [origin, source, relevance] = fixed_fields(pangine, concept, SOURCE)?;
    let relevance = Relevance::new(decode_signed(pangine, relevance)?);
    if let Some([percept]) = fixed_fields(pangine, origin, PERCEPT_SOURCE) {
        Some(QuestionSource::from_percept(percept.clone(), source.clone(), relevance))
    } else if tagged_fields(pangine, origin, SUBJECT_SOURCE)?.is_empty() && relevance == Relevance::DEFAULT {
        Some(QuestionSource::from_subject(source.clone()))
    } else {
        None
    }
}

fn encode_remainder(pangine: &mut Pangine, remainder: &CompletionRemainder) -> ConceptId {
    let side = tagged(
        pangine,
        match remainder.side {
            CompletionRemainderSide::Source => SOURCE_REMAINDER,
            CompletionRemainderSide::Question => QUESTION_REMAINDER,
        },
        Vec::new(),
    );
    let path = encode_index_path(pangine, &remainder.ordered_path);
    tagged(pangine, REMAINDER, vec![side, path, remainder.concept.clone()])
}

fn decode_remainder(pangine: &Pangine, concept: &ConceptId) -> Option<CompletionRemainder> {
    let [side, path, concept] = fixed_fields(pangine, concept, REMAINDER)?;
    let side = if tagged_fields(pangine, side, SOURCE_REMAINDER).is_some_and(|fields| fields.is_empty()) {
        CompletionRemainderSide::Source
    } else if tagged_fields(pangine, side, QUESTION_REMAINDER).is_some_and(|fields| fields.is_empty()) {
        CompletionRemainderSide::Question
    } else {
        return None;
    };
    Some(CompletionRemainder { side, ordered_path: decode_index_path(pangine, path)?, concept: concept.clone() })
}

fn tagged(pangine: &mut Pangine, tag: &str, fields: Vec<ConceptId>) -> ConceptId {
    let marker = pangine.reference_named(tag).expect("nonempty answer marker");
    let mut components = Vec::with_capacity(fields.len() + 1);
    components.push(marker);
    components.extend(fields);
    pangine.reference_ordered(components)
}

fn tagged_fields<'a>(pangine: &Pangine, concept: &'a ConceptId, tag: &str) -> Option<&'a [ConceptId]> {
    match &concept.0.kind {
        ConceptKind::Named(name) => (name == tag).then_some(&[]),
        ConceptKind::Ordered { components } => {
            let marker = components.first()?;
            (pangine.get_name(marker) == Some(tag)).then_some(&components[1..])
        }
        ConceptKind::Percept { .. } | ConceptKind::Unordered => None,
    }
}

fn fixed_fields<'a, const SIZE: usize>(pangine: &Pangine, concept: &'a ConceptId, tag: &str) -> Option<&'a [ConceptId; SIZE]> {
    tagged_fields(pangine, concept, tag)?.try_into().ok()
}

fn encode_concept_set(pangine: &mut Pangine, tag: &str, concepts: impl IntoIterator<Item = ConceptId>) -> ConceptId {
    let mut concepts = concepts.into_iter().collect::<Vec<_>>();
    concepts.sort_by(|left, right| pangine.compare_concepts(left, right));
    concepts.dedup();
    tagged(pangine, tag, concepts)
}

fn decode_concept_set(pangine: &Pangine, concept: &ConceptId, tag: &str) -> Option<BTreeSet<ConceptId>> {
    Some(tagged_fields(pangine, concept, tag)?.iter().cloned().collect())
}

fn encode_bindings<'a>(pangine: &mut Pangine, tag: &str, bindings: impl IntoIterator<Item = (&'a ConceptId, &'a ConceptId)>) -> ConceptId {
    let pairs = bindings.into_iter().map(|(percept, value)| tagged(pangine, BINDING, vec![percept.clone(), value.clone()])).collect::<Vec<_>>();
    encode_concept_set(pangine, tag, pairs)
}

fn decode_bindings(pangine: &Pangine, concept: &ConceptId, tag: &str) -> Option<ProjectionAssignment> {
    let mut bindings = ProjectionAssignment::new();
    for pair in tagged_fields(pangine, concept, tag)? {
        let [percept, value] = fixed_fields(pangine, pair, BINDING)?;
        if bindings.insert(percept.clone(), value.clone()).is_some() {
            return None;
        }
    }
    Some(bindings)
}

fn encode_signed(pangine: &mut Pangine, value: i64) -> ConceptId {
    pangine.reference_named(&format!("{SIGNED_PREFIX}{value}")).expect("nonempty signed value")
}

fn decode_signed(pangine: &Pangine, concept: &ConceptId) -> Option<i64> {
    pangine.get_name(concept)?.strip_prefix(SIGNED_PREFIX)?.parse().ok()
}

fn encode_unsigned(pangine: &mut Pangine, value: usize) -> ConceptId {
    pangine.reference_named(&format!("{UNSIGNED_PREFIX}{value}")).expect("nonempty unsigned value")
}

fn decode_unsigned(pangine: &Pangine, concept: &ConceptId) -> Option<usize> {
    pangine.get_name(concept)?.strip_prefix(UNSIGNED_PREFIX)?.parse().ok()
}

fn encode_index_path(pangine: &mut Pangine, path: &[usize]) -> ConceptId {
    let values = path.iter().map(|value| encode_unsigned(pangine, *value)).collect();
    tagged(pangine, INDEX_PATH, values)
}

fn decode_index_path(pangine: &Pangine, concept: &ConceptId) -> Option<Vec<usize>> {
    tagged_fields(pangine, concept, INDEX_PATH)?.iter().map(|value| decode_unsigned(pangine, value)).collect()
}

fn encode_route_set<'a>(pangine: &mut Pangine, tag: &str, routes: impl IntoIterator<Item = &'a CompletionRoute>) -> ConceptId {
    let routes = routes.into_iter().map(|route| encode_route(pangine, route)).collect::<Vec<_>>();
    encode_concept_set(pangine, tag, routes)
}

fn decode_route_set(pangine: &Pangine, concept: &ConceptId, tag: &str) -> Option<BTreeSet<CompletionRoute>> {
    tagged_fields(pangine, concept, tag)?.iter().map(|route| decode_route(pangine, route)).collect()
}

fn encode_route(pangine: &mut Pangine, route: &CompletionRoute) -> ConceptId {
    let coefficients = encode_concept_set(pangine, COEFFICIENT_ANCESTORS, route.coefficient_ancestors.iter().cloned());
    let selected =
        route.selected_entries.iter().map(|(container, entry)| tagged(pangine, SELECTED_ENTRY, vec![container.clone(), entry.clone()])).collect::<Vec<_>>();
    let selected = encode_concept_set(pangine, SELECTED_ENTRIES, selected);
    let windows = route.ordered_windows.iter().map(|window| encode_window(pangine, window)).collect::<Vec<_>>();
    let windows = encode_concept_set(pangine, ORDERED_WINDOWS, windows);
    let origins = route
        .binding_origins
        .iter()
        .map(|(percept, origins)| {
            let origins = origins.iter().map(|origin| encode_origin(pangine, origin)).collect::<Vec<_>>();
            let origins = encode_concept_set(pangine, BINDING_ORIGINS, origins);
            tagged(pangine, BINDING_ORIGIN_ENTRY, vec![percept.clone(), origins])
        })
        .collect::<Vec<_>>();
    let origins = encode_concept_set(pangine, BINDING_ORIGIN_ENTRIES, origins);
    tagged(pangine, ROUTE, vec![coefficients, selected, windows, origins])
}

fn decode_route(pangine: &Pangine, concept: &ConceptId) -> Option<CompletionRoute> {
    let [coefficients, selected, windows, origins] = fixed_fields(pangine, concept, ROUTE)?;
    let mut selected_entries = BTreeMap::new();
    for entry in tagged_fields(pangine, selected, SELECTED_ENTRIES)? {
        let [container, selected] = fixed_fields(pangine, entry, SELECTED_ENTRY)?;
        if selected_entries.insert(container.clone(), selected.clone()).is_some() {
            return None;
        }
    }
    let mut binding_origins = BTreeMap::new();
    for entry in tagged_fields(pangine, origins, BINDING_ORIGIN_ENTRIES)? {
        let [percept, origins] = fixed_fields(pangine, entry, BINDING_ORIGIN_ENTRY)?;
        let origins = tagged_fields(pangine, origins, BINDING_ORIGINS)?.iter().map(|origin| decode_origin(pangine, origin)).collect::<Option<BTreeSet<_>>>()?;
        if binding_origins.insert(percept.clone(), origins).is_some() {
            return None;
        }
    }
    Some(CompletionRoute {
        coefficient_ancestors: decode_concept_set(pangine, coefficients, COEFFICIENT_ANCESTORS)?,
        selected_entries,
        ordered_windows: tagged_fields(pangine, windows, ORDERED_WINDOWS)?
            .iter()
            .map(|window| decode_window(pangine, window))
            .collect::<Option<BTreeSet<_>>>()?,
        binding_origins,
    })
}

fn encode_steps(pangine: &mut Pangine, steps: &[CompletionOrderedStep]) -> ConceptId {
    let steps = steps
        .iter()
        .map(|step| {
            let position = encode_unsigned(pangine, step.position);
            tagged(pangine, ORDERED_STEP, vec![step.parent.clone(), position])
        })
        .collect();
    tagged(pangine, ORDERED_STEPS, steps)
}

fn decode_steps(pangine: &Pangine, concept: &ConceptId) -> Option<Vec<CompletionOrderedStep>> {
    tagged_fields(pangine, concept, ORDERED_STEPS)?
        .iter()
        .map(|step| {
            let [parent, position] = fixed_fields(pangine, step, ORDERED_STEP)?;
            Some(CompletionOrderedStep { parent: parent.clone(), position: decode_unsigned(pangine, position)? })
        })
        .collect()
}

fn encode_window(pangine: &mut Pangine, window: &CompletionOrderedWindow) -> ConceptId {
    let occurrence = encode_steps(pangine, &window.parent_occurrence);
    let start = encode_unsigned(pangine, window.start);
    let width = encode_unsigned(pangine, window.width);
    tagged(pangine, ORDERED_WINDOW, vec![window.parent.clone(), occurrence, start, width])
}

fn decode_window(pangine: &Pangine, concept: &ConceptId) -> Option<CompletionOrderedWindow> {
    let [parent, occurrence, start, width] = fixed_fields(pangine, concept, ORDERED_WINDOW)?;
    Some(CompletionOrderedWindow {
        parent: parent.clone(),
        parent_occurrence: decode_steps(pangine, occurrence)?,
        start: decode_unsigned(pangine, start)?,
        width: decode_unsigned(pangine, width)?,
    })
}

fn encode_origin(pangine: &mut Pangine, origin: &CompletionBindingOrigin) -> ConceptId {
    let occurrence = encode_steps(pangine, &origin.parent_occurrence);
    let start = encode_unsigned(pangine, origin.span_start);
    let width = encode_unsigned(pangine, origin.span_width);
    let nested = encode_index_path(pangine, &origin.nested_path);
    tagged(pangine, BINDING_ORIGIN, vec![origin.parent.clone(), occurrence, start, width, nested])
}

fn decode_origin(pangine: &Pangine, concept: &ConceptId) -> Option<CompletionBindingOrigin> {
    let [parent, occurrence, start, width, nested] = fixed_fields(pangine, concept, BINDING_ORIGIN)?;
    Some(CompletionBindingOrigin {
        parent: parent.clone(),
        parent_occurrence: decode_steps(pangine, occurrence)?,
        span_start: decode_unsigned(pangine, start)?,
        span_width: decode_unsigned(pangine, width)?,
        nested_path: decode_index_path(pangine, nested)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_concept_round_trips_every_retained_proof_field() {
        let mut pangine = Pangine::new();
        let subject = must_ref(&mut pangine, "x2(([left]->([A]->[r]->[M]->[s]->[D]))([right]->([A]->[r]->[M]->[s]->[D])))");
        let question = must_ref(&mut pangine, "(['start']->[r]->['middle'])(['middle']->[s]->['end'])");
        let result = pangine.complete_subject(&subject, &question).expect("valid path question");
        let answer = ConceptAnswer::from_result(&pangine, &result);
        let encoded = answer.encode(&mut pangine);
        let decoded = ConceptAnswer::decode(&pangine, &encoded).expect("ordinary Concept answer");

        assert!(decoded == answer);
        let restored = decoded.to_result(&mut pangine).expect("decoded completion result");
        assert!(ConceptAnswer::from_result(&pangine, &restored) == answer);
        assert_eq!(decoded.encode(&mut pangine), encoded);
        assert!(matches!(pangine.concept_kind(&encoded), Some(ConceptKind::Ordered { .. })));
        assert!(answer
            .rows
            .iter()
            .flat_map(|row| row.evidence())
            .flat_map(|evidence| evidence.routes())
            .any(|route| !route.coefficient_ancestors.is_empty() && !route.ordered_windows.is_empty()));

        let residual_source = must_ref(&mut pangine, "([kettle][empty])->([full]/[empty])");
        let residual_question = must_ref(&mut pangine, "([room][kettle][empty])->['delta']");
        let residual = pangine.complete_subject(&residual_source, &residual_question).expect("valid residual question");
        let residual = ConceptAnswer::from_result(&pangine, &residual);
        let residual_encoded = residual.encode(&mut pangine);
        let residual_decoded = ConceptAnswer::decode(&pangine, &residual_encoded).expect("residual Concept answer");
        assert!(residual_decoded == residual);
        assert!(residual.rows.iter().flat_map(|row| row.evidence()).any(|evidence| evidence.remainders().next().is_some()));
    }

    #[test]
    fn concept_answer_preserves_projection_and_ordered_or_simultaneous_collapse() {
        let mut pangine = weighted_animals();
        let answer = complete_answer(&mut pangine, &["memory"], "['animal']->['food']");
        let animal = pangine.reference_percept("animal");
        let food = pangine.reference_percept("food");
        let pair = must_ref(&mut pangine, "['animal']->['food']");
        let answer_data = ConceptAnswer::decode(&pangine, &answer).expect("Concept answer");

        assert_eq!(answer_data.materialize(&mut pangine, &animal), Some(must_ref(&mut pangine, "x8[cat]x7[dog]")));
        assert_eq!(answer_data.materialize(&mut pangine, &food), Some(must_ref(&mut pangine, "x10[fish]x5[milk]")));
        assert_eq!(answer_data.materialize(&mut pangine, &pair), Some(must_ref(&mut pangine, "x3([cat]->[fish])x5([cat]->[milk])x7([dog]->[fish])")));
        assert_eq!(answer_data.materialize_rows(&mut pangine), Some(must_ref(&mut pangine, "([cat]->[fish])([cat]->[milk])([dog]->[fish])")));

        let (selected_animal, animal_answer) = answer_data.choose(&mut pangine, &animal).expect("animal choice");
        assert_eq!(selected_animal, must_ref(&mut pangine, "[cat]"));
        let (selected_food, _) = animal_answer.choose(&mut pangine, &food).expect("conditioned food choice");
        assert_eq!(selected_food, must_ref(&mut pangine, "[milk]"));

        let (selected_pair, pair_answer) = answer_data.choose(&mut pangine, &pair).expect("simultaneous choice");
        assert_eq!(selected_pair, must_ref(&mut pangine, "[dog]->[fish]"));
        assert_eq!(pair_answer.materialize(&mut pangine, &animal), Some(must_ref(&mut pangine, "x7[dog]")));
        assert_eq!(pair_answer.materialize(&mut pangine, &food), Some(must_ref(&mut pangine, "x7[fish]")));
    }

    #[test]
    fn concept_answer_adjustment_keeps_zero_rows_sources_and_higher_order_composition() {
        let mut pangine = Pangine::new();
        remember(&mut pangine, "candidates", "[choice-a]->[decision]->[A]");
        remember(&mut pangine, "candidates", "[choice-b]->[decision]->[B]");
        remember(&mut pangine, "outcomes", "([episode-a]->[decision]->[A])([episode-a]->[outcome]->[helpful])");
        remember(&mut pangine, "outcomes", "([episode-b]->[decision]->[B])([episode-b]->[outcome]->[helpful])");
        remember(&mut pangine, "reliability", "([review-b]->[episode]->[episode-b])([review-b]->[assessment]->[trusted])");

        let candidates = complete_answer(&mut pangine, &["candidates"], "['candidate']->[decision]->['decision']");
        let outcomes = complete_answer(&mut pangine, &["outcomes"], "(['episode']->[decision]->['episode-decision'])(['episode']->[outcome]->[helpful])");
        let trusted = complete_answer(&mut pangine, &["reliability"], "(['review']->[episode]->['trusted-episode'])(['review']->[assessment]->[trusted])");
        let candidates = ConceptAnswer::decode(&pangine, &candidates).unwrap();
        let outcomes = ConceptAnswer::decode(&pangine, &outcomes).unwrap();
        let trusted = ConceptAnswer::decode(&pangine, &trusted).unwrap();
        let episode = pangine.reference_percept("episode");
        let trusted_episode = pangine.reference_percept("trusted-episode");
        let episode_decision = pangine.reference_percept("episode-decision");
        let decision = pangine.reference_percept("decision");

        let trusted_outcomes = outcomes.adjust(&mut pangine, &episode, &trusted, &trusted_episode, Relevance::DEFAULT).expect("trusted outcomes");
        let adjusted =
            candidates.adjust(&mut pangine, &decision, &trusted_outcomes, &episode_decision, Relevance::DEFAULT).expect("higher-order candidate adjustment");
        assert_eq!(adjusted.materialize(&mut pangine, &decision), Some(must_ref(&mut pangine, "x3[B]x2[A]")));
        assert_eq!(adjusted.choose(&mut pangine, &decision).unwrap().0, must_ref(&mut pangine, "[B]"));

        let cancelled = candidates.adjust(&mut pangine, &decision, &candidates, &decision, Relevance::new(-1)).expect("self cancellation");
        assert_eq!(cancelled.rows.len(), 2);
        for row in &cancelled.rows {
            assert!(row.evidence().iter().any(|evidence| evidence.source_contribution() == Relevance::DEFAULT));
            assert!(row.evidence().iter().any(|evidence| evidence.source_contribution() == Relevance::new(-1)));
            assert!(row.evidence().iter().all(|evidence| pangine.format_concept(evidence.source_subject(), false) == "['candidates']"));
        }
        assert!(cancelled.materialize(&mut pangine, &decision).is_none());
    }

    #[test]
    fn concept_answer_join_retains_source_occurrence_constraints() {
        let mut mapper = Pangine::new();
        let subject = must_ref(&mut mapper, "[A]->[r]->[M]->[s]->[D]->[gap]->[X]->[r]->[M]->[s]->[E]");
        let first = complete_subject_answer(&mut mapper, &subject, "['start']->[r]->['middle']");
        let second = complete_subject_answer(&mut mapper, &subject, "['middle']->[s]->['end']");
        let first = mapper.format_concept(&first, false);
        let second = mapper.format_concept(&second, false);

        let mut pangine = Pangine::new();
        let first = must_ref(&mut pangine, &first);
        let second = must_ref(&mut pangine, &second);
        let first = ConceptAnswer::decode(&pangine, &first).unwrap();
        let second = ConceptAnswer::decode(&pangine, &second).unwrap();
        let joined = first.join(&mut pangine, &second).expect("compatible extension");
        let path = must_ref(&mut pangine, "['start']->['middle']->['end']");

        assert_eq!(joined.materialize(&mut pangine, &path), Some(must_ref(&mut pangine, "([A]->[M]->[D])([X]->[M]->[E])")));
        assert_eq!(joined.rows.len(), 2);
    }

    #[test]
    fn clause_answers_reduce_a_cross_source_graph_join() {
        let mut mapper = Pangine::new();
        remember(&mut mapper, "first-facts", "[Socrates]->[is-a]->[human]");
        remember(&mut mapper, "second-facts", "[human]->[is-a]->[mortal]");
        let first = complete_answer(&mut mapper, &["first-facts"], "[Socrates]->[is-a]->['middle']");
        let second = complete_answer(&mut mapper, &["second-facts"], "['middle']->[is-a]->['conclusion']");
        let first = mapper.format_concept(&first, false);
        let second = mapper.format_concept(&second, false);

        let mut reducer = Pangine::new();
        let first = must_ref(&mut reducer, &first);
        let second = must_ref(&mut reducer, &second);
        let first = ConceptAnswer::decode(&reducer, &first).unwrap();
        let second = ConceptAnswer::decode(&reducer, &second).unwrap();
        let joined = first.join(&mut reducer, &second).expect("shared middle binding");
        let middle_conclusion = must_ref(&mut reducer, "['middle']->['conclusion']");
        let conclusion = reducer.reference_percept("conclusion");

        assert_eq!(joined.materialize(&mut reducer, &middle_conclusion), Some(must_ref(&mut reducer, "x2([human]->[mortal])")));
        assert_eq!(joined.materialize(&mut reducer, &conclusion), Some(must_ref(&mut reducer, "[mortal]")));
        assert_eq!(joined.rows.len(), 1);
    }

    #[test]
    fn connected_answer_joins_are_associative_and_commutative_across_occurrence_constraints() {
        let mut pangine = Pangine::new();
        let subject = must_ref(&mut pangine, "[A]->[r]->[M]->[s]->[N]->[t]->[D]->[gap]->[X]->[r]->[M]->[s]->[O]->[t]->[E]");
        let first = complete_subject_answer(&mut pangine, &subject, "['start']->[r]->['middle']");
        let second = complete_subject_answer(&mut pangine, &subject, "['middle']->[s]->['next']");
        let third = complete_subject_answer(&mut pangine, &subject, "['next']->[t]->['end']");
        let first = ConceptAnswer::decode(&pangine, &first).unwrap();
        let second = ConceptAnswer::decode(&pangine, &second).unwrap();
        let third = ConceptAnswer::decode(&pangine, &third).unwrap();

        let left_pair = first.join(&mut pangine, &second).unwrap();
        let left = left_pair.join(&mut pangine, &third).unwrap();
        let right_pair = second.join(&mut pangine, &third).unwrap();
        let right = first.join(&mut pangine, &right_pair).unwrap();
        let reverse_pair = third.join(&mut pangine, &second).unwrap();
        let reverse = reverse_pair.join(&mut pangine, &first).unwrap();
        let path = must_ref(&mut pangine, "['start']->['middle']->['next']->['end']");

        assert!(left == right);
        assert!(left == reverse);
        assert_eq!(left.rows.len(), 2);
        assert_eq!(left.materialize(&mut pangine, &path), Some(must_ref(&mut pangine, "([A]->[M]->[N]->[D])([X]->[M]->[O]->[E])")));
    }

    #[test]
    fn partitioned_concept_answers_reduce_deterministically_and_cross_an_engine_boundary() {
        let mut mapper = Pangine::new();
        remember(&mut mapper, "part-a", "([episode]->[a])([pair]->[cat]->[fish])");
        remember(&mut mapper, "part-b", "([episode]->[b])([pair]->[cat]->[milk])");
        remember(&mut mapper, "part-c", "([episode]->[c])([pair]->[dog]->[fish])");
        let question = "[pair]->['animal']->['food']";
        let mapped = ["part-a", "part-b", "part-c"].map(|source| complete_answer(&mut mapper, &[source], question));
        let full = complete_answer(&mut mapper, &["part-a", "part-b", "part-c"], question);
        let mapped_text = mapped.map(|answer| mapper.format_concept(&answer, false));
        let full_text = mapper.format_concept(&full, false);

        let mut reducer = Pangine::new();
        let mapped = mapped_text.map(|answer| must_ref(&mut reducer, &answer));
        let full = must_ref(&mut reducer, &full_text);
        let a = ConceptAnswer::decode(&reducer, &mapped[0]).unwrap();
        let b = ConceptAnswer::decode(&reducer, &mapped[1]).unwrap();
        let c = ConceptAnswer::decode(&reducer, &mapped[2]).unwrap();
        let forward = a.merge_partitions(&b).unwrap().merge_partitions(&c).unwrap();
        let reverse = c.merge_partitions(&b).unwrap().merge_partitions(&a).unwrap();
        let full = ConceptAnswer::decode(&reducer, &full).unwrap();

        assert!(forward == reverse);
        assert!(forward == full);
        assert_eq!(forward.encode(&mut reducer), full.encode(&mut reducer));
        let animal = reducer.reference_percept("animal");
        assert_eq!(forward.materialize(&mut reducer, &animal), Some(must_ref(&mut reducer, "x2[cat][dog]")));
    }

    #[test]
    fn reifying_the_answer_does_not_expand_indexed_source_visits() {
        let mut pangine = Pangine::new();
        for index in 0..128 {
            remember(&mut pangine, "memory", &format!("[noise-{index}]->[unrelated]->[value-{index}]"));
        }
        remember(&mut pangine, "memory", "[target]->[property]->[found]");
        let memory = pangine.reference_percept("memory");
        let question = must_ref(&mut pangine, "[target]->[property]->['answer']");

        pangine.question_source_visits = 0;
        let result = pangine.complete_question(&[memory], &question).expect("indexed question");
        assert_eq!(pangine.question_source_visits, 1);
        let answer = ConceptAnswer::from_result(&pangine, &result).encode(&mut pangine);
        let answer = ConceptAnswer::decode(&pangine, &answer).unwrap();
        let output = pangine.reference_percept("answer");
        assert_eq!(answer.materialize(&mut pangine, &output), Some(must_ref(&mut pangine, "[found]")));
    }

    #[test]
    fn detaching_a_projection_is_an_explicit_concept_transformation() {
        let mut pangine = weighted_animals();
        let answer = complete_answer(&mut pangine, &["memory"], "['animal']->['food']");
        let answer = ConceptAnswer::decode(&pangine, &answer).unwrap();
        let animal = pangine.reference_percept("animal");
        let food = pangine.reference_percept("food");
        let detached = answer.detach(&pangine, &animal).expect("remaining food answer");

        assert_eq!(detached.shape(&mut pangine), Some(food.clone()));
        assert_eq!(detached.materialize(&mut pangine, &food), Some(must_ref(&mut pangine, "x10[fish]x5[milk]")));
        assert!(detached.materialize(&mut pangine, &animal).is_none());
    }

    #[test]
    fn a_live_answer_value_round_trips_into_the_same_console_lifecycle_in_another_engine() {
        let mut mapper = weighted_animals();
        must_ref(&mut mapper, "['memory'] @ ['animal']->['food']");
        let animal = mapper.reference_percept("animal");
        let encoded = mapper.linked_answer_value(&animal).expect("ordinary live answer value");
        let mut inconsistent = LiveConceptAnswer::decode(&mapper, &encoded).expect("decoded live answer");
        inconsistent.projections.insert(animal, Some(must_ref(&mut mapper, "[wrong]")));
        let inconsistent = inconsistent.encode(&mut mapper);
        let inconsistent = mapper.format_concept(&inconsistent, false);
        let encoded = mapper.format_concept(&encoded, false);

        let mut pangine = Pangine::new();
        let ordinary = must_ref(&mut pangine, "[ordinary]");
        assert!(!pangine.install_answer_value(&ordinary));
        let inconsistent = must_ref(&mut pangine, &inconsistent);
        assert!(!pangine.install_answer_value(&inconsistent));
        let encoded = must_ref(&mut pangine, &encoded);
        assert!(pangine.install_answer_value(&encoded));

        assert_eq!(must_ref(&mut pangine, "&['animal']"), must_ref(&mut pangine, "['animal']->['food']"));
        assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "x8[cat]x7[dog]"));
        assert_eq!(must_ref(&mut pangine, "^['animal']"), must_ref(&mut pangine, "[cat]"));
        assert_eq!(must_ref(&mut pangine, "$['food']"), must_ref(&mut pangine, "x3[fish]x5[milk]"));
    }

    fn weighted_animals() -> Pangine {
        let mut pangine = Pangine::new();
        for (row, repetitions) in [("[cat]->[fish]", 3), ("[cat]->[milk]", 5), ("[dog]->[fish]", 7)] {
            for _ in 0..repetitions {
                remember(&mut pangine, "memory", row);
            }
        }
        pangine
    }

    fn complete_answer(pangine: &mut Pangine, sources: &[&str], question: &str) -> ConceptId {
        let sources = sources.iter().map(|source| pangine.reference_percept(source)).collect::<Vec<_>>();
        let question = must_ref(pangine, question);
        let result = pangine.complete_question(&sources, &question).expect("valid question");
        ConceptAnswer::from_result(pangine, &result).encode(pangine)
    }

    fn complete_subject_answer(pangine: &mut Pangine, subject: &ConceptId, question: &str) -> ConceptId {
        let question = must_ref(pangine, question);
        let result = pangine.complete_subject(subject, &question).expect("valid subject question");
        ConceptAnswer::from_result(pangine, &result).encode(pangine)
    }

    fn remember(pangine: &mut Pangine, source: &str, concept: &str) {
        must_ref(pangine, &format!("['{source}'] ~= {concept}"));
    }

    fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
        pangine
            .reference_concept(input)
            .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
            .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
    }
}
