//! Focused warning checks for decisions represented in Pangine state.
//!
//! These programs keep the main counterexamples executable without preserving
//! every intermediate decision pipeline explored along the way.

use pangine::{ConceptId, Pangine, Relevance};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
struct DecisionResult {
    candidates: BTreeMap<String, Relevance>,
    selected: Option<String>,
}

#[test]
#[ignore = "warning: additive experience and deterministic choice remain provisional"]
fn matching_experience_shapes_pangines_choice() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "memory", "[withdraw-one]->[at]->[hand]->[suggests]->[withdraw]", 2);
    remember(&mut pangine, "memory", "[continue-one]->[at]->[hand]->[suggests]->[continue]", 1);
    remember(&mut pangine, "memory", "[noise]->[at]->[foot]->[suggests]->[continue]", 20);

    ask(&mut pangine, "['memory']", "['event']->[at]->[hand]->[suggests]->['choice']");
    assert_decision(&mut pangine, "choice", &[("continue", 1), ("withdraw", 2)], Some("withdraw"));

    remember(&mut pangine, "memory", "[continue-two]->[at]->[hand]->[suggests]->[continue]", 1);
    remember(&mut pangine, "memory", "[continue-three]->[at]->[hand]->[suggests]->[continue]", 1);
    ask(&mut pangine, "['memory']", "['event']->[at]->[hand]->[suggests]->['choice']");
    assert_decision(&mut pangine, "choice", &[("continue", 3), ("withdraw", 2)], Some("continue"));
}

#[test]
#[ignore = "warning: event history and replaceable state are application-provided representations, not universal sensor semantics"]
fn event_history_and_current_state_remain_different() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "history", "[a-event]->[A]", 3);
    for event in ["b-one", "b-two", "b-three"] {
        remember(&mut pangine, "history", &format!("[{event}]->[B]"), 1);
    }

    for _ in 0..3 {
        must_ref(&mut pangine, "['current-a'] = [a-event]->[A]");
    }
    for (percept, event) in [("current-b-one", "b-one"), ("current-b-two", "b-two"), ("current-b-three", "b-three")] {
        must_ref(&mut pangine, &format!("['{percept}'] = [{event}]->[B]"));
    }

    ask(&mut pangine, "['history']", "['event']->['choice']");
    assert_decision(&mut pangine, "choice", &[("A", 3), ("B", 3)], Some("A"));

    ask(&mut pangine, "['current-a']['current-b-one']['current-b-two']['current-b-three']", "['event']->['choice']");
    assert_decision(&mut pangine, "choice", &[("A", 1), ("B", 3)], Some("B"));
}

#[test]
#[ignore = "warning: represented context is one explicit filter, not a universal eligibility rule"]
fn represented_context_filters_before_choice() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "reports", "[north-a]->[in]->[north]->[suggests]->[A]", 2);
    remember(&mut pangine, "reports", "[north-b]->[in]->[north]->[suggests]->[B]", 1);
    remember(&mut pangine, "reports", "[south-c]->[in]->[south]->[suggests]->[C]", 20);
    let question = "(['report']->[in]->['zone']->[suggests]->['choice'])([request]->[in]->['zone'])";

    must_ref(&mut pangine, "['context'] = [request]->[in]->[north]");
    ask(&mut pangine, "['reports']['context']", question);
    assert_decision(&mut pangine, "choice", &[("A", 2), ("B", 1)], Some("A"));

    must_ref(&mut pangine, "['context'] = [request]->[in]->[south]");
    ask(&mut pangine, "['reports']['context']", question);
    assert_decision(&mut pangine, "choice", &[("C", 20)], Some("C"));
}

