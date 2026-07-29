//! Research-only oracle for revision history made entirely from ordinary Concepts.
//!
//! The first fixtures use snapshot labels as observer Concepts, while a layout
//! comparison keeps a distinct asserting source in that position instead.
//! Both are application structures. Audit links are ordinary Observations and
//! are not interpreted by Pangine. Exact
//! complete-root membership, isolated canonical root-text validation, closed
//! payload checks, partition/replay behavior, by-value and indirect selection
//! claims, caller-held roots versus optional state selection, exact and
//! label-only audit relationships, observer-as-identity versus source-observed
//! layouts, ambiguous nested payloads, fuzzy keyed questions, non-default
//! payload edges, repeated payload keys, structural versus occurrence identity,
//! relevance-wrapped mentions, and in-band current-marker comparisons below
//! are disposable test adapters, not accepted production behavior or proposed
//! revision-specific grammar.

use std::collections::BTreeSet;

use pangine::{ConceptId, ConceptKind, Pangine, Relevance};

const AGENT_POLICY_CONTEXT: &str = "({[language]->[rust]}*{[operation]->[test]}*{[platform]->[windows]}*{[repo]->[pangine]}*{[scope]->[full]})";
const AGENT_POLICY_QUESTION: &str = "{({[language]->[rust]}*{[operation]->[test]}*{[platform]->[windows]}*{[repo]->[pangine]}*{[scope]->[full]})->$OUTPUT}";
const AGENT_LINT_RECORD: &str = "?[lint-pangine-full]:{({[language]->[rust]}*{[operation]->[lint]}*{[repo]->[pangine]}*{[scope]->[full]})->[cargo]}";

fn must_reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a concept from {source:?}"))
}

fn experience_state(pangine: &mut Pangine, percept_name: &str, source: &str) -> ConceptId {
    let input = must_reference(pangine, source);
    let percept = pangine.reference_percept(percept_name);
    pangine.perform_experience(&percept, Some(&input)).unwrap()
}

fn observation_records(pangine: &Pangine, state: &ConceptId) -> BTreeSet<ConceptId> {
    pangine.get_observations(state).unwrap().into_iter().collect()
}

fn union_records(pangine: &Pangine, states: &[&ConceptId]) -> BTreeSet<ConceptId> {
    states.iter().flat_map(|state| observation_records(pangine, state)).collect()
}

fn exact_observer_records(pangine: &Pangine, state: &ConceptId, observer: Option<&ConceptId>) -> BTreeSet<ConceptId> {
    pangine.get_observations(state).unwrap().into_iter().filter(|record| pangine.get_observer(record).as_ref() == observer).collect()
}

fn reference_observation_state(pangine: &mut Pangine, observations: &BTreeSet<ConceptId>) -> Option<ConceptId> {
    match observations.len() {
        0 => None,
        1 => observations.first().cloned(),
        _ => {
            let entries = observations.iter().map(|observation| pangine.format_concept(observation, false)).collect::<Vec<_>>().join(", ");
            pangine.reference_concept(&format!("<{entries}>")).unwrap()
        }
    }
}

fn sole_exact_observer_payload(pangine: &Pangine, state: &ConceptId, observer: &ConceptId) -> Option<ConceptId> {
    let selected = exact_observer_records(pangine, state, Some(observer));
    match selected.len() {
        0 => None,
        1 => pangine.get_observation(selected.first().unwrap()),
        count => panic!("expected at most one payload for an immutable snapshot identity, got {count}"),
    }
}

fn exact_observation_record(pangine: &Pangine, state: &ConceptId, expected: &ConceptId) -> Option<ConceptId> {
    pangine.get_observations(state)?.into_iter().find(|record| record == expected)
}

fn exact_observation_payload(pangine: &Pangine, state: &ConceptId, expected: &ConceptId) -> Option<ConceptId> {
    exact_observation_record(pangine, state, expected).and_then(|record| pangine.get_observation(&record))
}

fn contains_percept(pangine: &Pangine, concept: &ConceptId) -> bool {
    fn visit(pangine: &Pangine, concept: &ConceptId, visited: &mut BTreeSet<ConceptId>) -> bool {
        if !visited.insert(concept.clone()) {
            return false;
        }

        match pangine.concept_kind(concept) {
            Some(ConceptKind::Percept { .. }) => true,
            Some(ConceptKind::Named(_)) => false,
            Some(ConceptKind::Correlation { a, b }) => visit(pangine, a, visited) || visit(pangine, b, visited),
            Some(ConceptKind::Observation { observer, observation }) => {
                observer.as_ref().is_some_and(|observer| visit(pangine, observer, visited)) || visit(pangine, observation, visited)
            }
            Some(ConceptKind::Relevance | ConceptKind::ObservationSet) => {
                pangine.get_relevance_map(concept).into_iter().any(|(_, child)| visit(pangine, &child, visited))
            }
            None => false,
        }
    }

    visit(pangine, concept, &mut BTreeSet::new())
}

fn nested_observation_sets(pangine: &Pangine, concept: &ConceptId) -> BTreeSet<ConceptId> {
    fn visit(pangine: &Pangine, concept: &ConceptId, visited: &mut BTreeSet<ConceptId>, found: &mut BTreeSet<ConceptId>) {
        if !visited.insert(concept.clone()) {
            return;
        }

        match pangine.concept_kind(concept) {
            Some(ConceptKind::Named(_) | ConceptKind::Percept { .. }) | None => {}
            Some(ConceptKind::Correlation { a, b }) => {
                visit(pangine, a, visited, found);
                visit(pangine, b, visited, found);
            }
            Some(ConceptKind::Observation { observer, observation }) => {
                if let Some(observer) = observer {
                    visit(pangine, observer, visited, found);
                }
                visit(pangine, observation, visited, found);
            }
            Some(ConceptKind::Relevance) => {
                for (_, child) in pangine.get_relevance_map(concept) {
                    visit(pangine, &child, visited, found);
                }
            }
            Some(ConceptKind::ObservationSet) => {
                found.insert(concept.clone());
                for (_, child) in pangine.get_relevance_map(concept) {
                    visit(pangine, &child, visited, found);
                }
            }
        }
    }

    let mut found = BTreeSet::new();
    visit(pangine, concept, &mut BTreeSet::new(), &mut found);
    found
}

