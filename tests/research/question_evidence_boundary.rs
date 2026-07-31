//! Research-only characterization of the information lost by current questions.
//!
//! Current `@` results are structural candidate weights. They do not retain the
//! exact source records needed by a replay-safe recursive probability policy,
//! and the output Percept does not declare a candidate frame.

use std::collections::BTreeSet;

use pangine::{ConceptId, Pangine};

const WORKDAY_TEA: &str = "{{{[choice]->[morning]}->[workday]}->[tea]}";
const WORKDAY_QUESTION: &str = "{{{[choice]->[morning]}->[workday]}->['answer']}";

fn reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a Concept from {source:?}"))
}

fn experience(pangine: &mut Pangine, memory: &str, occurrence: &str) {
    reference(pangine, &format!("['{memory}'] ~= ?[{occurrence}]:{WORKDAY_TEA}"));
}

fn question(pangine: &mut Pangine, memory: &str, answer: &str) -> ConceptId {
    reference(pangine, &format!("['{memory}'] @ {}", WORKDAY_QUESTION.replace("['answer']", &format!("['{answer}']"))));
    reference(pangine, &format!("$['{answer}']"))
}

#[test]
fn collapsed_partition_answers_cannot_distinguish_replay_from_independent_occurrences() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "partition-a", "event-1");
    experience(&mut pangine, "partition-b", "event-1");
    experience(&mut pangine, "partition-c", "event-2");

    experience(&mut pangine, "replayed", "event-1");
    experience(&mut pangine, "replayed", "event-1");
    experience(&mut pangine, "independent", "event-1");
    experience(&mut pangine, "independent", "event-2");

    let partition_a = question(&mut pangine, "partition-a", "answer-a");
    let partition_b = question(&mut pangine, "partition-b", "answer-b");
    let partition_c = question(&mut pangine, "partition-c", "answer-c");
    let replayed = question(&mut pangine, "replayed", "answer-replayed");
    let independent = question(&mut pangine, "independent", "answer-independent");

    assert_eq!(partition_a, partition_b);
    assert_eq!(partition_a, partition_c);
    assert_eq!(replayed, partition_a);
    assert_ne!(independent, replayed);
    assert!(!pangine.format_concept(&partition_a, false).contains("event-"));
    assert!(!pangine.format_concept(&independent, false).contains("event-"));
}

#[test]
fn an_output_percept_does_not_supply_a_closed_candidate_frame() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "event-1");
    let answer = question(&mut pangine, "memory", "answer");
    let names =
        pangine.get_relevance_map(&answer).into_iter().filter_map(|(_, candidate)| pangine.get_name(&candidate).map(str::to_owned)).collect::<BTreeSet<_>>();

    assert_eq!(names, BTreeSet::from(["morning".to_owned(), "tea".to_owned(), "workday".to_owned()]));
}

#[test]
fn ordinary_source_observations_preserve_replay_without_nested_match_records() {
    let mut pangine = Pangine::new();
    let source_a_text = format!("?[camera]:{{[event-1]->{WORKDAY_TEA}}}");
    let source_b_text = format!("?[camera]:{{[event-2]->{WORKDAY_TEA}}}");
    let source_a = reference(&mut pangine, &source_a_text);
    let source_b = reference(&mut pangine, &source_b_text);
    let camera = reference(&mut pangine, "[camera]");

    assert_eq!(pangine.get_observer(&source_a), Some(camera.clone()));
    assert_eq!(pangine.get_observer(&source_b), Some(camera));
    assert_ne!(source_a, source_b);

    let replayed = reference(&mut pangine, &format!("<{source_a_text}, {source_a_text}>"));
    let independent = reference(&mut pangine, &format!("<{source_a_text}, {source_b_text}>"));
    assert_eq!(replayed, source_a);
    assert_ne!(independent, replayed);
}