#[test]
#[ignore = "warning: represented stance controls one explicit two-stream program, not a universal sign rule"]
fn represented_stance_routes_support_and_counterevidence() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "body", "[support-a]->[supports]->[A]", 4);
    remember(&mut pangine, "body", "[support-b]->[supports]->[B]", 1);
    remember(&mut pangine, "body", "[counter-a]->[opposes]->[A]", 1);
    remember(&mut pangine, "body", "[counter-b]->[opposes]->[B]", 3);

    set_stance(&mut pangine, "supports", "opposes");
    ask_stance(&mut pangine);
    assert_decision(&mut pangine, "net-choice", &[("A", 3), ("B", -2)], Some("A"));

    set_stance(&mut pangine, "opposes", "supports");
    ask_stance(&mut pangine);
    assert_decision(&mut pangine, "net-choice", &[("A", -3), ("B", 2)], Some("B"));
}

#[test]
#[ignore = "warning: report-first and event-first questions preserve different experience and neither is universal"]
fn represented_experience_can_select_between_question_orders() {
    let mut pangine = Pangine::new();
    for (report, event, choice) in [("r1", "event-x", "A"), ("r2", "event-x", "A"), ("r3", "event-y", "B"), ("r4", "event-z", "B"), ("r5", "event-z", "B")] {
        remember(&mut pangine, "reports", &format!("[{report}]->[reported]->[{event}]->[suggests]->[{choice}]"), 1);
    }

    ask(&mut pangine, "['reports']", "['report']->[reported]->['event']->[suggests]->['report-choice']");
    assert_decision(&mut pangine, "report-choice", &[("A", 2), ("B", 3)], Some("B"));

    ask(&mut pangine, "['reports']", "['event-report']->[reported]->['popular-event']");
    must_ref(&mut pangine, "['selected-event'] = ^['popular-event']");
    ask(&mut pangine, "['reports']", "['chosen-report']->[reported]->$['selected-event']->[suggests]->['event-choice']");
    assert_decision(&mut pangine, "event-choice", &[("A", 2)], Some("A"));

    must_ref(&mut pangine, "['report-result'] = ^['report-choice']");
    must_ref(&mut pangine, "['event-result'] = ^['event-choice']");
    must_ref(&mut pangine, "['views'] ~= [report-first]->[chooses]->['report-result']");
    must_ref(&mut pangine, "['views'] ~= [event-first]->[chooses]->['event-result']");
    remember(&mut pangine, "guidance", "[north]->[prefers]->[event-first]", 1);
    remember(&mut pangine, "guidance", "[south]->[prefers]->[report-first]", 1);

    ask(&mut pangine, "['guidance']['views']", "([north]->[prefers]->['view'])(['view']->[chooses]->['guided-choice'])");
    assert_decision(&mut pangine, "guided-choice", &[("A", 1)], Some("A"));
    ask(&mut pangine, "['guidance']['views']", "([south]->[prefers]->['view'])(['view']->[chooses]->['guided-choice'])");
    assert_decision(&mut pangine, "guided-choice", &[("B", 1)], Some("B"));
}

#[test]
#[ignore = "warning: represented source identity is one explicit provenance filter"]
fn represented_source_identity_keeps_overlapping_reports_separate() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "reports-a", "[shared-report]->[from]->[store-a]->[choice]->[A]", 2);
    remember(&mut pangine, "reports-b", "[shared-report]->[from]->[store-b]->[choice]->[B]", 3);
    let question = "(['report']->[from]->['store']->[choice]->['choice'])([active]->['store'])";

    must_ref(&mut pangine, "['active-store'] = [active]->[store-a]");
    ask(&mut pangine, "['reports-a']['reports-b']['active-store']", question);
    assert_decision(&mut pangine, "choice", &[("A", 2)], Some("A"));

    must_ref(&mut pangine, "['active-store'] = [active]->[store-b]");
    ask(&mut pangine, "['reports-a']['reports-b']['active-store']", question);
    assert_decision(&mut pangine, "choice", &[("B", 3)], Some("B"));
}