fn default_correlation_targets(pangine: &Pangine, concept: &ConceptId, source: &ConceptId) -> BTreeSet<ConceptId> {
    pangine
        .get_relevance_map(concept)
        .into_iter()
        .filter_map(|(relevance, child)| {
            (relevance == Relevance::DEFAULT && pangine.get_correlation_a(&child).as_ref() == Some(source)).then(|| pangine.get_correlation_b(&child)).flatten()
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct ExactObservationView {
    root: ConceptId,
    observation: ConceptId,
}

#[derive(Debug, PartialEq, Eq)]
enum LocatorSelectionError {
    InvalidSyntax,
    NoConcept,
    NonCanonical,
    NotObservation,
    ContainsPercept,
    InvalidAvailableConcept,
    Unavailable,
}

fn exact_observation_view_from_locator(pangine: &mut Pangine, available: &ConceptId, locator: &str) -> Result<ExactObservationView, LocatorSelectionError> {
    let mut validator = Pangine::new();
    let validated_root = match validator.reference_concept(locator) {
        Ok(Some(root)) => root,
        Ok(None) => return Err(LocatorSelectionError::NoConcept),
        Err(_) => return Err(LocatorSelectionError::InvalidSyntax),
    };

    if validator.format_concept(&validated_root, false) != locator {
        return Err(LocatorSelectionError::NonCanonical);
    }
    if !matches!(validator.concept_kind(&validated_root), Some(ConceptKind::Observation { .. })) {
        return Err(LocatorSelectionError::NotObservation);
    }
    if contains_percept(&validator, &validated_root) {
        return Err(LocatorSelectionError::ContainsPercept);
    }
    if pangine.get_observations(available).is_none() {
        return Err(LocatorSelectionError::InvalidAvailableConcept);
    }

    let local_root =
        pangine.reference_concept(locator).expect("validated canonical locator should parse in the live engine").expect("validated root should not be null");
    let observation = exact_observation_payload(pangine, available, &local_root).ok_or(LocatorSelectionError::Unavailable)?;
    Ok(ExactObservationView { root: local_root, observation })
}

fn agent_policy_record(source: &str, route: &str) -> String {
    format!("?[{source}]:{{{AGENT_POLICY_CONTEXT}->[{route}]}}")
}

fn agent_memory_payload(policy_source: &str, route: &str, reverse_payload_order: bool) -> String {
    let policy = agent_policy_record(policy_source, route);
    let entries = if reverse_payload_order { format!("{policy}, {AGENT_LINT_RECORD}") } else { format!("{AGENT_LINT_RECORD}, {policy}") };
    format!("<{entries}>")
}

fn agent_memory_snapshot(snapshot: &str, policy_source: &str, route: &str, reverse_payload_order: bool) -> String {
    format!("?[{snapshot}]:({})", agent_memory_payload(policy_source, route, reverse_payload_order))
}

fn source_observed_agent_memory_snapshot(source: &str, snapshot: &str, policy_source: &str, route: &str) -> String {
    format!("?[{source}]:{{[{snapshot}]->({})}}", agent_memory_payload(policy_source, route, false))
}

fn snapshot_claim(claimant: &str, snapshot: &str) -> String {
    format!("?[{claimant}]:{{({snapshot})->[preferred-snapshot]}}")
}

fn snapshot_audit(auditor: &str, predecessor: &str, successor: &str) -> String {
    let predecessor_edge = format!("{{({predecessor})->[superseded-by]}}");
    format!("?[{auditor}]:{{{predecessor_edge}->({successor})}}")
}

fn ask(pangine: &mut Pangine, memory_name: &str, memory: Option<ConceptId>, output_name: &str, question_template: &str) -> Option<ConceptId> {
    let memory_percept = pangine.reference_percept(memory_name);
    let output_percept = pangine.reference_percept(output_name);
    assert!(pangine.set_percept_value(&memory_percept, memory));
    assert!(pangine.set_percept_value(&output_percept, None));

    let question = question_template.replace("$OUTPUT", &format!("['{output_name}']"));
    must_reference(pangine, &format!("['{memory_name}'] @ {question}"));
    pangine.get_value(&output_percept)
}

fn named_candidates(pangine: &Pangine, answer: Option<&ConceptId>) -> BTreeSet<String> {
    answer
        .into_iter()
        .flat_map(|answer| pangine.get_relevance_map(answer))
        .filter_map(|(_, candidate)| pangine.get_name(&candidate).map(str::to_owned))
        .collect()
}

fn answer_candidates(pangine: &Pangine, answer: Option<&ConceptId>) -> BTreeSet<ConceptId> {
    answer.into_iter().flat_map(|answer| pangine.get_relevance_map(answer)).map(|(_, candidate)| candidate).collect()
}

fn top_named_candidate(pangine: &Pangine, answer: Option<&ConceptId>) -> Option<String> {
    answer.into_iter().flat_map(|answer| pangine.get_relevance_map(answer)).find_map(|(_, candidate)| pangine.get_name(&candidate).map(str::to_owned))
}

#[test]
fn observer_identity_does_not_make_fuzzy_questions_revision_exact() {
    let mut pangine = Pangine::new();
    let old_state = experience_state(&mut pangine, "fuzzy-old-input", "?[policy-v1]:{[full-test]->[cargo]}");
    let selected_state = experience_state(&mut pangine, "fuzzy-selected-input", "?[policy-v2]:{[full-test]->[cli-runner]}");
    let combined_records = union_records(&pangine, &[&old_state, &selected_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records);

    let combined_answer = ask(&mut pangine, "fuzzy-combined-memory", combined_state, "fuzzy-combined-answer", "?[policy-v2]:{[full-test]->$OUTPUT}");
    assert_eq!(named_candidates(&pangine, combined_answer.as_ref()), BTreeSet::from(["cargo".to_owned(), "cli-runner".to_owned()]));

    let missing_selected_answer = ask(&mut pangine, "fuzzy-degraded-memory", Some(old_state), "fuzzy-degraded-answer", "?[policy-v2]:{[full-test]->$OUTPUT}");
    assert_eq!(named_candidates(&pangine, missing_selected_answer.as_ref()), BTreeSet::from(["cargo".to_owned()]));
}

#[test]
fn unscoped_questions_keep_all_immutable_revisions_available() {
    let mut pangine = Pangine::new();
    let old_state = experience_state(&mut pangine, "unscoped-old-input", "?[policy-v1]:{[full-test]->[cargo]}");
    let selected_state = experience_state(&mut pangine, "unscoped-selected-input", "?[policy-v2]:{[full-test]->[cli-runner]}");
    let combined_records = union_records(&pangine, &[&old_state, &selected_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records);

    let answer = ask(&mut pangine, "unscoped-memory", combined_state, "unscoped-answer", "{[full-test]->$OUTPUT}");
    assert_eq!(named_candidates(&pangine, answer.as_ref()), BTreeSet::from(["cargo".to_owned(), "cli-runner".to_owned()]));
}

#[test]
fn exact_observer_view_commutes_with_partition_union_and_ignores_label_only_audit_links() {
    let mut pangine = Pangine::new();
    let old_state = experience_state(&mut pangine, "partition-old-input", "?[policy-v1]:{[full-test]->[cargo]}");
    let selected_state = experience_state(&mut pangine, "partition-selected-input", "?[policy-v2]:{[full-test]->[cli-runner]}");
    let forward_audit = experience_state(&mut pangine, "partition-forward-audit", "?[editor]:{{[policy-v1]->[superseded-by]}->[policy-v2]}");
    let reverse_audit = experience_state(&mut pangine, "partition-reverse-audit", "?[editor]:{{[policy-v2]->[superseded-by]}->[policy-v1]}");
    let selected_revision = must_reference(&mut pangine, "[policy-v2]");

    let merged_records = union_records(&pangine, &[&old_state, &selected_state, &forward_audit, &reverse_audit]);
    let merged_state = reference_observation_state(&mut pangine, &merged_records).unwrap();
    let selected_from_merged = exact_observer_records(&pangine, &merged_state, Some(&selected_revision));

    let mut selected_from_partitions = BTreeSet::new();
    for partition in [&old_state, &selected_state, &forward_audit, &reverse_audit] {
        selected_from_partitions.extend(exact_observer_records(&pangine, partition, Some(&selected_revision)));
    }
    assert_eq!(selected_from_partitions, selected_from_merged);

    let revision_records = union_records(&pangine, &[&old_state, &selected_state]);
    let revision_state = reference_observation_state(&mut pangine, &revision_records).unwrap();
    assert_eq!(exact_observer_records(&pangine, &revision_state, Some(&selected_revision)), selected_from_merged);
    assert!(exact_observer_records(&pangine, &forward_audit, Some(&selected_revision)).is_empty());
    assert!(exact_observer_records(&pangine, &reverse_audit, Some(&selected_revision)).is_empty());

    let selected_view = reference_observation_state(&mut pangine, &selected_from_merged).unwrap();
    assert!(matches!(pangine.concept_kind(&selected_view), Some(ConceptKind::Observation { .. } | ConceptKind::ObservationSet)));
    let answer = ask(&mut pangine, "exact-selected-memory", Some(selected_view), "exact-selected-answer", "{[full-test]->$OUTPUT}");
    assert_eq!(named_candidates(&pangine, answer.as_ref()), BTreeSet::from(["cli-runner".to_owned()]));
}

#[test]
fn losing_the_selected_partition_returns_no_view_instead_of_the_old_revision() {
    let mut pangine = Pangine::new();
    let old_state = experience_state(&mut pangine, "degraded-old-input", "?[policy-v1]:{[full-test]->[cargo]}");
    let audit_state = experience_state(&mut pangine, "degraded-audit-input", "?[editor]:{{[policy-v1]->[superseded-by]}->[policy-v2]}");
    let selected_revision = must_reference(&mut pangine, "[policy-v2]");
    let degraded_records = union_records(&pangine, &[&old_state, &audit_state]);
    let degraded_state = reference_observation_state(&mut pangine, &degraded_records).unwrap();
    let selected_records = exact_observer_records(&pangine, &degraded_state, Some(&selected_revision));

    assert!(selected_records.is_empty());
    let selected_view = reference_observation_state(&mut pangine, &selected_records);
    let answer = ask(&mut pangine, "exact-degraded-memory", selected_view, "exact-degraded-answer", "{[full-test]->$OUTPUT}");
    assert!(answer.is_none());
}

#[test]
fn a_revision_wrapper_preserves_the_original_observer_as_ordinary_nested_structure() {
    let mut pangine = Pangine::new();
    let old_state = experience_state(&mut pangine, "nested-old-input", "?[chart-v1]:(?[doctor]:{[treatment]->[cargo]})");
    let selected_state = experience_state(&mut pangine, "nested-selected-input", "?[chart-v2]:(?[doctor]:{[treatment]->[cli-runner]})");
    let selected_revision = must_reference(&mut pangine, "[chart-v2]");
    let doctor = must_reference(&mut pangine, "[doctor]");
    let selected_root = must_reference(&mut pangine, "?[chart-v2]:(?[doctor]:{[treatment]->[cli-runner]})");
    let combined_records = union_records(&pangine, &[&old_state, &selected_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();

    let selected_records = exact_observer_records(&pangine, &combined_state, Some(&selected_revision));
    assert_eq!(selected_records, BTreeSet::from([selected_root.clone()]));

    let source_observation = pangine.get_observation(&selected_root).unwrap();
    assert_eq!(pangine.get_observer(&source_observation), Some(doctor));
    assert_eq!(pangine.format_concept(&source_observation, false), "?[doctor]:{[treatment]->[cli-runner]}");

    let selected_view = reference_observation_state(&mut pangine, &selected_records);
    let answer = ask(&mut pangine, "nested-selected-memory", selected_view, "nested-selected-answer", "?[doctor]:{[treatment]->$OUTPUT}");
    assert_eq!(named_candidates(&pangine, answer.as_ref()), BTreeSet::from(["cli-runner".to_owned()]));
}

#[test]
fn snapshot_identity_does_not_require_the_observer_position() {
    let observer_as_identity_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let source_observed_source = source_observed_agent_memory_snapshot("snapshot-author", "agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner");

    let mut pangine = Pangine::new();
    let observer_as_identity_root = must_reference(&mut pangine, &observer_as_identity_source);
    let source_observed_root = must_reference(&mut pangine, &source_observed_source);
    let snapshot_identity = must_reference(&mut pangine, "[agent-memory-snapshot-v2]");
    let snapshot_author = must_reference(&mut pangine, "[snapshot-author]");

    assert_ne!(observer_as_identity_root, source_observed_root);
    assert_eq!(pangine.get_observer(&observer_as_identity_root), Some(snapshot_identity.clone()));
    assert_eq!(pangine.get_observer(&source_observed_root), Some(snapshot_author));

    let observer_as_identity_payload = pangine.get_observation(&observer_as_identity_root).unwrap();
    let source_observed_claim = pangine.get_observation(&source_observed_root).unwrap();
    assert_eq!(pangine.get_correlation_a(&source_observed_claim), Some(snapshot_identity));
    let source_observed_payload = pangine.get_correlation_b(&source_observed_claim).unwrap();
    assert_eq!(source_observed_payload, observer_as_identity_payload);

    let observer_as_identity_answer =
        ask(&mut pangine, "observer-as-identity-payload", Some(observer_as_identity_payload), "observer-as-identity-answer", AGENT_POLICY_QUESTION);
    let source_observed_answer = ask(&mut pangine, "source-observed-payload", Some(source_observed_payload), "source-observed-answer", AGENT_POLICY_QUESTION);
    assert_eq!(observer_as_identity_answer, source_observed_answer);
    assert_eq!(top_named_candidate(&pangine, source_observed_answer.as_ref()).as_deref(), Some("cli-runner"));

    let observer_as_identity_state = experience_state(&mut pangine, "observer-as-identity-state", &observer_as_identity_source);
    let source_observed_state = experience_state(&mut pangine, "source-observed-state", &source_observed_source);
    assert!(exact_observation_record(&pangine, &observer_as_identity_state, &source_observed_root).is_none());
    assert!(exact_observation_record(&pangine, &source_observed_state, &observer_as_identity_root).is_none());
}

#[test]
fn ordinary_structure_does_not_choose_between_multiple_nested_payloads() {
    let selected_payload_source = agent_memory_payload("policy-pangine-full-v2", "cli-runner", false);
    let attachment_payload_source = agent_memory_payload("policy-pangine-full-attachment", "cargo", false);
    let root_source = format!("?[snapshot-author]:({{[snapshot-v2]->({selected_payload_source})}}{{[attachment]->({attachment_payload_source})}})");

    let mut pangine = Pangine::new();
    let root = must_reference(&mut pangine, &root_source);
    let selected_payload = must_reference(&mut pangine, &selected_payload_source);
    let attachment_payload = must_reference(&mut pangine, &attachment_payload_source);
    let snapshot_identity = must_reference(&mut pangine, "[snapshot-v2]");

    assert_ne!(selected_payload, attachment_payload);
    assert_eq!(nested_observation_sets(&pangine, &root), BTreeSet::from([selected_payload.clone(), attachment_payload.clone()]));

    let envelope = pangine.get_observation(&root).unwrap();
    assert_eq!(default_correlation_targets(&pangine, &envelope, &snapshot_identity), BTreeSet::from([selected_payload.clone()]));

    let selected_answer = ask(&mut pangine, "explicit-selected-payload", Some(selected_payload), "explicit-selected-answer", AGENT_POLICY_QUESTION);
    let attachment_answer = ask(&mut pangine, "attachment-payload", Some(attachment_payload), "attachment-answer", AGENT_POLICY_QUESTION);
    assert_eq!(top_named_candidate(&pangine, selected_answer.as_ref()).as_deref(), Some("cli-runner"));
    assert_eq!(top_named_candidate(&pangine, attachment_answer.as_ref()).as_deref(), Some("cargo"));
}

#[test]
fn a_fuzzy_keyed_question_does_not_replace_exact_structural_payload_lookup() {
    let selected_payload_source = agent_memory_payload("policy-pangine-full-v2", "cli-runner", false);
    let attachment_payload_source = agent_memory_payload("policy-pangine-full-attachment", "cargo", false);
    let selected_edge_source = format!("{{[snapshot-v2]->({selected_payload_source})}}");
    let attachment_edge_source = format!("{{[attachment]->({attachment_payload_source})}}");
    let root_source = format!("?[snapshot-author]:(({selected_edge_source})({attachment_edge_source}))");

    let mut pangine = Pangine::new();
    let root = must_reference(&mut pangine, &root_source);
    let selected_payload = must_reference(&mut pangine, &selected_payload_source);
    let attachment_payload = must_reference(&mut pangine, &attachment_payload_source);
    let snapshot_identity = must_reference(&mut pangine, "[snapshot-v2]");
    let envelope = pangine.get_observation(&root).unwrap();

    let exact_targets = default_correlation_targets(&pangine, &envelope, &snapshot_identity);
    assert_eq!(exact_targets, BTreeSet::from([selected_payload.clone()]));

    let fuzzy_answer = ask(&mut pangine, "fuzzy-payload-envelope", Some(envelope), "fuzzy-payload-answer", "{[snapshot-v2]->$OUTPUT}");
    let fuzzy_candidates = answer_candidates(&pangine, fuzzy_answer.as_ref());
    assert_eq!(fuzzy_candidates, BTreeSet::from([selected_payload, attachment_payload]));
    assert_ne!(fuzzy_candidates, exact_targets);
}

#[test]
fn non_default_payload_edges_require_explicit_application_semantics() {
    let payload_source = agent_memory_payload("policy-pangine-full-v2", "cli-runner", false);
    let edge_source = format!("{{[snapshot-v2]->({payload_source})}}");
    let positive_root_source = format!("?[snapshot-author]:({edge_source})");
    let inverted_root_source = format!("?[snapshot-author]:(!({edge_source}))");
    let weighted_root_source = format!("?[snapshot-author]:(x2({edge_source}))");

    let mut pangine = Pangine::new();
    let payload = must_reference(&mut pangine, &payload_source);
    let edge = must_reference(&mut pangine, &edge_source);
    let positive_root = must_reference(&mut pangine, &positive_root_source);
    let inverted_root = must_reference(&mut pangine, &inverted_root_source);
    let weighted_root = must_reference(&mut pangine, &weighted_root_source);
    let snapshot_identity = must_reference(&mut pangine, "[snapshot-v2]");
    let positive_envelope = pangine.get_observation(&positive_root).unwrap();
    let inverted_envelope = pangine.get_observation(&inverted_root).unwrap();
    let weighted_envelope = pangine.get_observation(&weighted_root).unwrap();

    assert_ne!(positive_root, inverted_root);
    assert_ne!(positive_root, weighted_root);
    assert_eq!(pangine.get_relevance_map(&positive_envelope), vec![(Relevance::DEFAULT, edge.clone())]);
    assert_eq!(pangine.get_relevance_map(&inverted_envelope), vec![(Relevance::new(1.0, -1.0), edge.clone())]);
    assert_eq!(pangine.get_relevance_map(&weighted_envelope), vec![(Relevance::new(1.0, 2.0), edge)]);

    assert_eq!(default_correlation_targets(&pangine, &positive_envelope, &snapshot_identity), BTreeSet::from([payload]));
    assert!(default_correlation_targets(&pangine, &inverted_envelope, &snapshot_identity).is_empty());
    assert!(default_correlation_targets(&pangine, &weighted_envelope, &snapshot_identity).is_empty());
}

#[test]
fn a_reused_payload_key_preserves_both_targets_until_the_application_selects_an_exact_edge() {
    let selected_payload_source = agent_memory_payload("policy-pangine-full-v2a", "cli-runner", false);
    let competing_payload_source = agent_memory_payload("policy-pangine-full-v2b", "cargo", false);
    let selected_edge_source = format!("{{[snapshot-v2]->({selected_payload_source})}}");
    let competing_edge_source = format!("{{[snapshot-v2]->({competing_payload_source})}}");
    let root_source = format!("?[snapshot-author]:(({selected_edge_source})({competing_edge_source}))");

    let mut pangine = Pangine::new();
    let root = must_reference(&mut pangine, &root_source);
    let selected_payload = must_reference(&mut pangine, &selected_payload_source);
    let competing_payload = must_reference(&mut pangine, &competing_payload_source);
    let selected_edge = must_reference(&mut pangine, &selected_edge_source);
    let competing_edge = must_reference(&mut pangine, &competing_edge_source);
    let snapshot_identity = must_reference(&mut pangine, "[snapshot-v2]");
    let envelope = pangine.get_observation(&root).unwrap();

    assert_ne!(selected_edge, competing_edge);
    assert_eq!(
        pangine.get_relevance_map(&envelope).into_iter().map(|(_, child)| child).collect::<BTreeSet<_>>(),
        BTreeSet::from([selected_edge.clone(), competing_edge])
    );
    assert_eq!(default_correlation_targets(&pangine, &envelope, &snapshot_identity), BTreeSet::from([selected_payload.clone(), competing_payload.clone()]));

    assert_eq!(pangine.get_correlation_b(&selected_edge), Some(selected_payload.clone()));
    let selected_answer = ask(&mut pangine, "exact-edge-payload", Some(selected_payload), "exact-edge-answer", AGENT_POLICY_QUESTION);
    let competing_answer = ask(&mut pangine, "competing-edge-payload", Some(competing_payload), "competing-edge-answer", AGENT_POLICY_QUESTION);
    assert_eq!(top_named_candidate(&pangine, selected_answer.as_ref()).as_deref(), Some("cli-runner"));
    assert_eq!(top_named_candidate(&pangine, competing_answer.as_ref()).as_deref(), Some("cargo"));
}

#[test]
fn exact_observer_selection_is_a_general_source_view_for_recursive_and_global_observers() {
    let mut pangine = Pangine::new();
    let north_state = experience_state(&mut pangine, "source-north-input", "?({[sensor]->[north]}):{[weather]->[rain]}");
    let south_state = experience_state(&mut pangine, "source-south-input", "?({[sensor]->[south]}):{[weather]->[dry]}");
    let global_state = experience_state(&mut pangine, "source-global-input", "[maintenance]");
    let north_observer = must_reference(&mut pangine, "{[sensor]->[north]}");
    let north_root = must_reference(&mut pangine, "?({[sensor]->[north]}):{[weather]->[rain]}");
    let global_root = must_reference(&mut pangine, "?[]:[maintenance]");
    let combined_records = union_records(&pangine, &[&north_state, &south_state, &global_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();

    let north_records = exact_observer_records(&pangine, &combined_state, Some(&north_observer));
    assert!(north_records.contains(&north_root));
    assert!(north_records.iter().all(|record| pangine.get_observer(record).as_ref() == Some(&north_observer)));

    let global_records = exact_observer_records(&pangine, &combined_state, None);
    assert_eq!(global_records, BTreeSet::from([global_root]));

    let north_view = reference_observation_state(&mut pangine, &north_records);
    let answer = ask(&mut pangine, "source-north-memory", north_view, "source-north-answer", "{[weather]->$OUTPUT}");
    assert_eq!(named_candidates(&pangine, answer.as_ref()), BTreeSet::from(["rain".to_owned()]));
}

#[test]
fn losing_one_selected_subpartition_removes_only_its_revision_records() {
    let mut pangine = Pangine::new();
    let old_partition = experience_state(&mut pangine, "split-old-input", "?[policy-v1]:{[full-test]->[cargo]}");
    let selected_test_partition = experience_state(&mut pangine, "split-selected-test-input", "?[policy-v2]:{[full-test]->[cli-runner]}");
    let selected_lint_partition = experience_state(&mut pangine, "split-selected-lint-input", "?[policy-v2]:{[lint]->[clippy]}");
    let audit_partition = experience_state(&mut pangine, "split-audit-input", "?[editor]:{{[policy-v1]->[superseded-by]}->[policy-v2]}");
    let selected_revision = must_reference(&mut pangine, "[policy-v2]");
    let selected_test_root = must_reference(&mut pangine, "?[policy-v2]:{[full-test]->[cli-runner]}");
    let selected_lint_root = must_reference(&mut pangine, "?[policy-v2]:{[lint]->[clippy]}");

    let merged_records = union_records(&pangine, &[&old_partition, &selected_test_partition, &selected_lint_partition, &audit_partition]);
    let merged_state = reference_observation_state(&mut pangine, &merged_records).unwrap();
    let selected_from_merged = exact_observer_records(&pangine, &merged_state, Some(&selected_revision));

    let mut selected_from_partitions = BTreeSet::new();
    for partition in [&old_partition, &selected_test_partition, &selected_lint_partition, &audit_partition] {
        selected_from_partitions.extend(exact_observer_records(&pangine, partition, Some(&selected_revision)));
    }
    assert_eq!(selected_from_partitions, selected_from_merged);
    assert!(selected_from_merged.contains(&selected_test_root));
    assert!(selected_from_merged.contains(&selected_lint_root));

    let degraded_records = union_records(&pangine, &[&old_partition, &selected_lint_partition, &audit_partition]);
    let degraded_state = reference_observation_state(&mut pangine, &degraded_records).unwrap();
    let selected_degraded = exact_observer_records(&pangine, &degraded_state, Some(&selected_revision));
    assert_eq!(selected_degraded, exact_observer_records(&pangine, &selected_lint_partition, Some(&selected_revision)));
    assert!(!selected_degraded.contains(&selected_test_root));
    assert!(selected_degraded.contains(&selected_lint_root));
}

#[test]
fn a_self_contained_snapshot_avoids_predecessor_traversal_and_shares_unchanged_concepts() {
    let mut pangine = Pangine::new();
    let old_state = experience_state(&mut pangine, "snapshot-old-input", "?[snapshot-v1]:(<?[doctor]:{[treatment]->[cargo]}, ?[reviewer]:[approved]>)");
    let selected_state =
        experience_state(&mut pangine, "snapshot-selected-input", "?[snapshot-v2]:(<?[doctor]:{[treatment]->[cli-runner]}, ?[reviewer]:[approved]>)");
    let forward_audit = experience_state(&mut pangine, "snapshot-forward-audit", "?[editor]:{{[snapshot-v1]->[superseded-by]}->[snapshot-v2]}");
    let reverse_audit = experience_state(&mut pangine, "snapshot-reverse-audit", "?[editor]:{{[snapshot-v2]->[superseded-by]}->[snapshot-v1]}");
    let old_revision = must_reference(&mut pangine, "[snapshot-v1]");
    let selected_revision = must_reference(&mut pangine, "[snapshot-v2]");
    let unchanged_approval = must_reference(&mut pangine, "?[reviewer]:[approved]");

    let old_payload = sole_exact_observer_payload(&pangine, &old_state, &old_revision).unwrap();
    let selected_payload = sole_exact_observer_payload(&pangine, &selected_state, &selected_revision).unwrap();
    assert!(observation_records(&pangine, &old_payload).contains(&unchanged_approval));
    assert!(observation_records(&pangine, &selected_payload).contains(&unchanged_approval));

    let combined_records = union_records(&pangine, &[&old_state, &selected_state, &forward_audit, &reverse_audit]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();
    assert_eq!(sole_exact_observer_payload(&pangine, &combined_state, &selected_revision), Some(selected_payload.clone()));

    let answer = ask(&mut pangine, "snapshot-selected-memory", Some(selected_payload), "snapshot-selected-answer", "?[doctor]:{[treatment]->$OUTPUT}");
    assert_eq!(named_candidates(&pangine, answer.as_ref()), BTreeSet::from(["cli-runner".to_owned()]));
}

#[test]
fn delta_reconstruction_needs_application_replacement_semantics() {
    let mut pangine = Pangine::new();
    let base_state = experience_state(&mut pangine, "delta-base-input", "?[delta-v1]:(<?[doctor]:{[treatment]->[cargo]}, ?[reviewer]:[approved]>)");
    let delta_state = experience_state(&mut pangine, "delta-change-input", "?[delta-v2]:(?[doctor]:{[treatment]->[cli-runner]})");
    let base_revision = must_reference(&mut pangine, "[delta-v1]");
    let delta_revision = must_reference(&mut pangine, "[delta-v2]");
    let unchanged_approval = must_reference(&mut pangine, "?[reviewer]:[approved]");
    let base_payload = sole_exact_observer_payload(&pangine, &base_state, &base_revision).unwrap();
    let delta_payload = sole_exact_observer_payload(&pangine, &delta_state, &delta_revision).unwrap();

    assert!(observation_records(&pangine, &base_payload).contains(&unchanged_approval));
    assert!(!observation_records(&pangine, &delta_payload).contains(&unchanged_approval));

    let reconstructed_records = union_records(&pangine, &[&base_payload, &delta_payload]);
    let reconstructed_state = reference_observation_state(&mut pangine, &reconstructed_records);
    let answer = ask(&mut pangine, "delta-reconstructed-memory", reconstructed_state, "delta-reconstructed-answer", "?[doctor]:{[treatment]->$OUTPUT}");
    assert_eq!(named_candidates(&pangine, answer.as_ref()), BTreeSet::from(["cargo".to_owned(), "cli-runner".to_owned()]));
}

#[test]
fn a_reused_revision_label_is_ambiguous_while_each_complete_root_remains_exact() {
    let mut pangine = Pangine::new();
    let first_state = experience_state(&mut pangine, "collision-first-input", "?[snapshot-id]:(?[doctor]:{[treatment]->[cargo]})");
    let second_state = experience_state(&mut pangine, "collision-second-input", "?[snapshot-id]:(?[doctor]:{[treatment]->[cli-runner]})");
    let revision = must_reference(&mut pangine, "[snapshot-id]");
    let first_root = must_reference(&mut pangine, "?[snapshot-id]:(?[doctor]:{[treatment]->[cargo]})");
    let second_root = must_reference(&mut pangine, "?[snapshot-id]:(?[doctor]:{[treatment]->[cli-runner]})");
    let combined_records = union_records(&pangine, &[&first_state, &second_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();
    let selected = exact_observer_records(&pangine, &combined_state, Some(&revision));

    assert_eq!(selected.len(), 2);
    assert!(selected.iter().all(|record| pangine.get_observer(record).as_ref() == Some(&revision)));
    assert_eq!(exact_observation_record(&pangine, &combined_state, &first_root), Some(first_root.clone()));
    assert_eq!(exact_observation_record(&pangine, &combined_state, &second_root), Some(second_root.clone()));

    assert!(exact_observation_record(&pangine, &second_state, &first_root).is_none());
    assert_eq!(exact_observation_record(&pangine, &second_state, &second_root), Some(second_root));
}

#[test]
fn a_live_percept_reference_is_not_portable_snapshot_content() {
    let mut producer = Pangine::new();
    let producer_policy = agent_policy_record("policy-pangine-full-v1", "cargo");
    must_reference(&mut producer, &format!("['agent-policy'] = {producer_policy}"));
    let producer_root = must_reference(&mut producer, "?[agent-memory-live]:['agent-policy']");
    let locator = producer.format_concept(&producer_root, false);
    let producer_evaluated = producer.format_concept(&producer_root, true);
    assert!(contains_percept(&producer, &producer_root));

    let mut consumer = Pangine::new();
    let consumer_policy = agent_policy_record("policy-pangine-full-v2", "cli-runner");
    must_reference(&mut consumer, &format!("['agent-policy'] = {consumer_policy}"));
    let consumer_root = must_reference(&mut consumer, &locator);

    assert_eq!(consumer.format_concept(&consumer_root, false), locator);
    assert_ne!(consumer.format_concept(&consumer_root, true), producer_evaluated);
}

#[test]
fn evaluating_a_percept_before_wrapping_freezes_an_ordinary_snapshot() {
    let mut producer = Pangine::new();
    let old_policy = agent_policy_record("policy-pangine-full-v1", "cargo");
    must_reference(&mut producer, &format!("['agent-policy'] = {old_policy}"));
    let frozen_root = must_reference(&mut producer, "?[agent-memory-frozen]:($['agent-policy'])");
    let locator = producer.format_concept(&frozen_root, false);
    let evaluated_before_update = producer.format_concept(&frozen_root, true);
    assert!(!contains_percept(&producer, &frozen_root));

    let new_policy = agent_policy_record("policy-pangine-full-v2", "cli-runner");
    must_reference(&mut producer, &format!("['agent-policy'] = {new_policy}"));
    assert_eq!(producer.format_concept(&frozen_root, false), locator);
    assert_eq!(producer.format_concept(&frozen_root, true), evaluated_before_update);

    let mut consumer = Pangine::new();
    must_reference(&mut consumer, &format!("['agent-policy'] = {new_policy}"));
    let local_root = must_reference(&mut consumer, &locator);
    assert_eq!(consumer.format_concept(&local_root, true), evaluated_before_update);

    let available_state = experience_state(&mut consumer, "agent-memory-frozen-input", &locator);
    let payload = exact_observation_payload(&consumer, &available_state, &local_root).unwrap();
    let answer = ask(&mut consumer, "agent-memory-frozen-memory", Some(payload), "agent-memory-frozen-answer", AGENT_POLICY_QUESTION);
    assert_eq!(named_candidates(&consumer, answer.as_ref()), BTreeSet::from(["cargo".to_owned()]));
}

#[test]
fn evaluating_a_percept_cycle_does_not_produce_self_contained_content() {
    let mut pangine = Pangine::new();
    must_reference(&mut pangine, "['agent-policy'] = ['agent-policy']");
    let root = must_reference(&mut pangine, "?[agent-memory-cycle]:($['agent-policy'])");

    assert!(contains_percept(&pangine, &root));
    assert_eq!(pangine.format_concept(&root, false), pangine.format_concept(&root, true));
}

#[test]
fn canonical_snapshot_text_round_trips_across_engines() {
    let source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let reverse_order_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", true);

    let mut producer = Pangine::new();
    let producer_root = must_reference(&mut producer, &source);
    let locator = producer.format_concept(&producer_root, false);

    let mut alternate_producer = Pangine::new();
    let alternate_root = must_reference(&mut alternate_producer, &reverse_order_source);
    assert_eq!(alternate_producer.format_concept(&alternate_root, false), locator);

    let mut consumer = Pangine::new();
    assert!(consumer.get_observation(&producer_root).is_none());
    let local_root = must_reference(&mut consumer, &locator);
    assert_ne!(local_root, producer_root);
    assert_eq!(consumer.format_concept(&local_root, false), locator);

    let available_state = experience_state(&mut consumer, "agent-memory-available-input", &source);
    let payload = exact_observation_payload(&consumer, &available_state, &local_root).unwrap();
    let answer = ask(&mut consumer, "agent-memory-selected-memory", Some(payload), "agent-memory-selected-answer", AGENT_POLICY_QUESTION);
    assert_eq!(named_candidates(&consumer, answer.as_ref()), BTreeSet::from(["cargo".to_owned(), "cli-runner".to_owned()]));
    assert_eq!(top_named_candidate(&consumer, answer.as_ref()).as_deref(), Some("cli-runner"));
}

#[test]
fn a_caller_held_snapshot_needs_unwrapping_but_not_state_selection() {
    let selected_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let old_source = agent_memory_snapshot("agent-memory-snapshot-v1", "policy-pangine-full-v1", "cargo", false);

    let mut producer = Pangine::new();
    let producer_root = must_reference(&mut producer, &selected_source);
    let selected_root_text = producer.format_concept(&producer_root, false);

    let mut consumer = Pangine::new();
    let selected_root = must_reference(&mut consumer, &selected_root_text);
    let selected_payload = consumer.get_observation(&selected_root).unwrap();
    let old_state = experience_state(&mut consumer, "caller-held-old-memory", &old_source);

    assert_eq!(exact_observation_view_from_locator(&mut consumer, &old_state, &selected_root_text), Err(LocatorSelectionError::Unavailable));

    let root_answer = ask(&mut consumer, "caller-held-root", Some(selected_root.clone()), "caller-held-root-answer", AGENT_POLICY_QUESTION);
    let payload_answer = ask(&mut consumer, "caller-held-payload", Some(selected_payload.clone()), "caller-held-payload-answer", AGENT_POLICY_QUESTION);

    assert!(root_answer.is_none());
    assert_eq!(top_named_candidate(&consumer, payload_answer.as_ref()).as_deref(), Some("cli-runner"));

    let selected_state = experience_state(&mut consumer, "caller-held-selected-memory", &selected_source);
    let available_records = union_records(&consumer, &[&old_state, &selected_state]);
    let available_state = reference_observation_state(&mut consumer, &available_records).unwrap();
    let selected_view = exact_observation_view_from_locator(&mut consumer, &available_state, &selected_root_text).unwrap();
    assert_eq!(selected_view.root, selected_root);
    assert_eq!(selected_view.observation, selected_payload);
}

#[test]
fn parsed_root_text_does_not_prove_membership_in_another_state() {
    let selected_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);

    let mut producer = Pangine::new();
    let producer_root = must_reference(&mut producer, &selected_source);
    let locator = producer.format_concept(&producer_root, false);

    let mut consumer = Pangine::new();
    let local_root = must_reference(&mut consumer, &locator);
    let old_source = agent_memory_snapshot("agent-memory-snapshot-v1", "policy-pangine-full-v1", "cargo", false);
    let old_partition = experience_state(&mut consumer, "agent-memory-old-partition", &old_source);
    let selected_policy = agent_policy_record("policy-pangine-full-v2", "cli-runner");
    let selected_policy_partition = experience_state(&mut consumer, "agent-memory-selected-policy-fragment", &selected_policy);
    let degraded_records = union_records(&consumer, &[&old_partition, &selected_policy_partition]);
    let degraded_state = reference_observation_state(&mut consumer, &degraded_records).unwrap();

    let interned_payload = consumer.get_observation(&local_root).unwrap();
    assert!(observation_records(&consumer, &interned_payload).is_subset(&degraded_records));
    assert!(exact_observation_payload(&consumer, &degraded_state, &local_root).is_none());

    let selected_partition = experience_state(&mut consumer, "agent-memory-selected-partition", &selected_source);
    let available_records = union_records(&consumer, &[&old_partition, &selected_policy_partition, &selected_partition]);
    let available_state = reference_observation_state(&mut consumer, &available_records).unwrap();
    let selected_payload = exact_observation_payload(&consumer, &available_state, &local_root).unwrap();
    let answer = ask(&mut consumer, "agent-memory-restored-memory", Some(selected_payload), "agent-memory-restored-answer", AGENT_POLICY_QUESTION);
    assert_eq!(named_candidates(&consumer, answer.as_ref()), BTreeSet::from(["cargo".to_owned(), "cli-runner".to_owned()]));
    assert_eq!(top_named_candidate(&consumer, answer.as_ref()).as_deref(), Some("cli-runner"));
}

#[test]
fn competing_agent_memory_snapshots_remain_explicit_caller_choices() {
    let cli_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full-v2a", "cli-runner", false);
    let cargo_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full-v2b", "cargo", false);
    let mut pangine = Pangine::new();
    let cli_partition = experience_state(&mut pangine, "agent-memory-cli-branch", &cli_source);
    let cargo_partition = experience_state(&mut pangine, "agent-memory-cargo-branch", &cargo_source);
    let snapshot_label = must_reference(&mut pangine, "[agent-memory-snapshot]");
    let cli_root = must_reference(&mut pangine, &cli_source);
    let cargo_root = must_reference(&mut pangine, &cargo_source);
    let combined_records = union_records(&pangine, &[&cli_partition, &cargo_partition]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();

    assert_eq!(exact_observer_records(&pangine, &combined_state, Some(&snapshot_label)), BTreeSet::from([cli_root.clone(), cargo_root.clone()]));

    let cli_payload = exact_observation_payload(&pangine, &combined_state, &cli_root).unwrap();
    let cli_answer = ask(&mut pangine, "agent-memory-cli-memory", Some(cli_payload), "agent-memory-cli-answer", AGENT_POLICY_QUESTION);
    assert_eq!(named_candidates(&pangine, cli_answer.as_ref()), BTreeSet::from(["cargo".to_owned(), "cli-runner".to_owned()]));
    assert_eq!(top_named_candidate(&pangine, cli_answer.as_ref()).as_deref(), Some("cli-runner"));

    let cargo_payload = exact_observation_payload(&pangine, &combined_state, &cargo_root).unwrap();
    let cargo_answer = ask(&mut pangine, "agent-memory-cargo-memory", Some(cargo_payload), "agent-memory-cargo-answer", AGENT_POLICY_QUESTION);
    assert_eq!(named_candidates(&pangine, cargo_answer.as_ref()), BTreeSet::from(["cargo".to_owned()]));
    assert_eq!(top_named_candidate(&pangine, cargo_answer.as_ref()).as_deref(), Some("cargo"));
}

#[test]
fn an_exact_state_membership_gate_reports_distinct_contract_failures_without_falling_back() {
    let selected_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let mut producer = Pangine::new();
    let producer_root = must_reference(&mut producer, &selected_source);
    let locator = producer.format_concept(&producer_root, false);

    let mut live_locator_engine = Pangine::new();
    let live_root = must_reference(&mut live_locator_engine, "?[agent-memory-live]:['agent-policy']");
    let live_locator = live_locator_engine.format_concept(&live_root, false);

    let mut pangine = Pangine::new();
    let invalid_available = must_reference(&mut pangine, "[not-observation-state]");
    let old_source = agent_memory_snapshot("agent-memory-snapshot-v1", "policy-pangine-full-v1", "cargo", false);
    let old_state = experience_state(&mut pangine, "locator-old-state", &old_source);

    assert_eq!(exact_observation_view_from_locator(&mut pangine, &old_state, "?[broken]:"), Err(LocatorSelectionError::InvalidSyntax));
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &old_state, "[]"), Err(LocatorSelectionError::NoConcept));
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &old_state, &format!("({locator})")), Err(LocatorSelectionError::NonCanonical));
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &old_state, "[plain]"), Err(LocatorSelectionError::NotObservation));
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &old_state, &live_locator), Err(LocatorSelectionError::ContainsPercept));
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &invalid_available, &locator), Err(LocatorSelectionError::InvalidAvailableConcept));
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &old_state, &locator), Err(LocatorSelectionError::Unavailable));

    let selected_state = experience_state(&mut pangine, "locator-selected-state", &selected_source);
    let available_records = union_records(&pangine, &[&old_state, &selected_state]);
    let available_state = reference_observation_state(&mut pangine, &available_records).unwrap();
    let selected_view = exact_observation_view_from_locator(&mut pangine, &available_state, &locator).unwrap();
    assert_eq!(pangine.format_concept(&selected_view.root, false), locator);

    let answer = ask(&mut pangine, "locator-selected-memory", Some(selected_view.observation), "locator-selected-answer", AGENT_POLICY_QUESTION);
    assert_eq!(top_named_candidate(&pangine, answer.as_ref()).as_deref(), Some("cli-runner"));
}

#[test]
fn root_text_validation_does_not_execute_untrusted_text_in_the_live_engine() {
    let selected_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let mut producer = Pangine::new();
    let producer_root = must_reference(&mut producer, &selected_source);
    let locator = producer.format_concept(&producer_root, false);
    let assignment = format!("['selection-target'] = {locator}");

    let mut direct_engine = Pangine::new();
    let direct_target = direct_engine.reference_percept("selection-target");
    let directly_parsed_root = must_reference(&mut direct_engine, &assignment);
    assert_eq!(direct_engine.get_value(&direct_target), Some(directly_parsed_root));

    let mut protected_engine = Pangine::new();
    let before = must_reference(&mut protected_engine, "[before]");
    let protected_target = protected_engine.reference_percept("selection-target");
    assert!(protected_engine.set_percept_value(&protected_target, Some(before.clone())));
    let available_state = experience_state(&mut protected_engine, "protected-locator-state", &selected_source);

    assert_eq!(exact_observation_view_from_locator(&mut protected_engine, &available_state, &assignment), Err(LocatorSelectionError::NonCanonical));
    assert_eq!(protected_engine.get_value(&protected_target), Some(before));
}

#[test]
fn a_closed_exact_observation_view_cannot_certify_snapshot_completeness() {
    let complete_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let delta_policy = agent_policy_record("policy-pangine-full-v2", "cli-runner");
    let delta_source = format!("?[agent-memory-delta-v2]:({delta_policy})");

    let mut producer = Pangine::new();
    let complete_root = must_reference(&mut producer, &complete_source);
    let delta_root = must_reference(&mut producer, &delta_source);
    let complete_locator = producer.format_concept(&complete_root, false);
    let delta_locator = producer.format_concept(&delta_root, false);

    let mut pangine = Pangine::new();
    let complete_state = experience_state(&mut pangine, "complete-view-state", &complete_source);
    let delta_state = experience_state(&mut pangine, "delta-view-state", &delta_source);
    let lint_record = must_reference(&mut pangine, AGENT_LINT_RECORD);

    let complete_view = exact_observation_view_from_locator(&mut pangine, &complete_state, &complete_locator).unwrap();
    let delta_view = exact_observation_view_from_locator(&mut pangine, &delta_state, &delta_locator).unwrap();
    assert!(observation_records(&pangine, &complete_view.observation).contains(&lint_record));
    assert!(!observation_records(&pangine, &delta_view.observation).contains(&lint_record));

    let delta_answer = ask(&mut pangine, "delta-view-memory", Some(delta_view.observation), "delta-view-answer", AGENT_POLICY_QUESTION);
    assert_eq!(top_named_candidate(&pangine, delta_answer.as_ref()).as_deref(), Some("cli-runner"));
}