#[test]
#[ignore = "warning: ordinary decision records preserve one chosen view, not every reason behind it"]
fn ordinary_records_keep_an_old_choice_after_live_experience_changes() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "memory", "[a-one]->[A]", 2);
    remember(&mut pangine, "memory", "[b-one]->[B]", 1);
    ask(&mut pangine, "['memory']", "['event']->['choice']");
    must_ref(&mut pangine, "['selected'] = ^['choice']");
    must_ref(&mut pangine, "['records'] ~= [run-one]->[selected]->['selected']");

    for event in ["b-two", "b-three", "b-four"] {
        remember(&mut pangine, "memory", &format!("[{event}]->[B]"), 1);
    }
    ask(&mut pangine, "['memory']", "['event']->['choice']");
    assert_decision(&mut pangine, "choice", &[("A", 2), ("B", 4)], Some("B"));

    ask(&mut pangine, "['records']", "[run-one]->[selected]->['recorded-choice']");
    assert_decision(&mut pangine, "recorded-choice", &[("A", 1)], Some("A"));
}

#[test]
#[ignore = "warning: coefficient structure and repeated experience remain distinct decision inputs"]
fn coefficient_structure_does_not_become_experience_count() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "memory", "[event-a]->[choice]->[A]", 3);
    remember(&mut pangine, "memory", "x3([event-b]->[choice]->[B])", 1);

    ask(&mut pangine, "['memory']", "['event']->[choice]->['choice']");
    assert_decision(&mut pangine, "choice", &[("A", 3), ("B", 1)], Some("A"));

    ask(&mut pangine, "['memory']", "x3(['weighted-event']->[choice]->['weighted-choice'])");
    assert_decision(&mut pangine, "weighted-choice", &[("B", 1)], Some("B"));
}

fn set_stance(pangine: &mut Pangine, positive: &str, negative: &str) {
    must_ref(pangine, &format!("['stance'] = ([current]->[positive]->[{positive}])([current]->[negative]->[{negative}])"));
}

fn ask_stance(pangine: &mut Pangine) {
    ask(pangine, "['body']['stance']", "(['positive-event']->['positive-relation']->['positive-choice'])([current]->[positive]->['positive-relation'])");
    ask(pangine, "['body']['stance']", "(['negative-event']->['negative-relation']->['negative-choice'])([current]->[negative]->['negative-relation'])");
    must_ref(pangine, "['net-choice'] = $['positive-choice']");
    must_ref(pangine, "['net-choice'] /= $['negative-choice']");
}

fn ask(pangine: &mut Pangine, sources: &str, question: &str) {
    must_ref(pangine, &format!("{sources} @ {question}"));
}

fn remember(pangine: &mut Pangine, percept: &str, concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        must_ref(pangine, &format!("['{percept}'] ~= {concept}"));
    }
}

#[track_caller]
fn assert_decision(pangine: &mut Pangine, percept: &str, expected: &[(&str, i64)], selected: Option<&str>) {
    assert_eq!(
        read_decision(pangine, percept),
        DecisionResult {
            candidates: expected.iter().map(|(name, weight)| (format!("[{name}]"), Relevance::new(*weight))).collect(),
            selected: selected.map(|name| format!("[{name}]")),
        }
    );
}

fn read_decision(pangine: &mut Pangine, percept: &str) -> DecisionResult {
    let value = must_run(pangine, &format!("$['{percept}']"));
    let candidates = value
        .iter()
        .flat_map(|value| pangine.get_relevance_map(value))
        .map(|(relevance, candidate)| (pangine.format_concept(&candidate, false), relevance))
        .collect();

    let probe = pangine.reference_percept("represented-choice-probe");
    assert!(pangine.set_percept_value(&probe, value));
    let selected = must_run(pangine, "^['represented-choice-probe']").map(|candidate| pangine.format_concept(&candidate, false));
    DecisionResult { candidates, selected }
}

fn must_run(pangine: &mut Pangine, input: &str) -> Option<ConceptId> {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    must_run(pangine, input).unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