#[test]
fn an_in_band_current_observation_accumulates_and_reactivates_an_old_choice() {
    let old_source = agent_memory_snapshot("agent-memory-snapshot-v1", "policy-pangine-full-v1", "cargo", false);
    let selected_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let old_current_source = format!("?[agent-memory-current]:({old_source})");
    let selected_current_source = format!("?[agent-memory-current]:({selected_source})");

    let mut pangine = Pangine::new();
    let old_partition = experience_state(&mut pangine, "current-old-partition", &old_current_source);
    let selected_partition = experience_state(&mut pangine, "current-selected-partition", &selected_current_source);
    let current_label = must_reference(&mut pangine, "[agent-memory-current]");
    let old_current_root = must_reference(&mut pangine, &old_current_source);
    let selected_current_root = must_reference(&mut pangine, &selected_current_source);
    let old_snapshot_root = must_reference(&mut pangine, &old_source);
    let selected_snapshot_root = must_reference(&mut pangine, &selected_source);
    let selected_locator = pangine.format_concept(&selected_snapshot_root, false);

    let combined_records = union_records(&pangine, &[&old_partition, &selected_partition]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();
    assert_eq!(exact_observer_records(&pangine, &combined_state, Some(&current_label)), BTreeSet::from([old_current_root.clone(), selected_current_root]));

    let degraded_current = exact_observer_records(&pangine, &old_partition, Some(&current_label));
    assert_eq!(degraded_current, BTreeSet::from([old_current_root.clone()]));
    let reactivated_snapshot = pangine.get_observation(&old_current_root).unwrap();
    assert_eq!(reactivated_snapshot, old_snapshot_root);

    assert!(exact_observation_view_from_locator(&mut pangine, &combined_state, &selected_locator).is_ok());
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &old_partition, &selected_locator), Err(LocatorSelectionError::Unavailable));
}

#[test]
fn exact_complete_root_membership_is_an_idempotent_partition_fold() {
    let old_source = agent_memory_snapshot("agent-memory-snapshot-v1", "policy-pangine-full-v1", "cargo", false);
    let selected_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let mut pangine = Pangine::new();
    let old_partition = experience_state(&mut pangine, "root-fold-old", &old_source);
    let selected_partition = experience_state(&mut pangine, "root-fold-selected", &selected_source);
    let replay_partition = experience_state(&mut pangine, "root-fold-replay", &selected_source);
    let audit_partition =
        experience_state(&mut pangine, "root-fold-audit", "?[editor]:{{[agent-memory-snapshot-v1]->[superseded-by]}->[agent-memory-snapshot-v2]}");
    let selected_root = must_reference(&mut pangine, &selected_source);
    let partitions = [&audit_partition, &selected_partition, &old_partition, &replay_partition, &selected_partition];

    let selected_from_partitions =
        partitions.iter().filter_map(|partition| exact_observation_record(&pangine, partition, &selected_root)).collect::<BTreeSet<_>>();
    let merged_records = union_records(&pangine, &partitions);
    let merged_state = reference_observation_state(&mut pangine, &merged_records).unwrap();
    let selected_from_merged = exact_observation_record(&pangine, &merged_state, &selected_root).into_iter().collect::<BTreeSet<_>>();

    assert_eq!(selected_from_partitions, BTreeSet::from([selected_root.clone()]));
    assert_eq!(selected_from_partitions, selected_from_merged);

    let reversed_partitions = [&replay_partition, &old_partition, &audit_partition, &selected_partition];
    let reversed_records = union_records(&pangine, &reversed_partitions);
    assert_eq!(reversed_records, merged_records);

    let degraded_records = union_records(&pangine, &[&old_partition, &audit_partition]);
    let degraded_state = reference_observation_state(&mut pangine, &degraded_records).unwrap();
    assert!(exact_observation_record(&pangine, &degraded_state, &selected_root).is_none());
}

#[test]
fn a_by_value_selection_claim_makes_its_own_target_available() {
    let old_source = agent_memory_snapshot("agent-memory-snapshot-v1", "policy-pangine-full-v1", "cargo", false);
    let selected_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let relay_source = snapshot_claim("untrusted-relay", &selected_source);

    let mut producer = Pangine::new();
    let producer_root = must_reference(&mut producer, &selected_source);
    let locator = producer.format_concept(&producer_root, false);

    let mut pangine = Pangine::new();
    let selected_root = must_reference(&mut pangine, &selected_source);
    let memory = pangine.reference_percept("in-band-selection-memory");
    let old_input = must_reference(&mut pangine, &old_source);
    let prior_state = pangine.perform_experience(&memory, Some(&old_input)).unwrap();
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &prior_state, &locator), Err(LocatorSelectionError::Unavailable));

    let relay_root = must_reference(&mut pangine, &relay_source);
    let state_after_claim = pangine.perform_experience(&memory, Some(&relay_root)).unwrap();
    assert_eq!(exact_observation_record(&pangine, &state_after_claim, &relay_root), Some(relay_root.clone()));
    assert_eq!(exact_observation_record(&pangine, &state_after_claim, &selected_root), Some(selected_root.clone()));
    let relay_payload = pangine.get_observation(&relay_root).unwrap();
    assert_eq!(pangine.get_correlation_a(&relay_payload), Some(selected_root.clone()));
    assert!(pangine.format_concept(&relay_root, false).contains(&locator));

    let relayed_view = exact_observation_view_from_locator(&mut pangine, &state_after_claim, &locator).unwrap();
    let direct_state = experience_state(&mut pangine, "direct-snapshot", &selected_source);
    let direct_payload = exact_observation_payload(&pangine, &direct_state, &selected_root).unwrap();
    assert_eq!(relayed_view.observation, direct_payload);
}

#[test]
fn a_label_only_claim_does_not_identify_a_complete_snapshot_root() {
    let cli_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full-cli", "cli-runner", false);
    let cargo_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full-cargo", "cargo", false);
    let claim_source = "?[reviewer]:{[agent-memory-snapshot]->[preferred-snapshot]}";

    let mut pangine = Pangine::new();
    let claim_state = experience_state(&mut pangine, "label-claim", claim_source);
    let claim_root = must_reference(&mut pangine, claim_source);
    let snapshot_label = must_reference(&mut pangine, "[agent-memory-snapshot]");
    let cli_root = must_reference(&mut pangine, &cli_source);
    let cargo_root = must_reference(&mut pangine, &cargo_source);
    let claim_payload = pangine.get_observation(&claim_root).unwrap();
    assert_eq!(pangine.get_correlation_a(&claim_payload), Some(snapshot_label.clone()));
    assert!(exact_observation_payload(&pangine, &claim_state, &cli_root).is_none());
    assert!(exact_observation_payload(&pangine, &claim_state, &cargo_root).is_none());

    let cli_state = experience_state(&mut pangine, "label-cli-snapshot", &cli_source);
    let cargo_state = experience_state(&mut pangine, "label-cargo-snapshot", &cargo_source);
    let cli_records = union_records(&pangine, &[&claim_state, &cli_state]);
    let cli_available = reference_observation_state(&mut pangine, &cli_records).unwrap();
    assert_eq!(exact_observer_records(&pangine, &cli_available, Some(&snapshot_label)), BTreeSet::from([cli_root.clone()]));

    let both_records = union_records(&pangine, &[&claim_state, &cli_state, &cargo_state]);
    let both_available = reference_observation_state(&mut pangine, &both_records).unwrap();
    assert_eq!(exact_observer_records(&pangine, &both_available, Some(&snapshot_label)), BTreeSet::from([cli_root.clone(), cargo_root.clone()]));

    let cargo_records = union_records(&pangine, &[&claim_state, &cargo_state]);
    let cargo_available = reference_observation_state(&mut pangine, &cargo_records).unwrap();
    assert_eq!(exact_observer_records(&pangine, &cargo_available, Some(&snapshot_label)), BTreeSet::from([cargo_root]));
}

#[test]
fn a_percept_claim_is_live_until_freezing_turns_it_into_by_value_content() {
    let old_source = agent_memory_snapshot("agent-memory-snapshot-v1", "policy-pangine-full-v1", "cargo", false);
    let selected_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let live_claim_source = "?[reviewer]:{['snapshot-pointer']->[preferred-snapshot]}";
    let frozen_claim_expression = "?[reviewer]:{($['snapshot-pointer'])->[preferred-snapshot]}";

    let mut pangine = Pangine::new();
    let old_root = must_reference(&mut pangine, &old_source);
    let selected_root = must_reference(&mut pangine, &selected_source);
    let old_locator = pangine.format_concept(&old_root, false);
    let selected_locator = pangine.format_concept(&selected_root, false);
    must_reference(&mut pangine, &format!("['snapshot-pointer'] = {old_source}"));

    let live_claim_root = must_reference(&mut pangine, live_claim_source);
    let live_canonical = pangine.format_concept(&live_claim_root, false);
    let live_evaluated_before = pangine.format_concept(&live_claim_root, true);
    assert!(contains_percept(&pangine, &live_claim_root));
    assert!(live_evaluated_before.contains(&old_locator));

    let live_state = experience_state(&mut pangine, "live-pointer-claim", live_claim_source);
    assert!(exact_observation_payload(&pangine, &live_state, &old_root).is_none());
    assert!(exact_observation_payload(&pangine, &live_state, &selected_root).is_none());

    let frozen_claim_root = must_reference(&mut pangine, frozen_claim_expression);
    let frozen_source = pangine.format_concept(&frozen_claim_root, false);
    let frozen_evaluated = pangine.format_concept(&frozen_claim_root, true);
    let frozen_payload = pangine.get_observation(&frozen_claim_root).unwrap();
    assert!(!contains_percept(&pangine, &frozen_claim_root));
    assert_eq!(pangine.get_correlation_a(&frozen_payload), Some(old_root.clone()));
    assert!(frozen_source.contains(&old_locator));

    must_reference(&mut pangine, &format!("['snapshot-pointer'] = {selected_source}"));
    assert_eq!(pangine.format_concept(&live_claim_root, false), live_canonical);
    let live_evaluated_after = pangine.format_concept(&live_claim_root, true);
    assert_ne!(live_evaluated_after, live_evaluated_before);
    assert!(live_evaluated_after.contains(&selected_locator));
    assert_eq!(pangine.format_concept(&frozen_claim_root, true), frozen_evaluated);

    let frozen_state = experience_state(&mut pangine, "frozen-pointer-claim", &frozen_source);
    assert!(exact_observation_payload(&pangine, &frozen_state, &old_root).is_some());
    assert!(exact_observation_payload(&pangine, &frozen_state, &selected_root).is_none());
}

#[test]
fn an_exact_by_value_audit_preserves_history_without_controlling_selection() {
    let old_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full-old", "cargo", false);
    let selected_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full-selected", "cli-runner", false);
    let forward_source = snapshot_audit("editor", &old_source, &selected_source);
    let reverse_source = snapshot_audit("editor", &selected_source, &old_source);

    let mut pangine = Pangine::new();
    let old_root = must_reference(&mut pangine, &old_source);
    let selected_root = must_reference(&mut pangine, &selected_source);
    let old_locator = pangine.format_concept(&old_root, false);
    let selected_locator = pangine.format_concept(&selected_root, false);
    let relation = must_reference(&mut pangine, "[superseded-by]");
    let snapshot_label = must_reference(&mut pangine, "[agent-memory-snapshot]");

    let forward_state = experience_state(&mut pangine, "exact-audit-forward", &forward_source);
    let forward_root = must_reference(&mut pangine, &forward_source);
    let forward_payload = pangine.get_observation(&forward_root).unwrap();
    let forward_predecessor_edge = pangine.get_correlation_a(&forward_payload).unwrap();
    assert_eq!(pangine.get_correlation_a(&forward_predecessor_edge), Some(old_root.clone()));
    assert_eq!(pangine.get_correlation_b(&forward_predecessor_edge), Some(relation.clone()));
    assert_eq!(pangine.get_correlation_b(&forward_payload), Some(selected_root.clone()));
    let canonical_forward = pangine.format_concept(&forward_root, false);
    assert!(canonical_forward.contains(&old_locator));
    assert!(canonical_forward.contains(&selected_locator));
    assert_eq!(exact_observer_records(&pangine, &forward_state, Some(&snapshot_label)), BTreeSet::from([old_root.clone(), selected_root.clone()]));

    let forward_selected_payload = exact_observation_payload(&pangine, &forward_state, &selected_root).unwrap();
    let selected_answer =
        ask(&mut pangine, "exact-audit-selected-memory", Some(forward_selected_payload.clone()), "exact-audit-selected-answer", AGENT_POLICY_QUESTION);
    assert_eq!(top_named_candidate(&pangine, selected_answer.as_ref()).as_deref(), Some("cli-runner"));

    let reverse_state = experience_state(&mut pangine, "exact-audit-reverse", &reverse_source);
    let reverse_root = must_reference(&mut pangine, &reverse_source);
    let reverse_payload = pangine.get_observation(&reverse_root).unwrap();
    let reverse_predecessor_edge = pangine.get_correlation_a(&reverse_payload).unwrap();
    assert_eq!(pangine.get_correlation_a(&reverse_predecessor_edge), Some(selected_root.clone()));
    assert_eq!(pangine.get_correlation_b(&reverse_predecessor_edge), Some(relation));
    assert_eq!(pangine.get_correlation_b(&reverse_payload), Some(old_root.clone()));

    let combined_records = union_records(&pangine, &[&forward_state, &reverse_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();
    assert_eq!(exact_observation_record(&pangine, &combined_state, &forward_root), Some(forward_root));
    assert_eq!(exact_observation_record(&pangine, &combined_state, &reverse_root), Some(reverse_root));
    assert_eq!(exact_observation_payload(&pangine, &combined_state, &selected_root), Some(forward_selected_payload));

    let combined_old_payload = exact_observation_payload(&pangine, &combined_state, &old_root).unwrap();
    let old_answer = ask(&mut pangine, "exact-audit-old-memory", Some(combined_old_payload), "exact-audit-old-answer", AGENT_POLICY_QUESTION);
    assert_eq!(top_named_candidate(&pangine, old_answer.as_ref()).as_deref(), Some("cargo"));
}

#[test]
fn a_label_only_audit_cannot_distinguish_same_label_snapshot_roots() {
    let old_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full-old", "cargo", false);
    let selected_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full-selected", "cli-runner", false);
    let audit_source = "?[editor]:{{[agent-memory-snapshot]->[superseded-by]}->[agent-memory-snapshot]}";

    let mut pangine = Pangine::new();
    let audit_state = experience_state(&mut pangine, "label-only-audit", audit_source);
    let audit_root = must_reference(&mut pangine, audit_source);
    let old_root = must_reference(&mut pangine, &old_source);
    let selected_root = must_reference(&mut pangine, &selected_source);
    let snapshot_label = must_reference(&mut pangine, "[agent-memory-snapshot]");
    let relation = must_reference(&mut pangine, "[superseded-by]");

    let audit_payload = pangine.get_observation(&audit_root).unwrap();
    let predecessor_edge = pangine.get_correlation_a(&audit_payload).unwrap();
    assert_eq!(pangine.get_correlation_a(&predecessor_edge), Some(snapshot_label.clone()));
    assert_eq!(pangine.get_correlation_b(&predecessor_edge), Some(relation));
    assert_eq!(pangine.get_correlation_b(&audit_payload), Some(snapshot_label.clone()));
    assert!(exact_observation_payload(&pangine, &audit_state, &old_root).is_none());
    assert!(exact_observation_payload(&pangine, &audit_state, &selected_root).is_none());

    let old_state = experience_state(&mut pangine, "label-audit-old", &old_source);
    let selected_state = experience_state(&mut pangine, "label-audit-selected", &selected_source);
    let combined_records = union_records(&pangine, &[&audit_state, &old_state, &selected_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();
    assert_eq!(exact_observer_records(&pangine, &combined_state, Some(&snapshot_label)), BTreeSet::from([old_root, selected_root]));
}

#[test]
fn ordinary_selection_claims_preserve_disagreement_without_resolving_authority() {
    let cli_source = agent_memory_snapshot("agent-memory-snapshot-cli", "policy-pangine-full-cli", "cli-runner", false);
    let cargo_source = agent_memory_snapshot("agent-memory-snapshot-cargo", "policy-pangine-full-cargo", "cargo", false);
    let cli_claim_source = snapshot_claim("reviewer-a", &cli_source);
    let cargo_claim_source = snapshot_claim("reviewer-b", &cargo_source);

    let mut pangine = Pangine::new();
    let cli_claim_state = experience_state(&mut pangine, "claim-reviewer-a", &cli_claim_source);
    let cargo_claim_state = experience_state(&mut pangine, "claim-reviewer-b", &cargo_claim_source);
    let cli_claim_root = must_reference(&mut pangine, &cli_claim_source);
    let cargo_claim_root = must_reference(&mut pangine, &cargo_claim_source);
    let cli_root = must_reference(&mut pangine, &cli_source);
    let cargo_root = must_reference(&mut pangine, &cargo_source);
    let preferred_marker = must_reference(&mut pangine, "[preferred-snapshot]");

    let cli_claim = pangine.get_observation(&cli_claim_root).unwrap();
    assert_eq!(pangine.get_correlation_a(&cli_claim), Some(cli_root.clone()));
    assert_eq!(pangine.get_correlation_b(&cli_claim), Some(preferred_marker.clone()));
    let cargo_claim = pangine.get_observation(&cargo_claim_root).unwrap();
    assert_eq!(pangine.get_correlation_a(&cargo_claim), Some(cargo_root.clone()));
    assert_eq!(pangine.get_correlation_b(&cargo_claim), Some(preferred_marker));

    let combined_records = union_records(&pangine, &[&cli_claim_state, &cargo_claim_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();
    let cli_payload = exact_observation_payload(&pangine, &combined_state, &cli_root).unwrap();
    let cargo_payload = exact_observation_payload(&pangine, &combined_state, &cargo_root).unwrap();
    let cli_answer = ask(&mut pangine, "claim-cli-memory", Some(cli_payload), "claim-cli-answer", AGENT_POLICY_QUESTION);
    let cargo_answer = ask(&mut pangine, "claim-cargo-memory", Some(cargo_payload), "claim-cargo-answer", AGENT_POLICY_QUESTION);
    assert_eq!(top_named_candidate(&pangine, cli_answer.as_ref()).as_deref(), Some("cli-runner"));
    assert_eq!(top_named_candidate(&pangine, cargo_answer.as_ref()).as_deref(), Some("cargo"));

    assert!(exact_observation_payload(&pangine, &cargo_claim_state, &cli_root).is_none());
    assert!(exact_observation_payload(&pangine, &cargo_claim_state, &cargo_root).is_some());
}

#[test]
fn exact_roots_identify_structure_not_identical_revision_occurrences() {
    let snapshot_source = agent_memory_snapshot("agent-memory-snapshot", "policy-pangine-full", "cargo", false);
    let audit_source = snapshot_audit("editor", &snapshot_source, &snapshot_source);

    let mut pangine = Pangine::new();
    let first_partition = experience_state(&mut pangine, "structural-identity-first", &snapshot_source);
    let replay_partition = experience_state(&mut pangine, "structural-identity-replay", &snapshot_source);
    let snapshot_root = must_reference(&mut pangine, &snapshot_source);
    let combined_records = union_records(&pangine, &[&first_partition, &replay_partition]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();

    assert_eq!(observation_records(&pangine, &first_partition), observation_records(&pangine, &replay_partition));
    assert_eq!(exact_observation_record(&pangine, &combined_state, &snapshot_root), Some(snapshot_root.clone()));

    let audit_root = must_reference(&mut pangine, &audit_source);
    let audit_payload = pangine.get_observation(&audit_root).unwrap();
    let predecessor_edge = pangine.get_correlation_a(&audit_payload).unwrap();
    assert_eq!(pangine.get_correlation_a(&predecessor_edge), Some(snapshot_root.clone()));
    assert_eq!(pangine.get_correlation_b(&audit_payload), Some(snapshot_root));
}

#[test]
fn explicit_ordinary_identity_distinguishes_revisions_with_equal_payloads() {
    let first_source = agent_memory_snapshot("agent-memory-snapshot-v1", "policy-pangine-full", "cargo", false);
    let restored_source = agent_memory_snapshot("agent-memory-snapshot-v3", "policy-pangine-full", "cargo", false);
    let audit_source = snapshot_audit("editor", &first_source, &restored_source);

    let mut pangine = Pangine::new();
    let first_root = must_reference(&mut pangine, &first_source);
    let restored_root = must_reference(&mut pangine, &restored_source);
    assert_ne!(first_root, restored_root);
    assert_eq!(pangine.get_observation(&first_root), pangine.get_observation(&restored_root));

    let first_partition = experience_state(&mut pangine, "occurrence-identity-first", &first_source);
    let restored_partition = experience_state(&mut pangine, "occurrence-identity-restored", &restored_source);
    let combined_records = union_records(&pangine, &[&first_partition, &restored_partition]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();
    assert_eq!(exact_observation_record(&pangine, &combined_state, &first_root), Some(first_root.clone()));
    assert_eq!(exact_observation_record(&pangine, &combined_state, &restored_root), Some(restored_root.clone()));

    let audit_root = must_reference(&mut pangine, &audit_source);
    let audit_payload = pangine.get_observation(&audit_root).unwrap();
    let predecessor_edge = pangine.get_correlation_a(&audit_payload).unwrap();
    assert_eq!(pangine.get_correlation_a(&predecessor_edge), Some(first_root));
    assert_eq!(pangine.get_correlation_b(&audit_payload), Some(restored_root));
}

#[test]
fn relevance_wrapped_mentions_neither_supply_nor_retract_the_positive_snapshot_root() {
    let snapshot_source = agent_memory_snapshot("agent-memory-snapshot-v2", "policy-pangine-full-v2", "cli-runner", false);
    let inverted_claim_source = format!("?[reviewer-inverted]:{{(!({snapshot_source}))->[mentioned-snapshot]}}");
    let weighted_claim_source = format!("?[reviewer-weighted]:{{(x2({snapshot_source}))->[mentioned-snapshot]}}");

    let mut producer = Pangine::new();
    let producer_root = must_reference(&mut producer, &snapshot_source);
    let locator = producer.format_concept(&producer_root, false);

    let mut pangine = Pangine::new();
    let snapshot_root = must_reference(&mut pangine, &snapshot_source);
    let inverted_claim_state = experience_state(&mut pangine, "relevance-inverted-claim", &inverted_claim_source);
    let weighted_claim_state = experience_state(&mut pangine, "relevance-weighted-claim", &weighted_claim_source);
    let inverted_claim_root = must_reference(&mut pangine, &inverted_claim_source);
    let weighted_claim_root = must_reference(&mut pangine, &weighted_claim_source);

    assert_eq!(exact_observation_record(&pangine, &inverted_claim_state, &inverted_claim_root), Some(inverted_claim_root));
    assert_eq!(exact_observation_record(&pangine, &weighted_claim_state, &weighted_claim_root), Some(weighted_claim_root));
    assert!(exact_observation_record(&pangine, &inverted_claim_state, &snapshot_root).is_none());
    assert!(exact_observation_record(&pangine, &weighted_claim_state, &snapshot_root).is_none());

    let mention_records = union_records(&pangine, &[&inverted_claim_state, &weighted_claim_state]);
    let mention_state = reference_observation_state(&mut pangine, &mention_records).unwrap();
    assert_eq!(exact_observation_view_from_locator(&mut pangine, &mention_state, &locator), Err(LocatorSelectionError::Unavailable));

    let positive_state = experience_state(&mut pangine, "relevance-positive-snapshot", &snapshot_source);
    let positive_payload = exact_observation_payload(&pangine, &positive_state, &snapshot_root).unwrap();
    let combined_records = union_records(&pangine, &[&positive_state, &inverted_claim_state, &weighted_claim_state]);
    let combined_state = reference_observation_state(&mut pangine, &combined_records).unwrap();
    let combined_view = exact_observation_view_from_locator(&mut pangine, &combined_state, &locator).unwrap();
    assert_eq!(combined_view.observation, positive_payload);
}
