//! Warning checks for Pangine-native decision-time records.
//!
//! These fixtures compare ordinary snapshots that preserve different parts of
//! one represented-stance decision. They do not establish a tracing format or
//! authorize a host to reconstruct Pangine's reasoning from completion data.

use pangine::{ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, PartialEq, Eq)]
struct DecisionState {
    candidates: BTreeMap<String, Relevance>,
    selected: Option<String>,
}

#[test]
#[ignore = "warning: current snapshot shapes preserve different parts of a decision-time record"]
fn net_totals_rows_and_source_values_have_distinct_record_capabilities() {
    let mut pangine = decision_fixture();
    run_stance_program(&mut pangine, "['archive']['decision-stance']", "live");
    assert_eq!(decision_state(&mut pangine, "live-net"), state(&[("[A]", 1), ("[B]", -2)], Some("[A]")));

    capture_separate_records(&mut pangine);
    experience(&mut pangine, "archive", "[later-amber-b]->[amber]->[B]", 10);
    run_stance_program(&mut pangine, "['archive']['decision-stance']", "live");
    assert_eq!(decision_state(&mut pangine, "live-net"), state(&[("[A]", 1), ("[B]", 8)], Some("[B]")));

    assert_eq!(decision_state(&mut pangine, "net-record"), state(&[("[A]", 1), ("[B]", -2)], Some("[A]")));
    assert!(history_completions(&mut pangine, &["net-record"]).is_empty(), "the saved net has no event or role relationships");

    replay_totals(&mut pangine, "positive-record", "negative-record", "totals-replay");
    assert_eq!(decision_state(&mut pangine, "totals-replay"), state(&[("[A]", 1), ("[B]", -2)], Some("[A]")));
    assert!(history_completions(&mut pangine, &["positive-record", "negative-record"]).is_empty(), "saved totals have no event or role relationships");

    let row_histories = history_completions(&mut pangine, &["row-record"]);
    assert_eq!(row_histories.keys().cloned().collect::<BTreeSet<_>>(), original_events());
    assert!(row_histories.values().all(|history| history.source_relevance == Relevance::DEFAULT && history.coefficient.is_none()));
    run_stance_program(&mut pangine, "['row-record']", "rows");
    assert_eq!(decision_state(&mut pangine, "rows-positive"), state(&[("[A]", 1), ("[B]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "rows-negative"), state(&[("[A]", 1), ("[B]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "rows-net"), state(&[], None));

    let source_histories = history_completions(&mut pangine, &["source-record", "stance-record"]);
    assert_eq!(source_histories.keys().cloned().collect::<BTreeSet<_>>(), original_events());
    assert_eq!(
        source_histories.iter().map(|(event, history)| (event.clone(), history.coefficient.expect("source snapshot coefficient"))).collect::<BTreeMap<_, _>>(),
        source_coefficients()
    );
    assert!(source_histories.values().all(|history| history.source_relevance == Relevance::DEFAULT));
    run_stance_program(&mut pangine, "['source-record']['stance-record']", "source");
    assert_eq!(decision_state(&mut pangine, "source-positive"), state(&[("[A]", 1), ("[B]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "source-negative"), state(&[("[A]", 1), ("[B]", 1)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "source-net"), state(&[], None));

    let known_count_question = must_ref(&mut pangine, "x4(['known-event']->['known-relation']->['known-choice'])");
    let source_record = pangine.reference_percept("source-record");
    let known_count =
        pangine.complete_question(std::slice::from_ref(&source_record), &known_count_question).expect("valid exact snapshot coefficient question");
    let [known_completion] = known_count.completions() else {
        panic!("only the x4 source should answer the exact x4 question");
    };
    let known_event = pangine.reference_percept("known-event");
    let known_choice = pangine.reference_percept("known-choice");
    assert_eq!(pangine.format_concept(known_completion.binding(&known_event).expect("known-count event"), false), "[amber-a]");
    assert_eq!(pangine.format_concept(known_completion.binding(&known_choice).expect("known-count choice"), false), "[A]");
}

#[test]
#[ignore = "warning: the combined record is useful but has no generic Pangine binding for an unknown source coefficient"]
fn one_combined_record_replays_and_rejoins_without_becoming_a_complete_explanation_format() {
    let mut pangine = decision_fixture();
    run_stance_program(&mut pangine, "['archive']['decision-stance']", "live");
    capture_separate_records(&mut pangine);
    capture_combined_record(&mut pangine);

    experience(&mut pangine, "archive", "[later-amber-b]->[amber]->[B]", 10);
    run_stance_program(&mut pangine, "['archive']['decision-stance']", "live");
    assert_eq!(decision_state(&mut pangine, "live-net"), state(&[("[A]", 1), ("[B]", 8)], Some("[B]")));

    for (label, output) in [
        ("positive", "saved-positive"),
        ("negative", "saved-negative"),
        ("net", "saved-net"),
        ("rows", "saved-rows"),
        ("source", "saved-source"),
        ("stance", "saved-stance"),
    ] {
        extract_record_member(&mut pangine, label, output);
    }

    assert_eq!(percept_value(&mut pangine, "saved-positive"), percept_value(&mut pangine, "positive-record"));
    assert_eq!(percept_value(&mut pangine, "saved-negative"), percept_value(&mut pangine, "negative-record"));
    assert_eq!(percept_value(&mut pangine, "saved-net"), percept_value(&mut pangine, "net-record"));
    assert_eq!(percept_value(&mut pangine, "saved-rows"), percept_value(&mut pangine, "row-record"));
    assert_eq!(percept_value(&mut pangine, "saved-source"), percept_value(&mut pangine, "source-record"));
    assert_eq!(percept_value(&mut pangine, "saved-stance"), percept_value(&mut pangine, "stance-record"));

    replay_totals(&mut pangine, "saved-positive", "saved-negative", "combined-replay");
    assert_eq!(decision_state(&mut pangine, "combined-replay"), state(&[("[A]", 1), ("[B]", -2)], Some("[A]")));
    assert_eq!(decision_state(&mut pangine, "saved-net"), state(&[("[A]", 1), ("[B]", -2)], Some("[A]")));

    let saved_rows = history_completions(&mut pangine, &["saved-rows"]);
    assert_eq!(saved_rows.keys().cloned().collect::<BTreeSet<_>>(), original_events());
    assert!(saved_rows.values().all(|history| history.coefficient.is_none()));

    let saved_source = history_completions(&mut pangine, &["saved-source", "saved-stance"]);
    assert_eq!(saved_source.keys().cloned().collect::<BTreeSet<_>>(), original_events());
    assert_eq!(
        saved_source.iter().map(|(event, history)| (event.clone(), history.coefficient.expect("combined source coefficient"))).collect::<BTreeMap<_, _>>(),
        source_coefficients()
    );
    run_stance_program(&mut pangine, "['saved-source']['saved-stance']", "combined-source");
    assert_eq!(decision_state(&mut pangine, "combined-source-net"), state(&[], None));
}

#[test]
#[ignore = "warning: a record can select one unchanged source reference through the current decision placeholder"]
fn an_unchanged_source_version_preserves_exact_history_and_can_be_selected_through_a_record() {
    let mut pangine = decision_fixture();
    run_stance_program(&mut pangine, "['archive']['decision-stance']", "decision");
    assert_eq!(decision_state(&mut pangine, "decision-net"), state(&[("[A]", 1), ("[B]", -2)], Some("[A]")));

    must_ref(&mut pangine, "['source-pointer-record'] = ([decision]->[source]->['archive'])([decision]->[stance]->['decision-stance'])");
    must_ref(&mut pangine, "['source-pointer-record'] @ [decision]->[source]->['record-source']");
    must_ref(&mut pangine, "['source-pointer-record'] @ [decision]->[stance]->['record-stance']");

    experience(&mut pangine, "later-archive", "[later-amber-b]->[amber]->[B]", 10);
    run_stance_program(&mut pangine, "['archive']['later-archive']['decision-stance']", "current");
    assert_eq!(decision_state(&mut pangine, "current-net"), state(&[("[A]", 1), ("[B]", 8)], Some("[B]")));

    run_stance_program(&mut pangine, "['archive']['decision-stance']", "version-replay");
    assert_eq!(decision_state(&mut pangine, "version-replay-net"), state(&[("[A]", 1), ("[B]", -2)], Some("[A]")));
    let version_histories = history_completions(&mut pangine, &["archive", "decision-stance"]);
    assert_eq!(version_histories.keys().cloned().collect::<BTreeSet<_>>(), original_events());
    assert_eq!(version_histories.iter().map(|(event, history)| (event.clone(), history.source_relevance)).collect::<BTreeMap<_, _>>(), source_coefficients());
    assert!(version_histories.values().all(|history| history.coefficient.is_none()));

    let archive = pangine.reference_percept("archive");
    assert_eq!(percept_value(&mut pangine, "record-source"), Some(archive));
    let decision_stance = pangine.reference_percept("decision-stance");
    assert_eq!(percept_value(&mut pangine, "record-stance"), Some(decision_stance));
    run_stance_program(&mut pangine, "['record-source']['record-stance']", "indirect");
    assert_eq!(decision_state(&mut pangine, "indirect-positive"), state(&[], None));
    assert_eq!(decision_state(&mut pangine, "indirect-negative"), state(&[], None));
    assert_eq!(decision_state(&mut pangine, "indirect-net"), state(&[], None));

    run_stance_program(&mut pangine, "^['record-source']^['record-stance']", "record-selected");
    assert_eq!(decision_state(&mut pangine, "record-selected-net"), state(&[("[A]", 1), ("[B]", -2)], Some("[A]")));

    must_ref(&mut pangine, "($['record-source']) @ ['evaluated-event']->['evaluated-relation']->['evaluated-choice']");
    assert_eq!(decision_state(&mut pangine, "evaluated-choice"), state(&[("[A]", 1), ("[B]", 1)], Some("[A]")));
    run_stance_program(&mut pangine, "($['record-source'])($['record-stance'])", "evaluated-pointer");
    assert_eq!(decision_state(&mut pangine, "evaluated-pointer-positive"), state(&[], None));
    assert_eq!(decision_state(&mut pangine, "evaluated-pointer-negative"), state(&[], None));
    assert_eq!(decision_state(&mut pangine, "evaluated-pointer-net"), state(&[], None));
}

#[derive(Debug, PartialEq, Eq)]
struct HistoryRecord {
    choice: String,
    role: String,
    source_relevance: Relevance,
    coefficient: Option<Relevance>,
}

fn decision_fixture() -> Pangine {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "archive", "[amber-a]->[amber]->[A]", 4);
    experience(&mut pangine, "archive", "[amber-b]->[amber]->[B]", 3);
    experience(&mut pangine, "archive", "[violet-a]->[violet]->[A]", 3);
    experience(&mut pangine, "archive", "[violet-b]->[violet]->[B]", 5);
    must_ref(&mut pangine, "['decision-stance'] = ([amber]->[role]->[positive])([violet]->[role]->[negative])");
    pangine
}

fn run_stance_program(pangine: &mut Pangine, selector: &str, prefix: &str) {
    let positive = format!("{prefix}-positive");
    let negative = format!("{prefix}-negative");
    let net = format!("{prefix}-net");
    let input = format!(
        "{selector} @ (['{prefix}-positive-event']->['{prefix}-positive-relation']->['{positive}'])(['{prefix}-positive-relation']->[role]->[positive]);
         {selector} @ (['{prefix}-negative-event']->['{prefix}-negative-relation']->['{negative}'])(['{prefix}-negative-relation']->[role]->[negative]);
         ['{net}'] = $['{positive}'];
         ['{net}'] /= $['{negative}']"
    );
    pangine.reference_concept(&input).unwrap_or_else(|error| panic!("failed to run represented stance program: {error}"));
}

fn capture_separate_records(pangine: &mut Pangine) {
    must_ref(pangine, "['net-record'] = $['live-net']");
    must_ref(pangine, "['positive-record'] = $['live-positive']");
    must_ref(pangine, "['negative-record'] = $['live-negative']");
    must_ref(pangine, "['source-record'] = $['archive']");
    must_ref(pangine, "['stance-record'] = $['decision-stance']");
    let history_question = "(['record-event']->['record-relation']->['record-choice'])(['record-relation']->[role]->['record-role'])";
    must_ref(pangine, &format!("['row-record'] = ['archive']['decision-stance'] @ {history_question}"));
}

fn capture_combined_record(pangine: &mut Pangine) {
    must_ref(
        pangine,
        "['decision-record'] =
           ([decision]->[positive]->$['positive-record'])
           ([decision]->[negative]->$['negative-record'])
           ([decision]->[net]->$['net-record'])
           ([decision]->[rows]->$['row-record'])
           ([decision]->[source]->$['source-record'])
           ([decision]->[stance]->$['stance-record'])",
    );
}

fn extract_record_member(pangine: &mut Pangine, label: &str, output: &str) {
    must_ref(pangine, &format!("['decision-record'] @ [decision]->[{label}]->['{output}']"));
}

fn replay_totals(pangine: &mut Pangine, positive: &str, negative: &str, output: &str) {
    let input = format!("['{output}'] = $['{positive}']; ['{output}'] /= $['{negative}']");
    pangine.reference_concept(&input).unwrap_or_else(|error| panic!("failed to replay saved totals: {error}"));
}

fn history_completions(pangine: &mut Pangine, source_names: &[&str]) -> BTreeMap<String, HistoryRecord> {
    let sources = source_names.iter().map(|name| pangine.reference_percept(name)).collect::<Vec<_>>();
    let event = pangine.reference_percept("history-event");
    let choice = pangine.reference_percept("history-choice");
    let role = pangine.reference_percept("history-role");
    let question = must_ref(pangine, "(['history-event']->['history-relation']->['history-choice'])(['history-relation']->[role]->['history-role'])");
    pangine
        .complete_question(&sources, &question)
        .expect("valid decision-record history question")
        .completions()
        .iter()
        .map(|completion| {
            let event_value = completion.binding(&event).expect("history event");
            let source_evidence = completion
                .evidence()
                .iter()
                .find(|evidence| evidence.binding(&event).is_some() && evidence.binding(&choice).is_some())
                .expect("history source evidence");
            let ancestors = source_evidence.coefficient_ancestors().collect::<Vec<_>>();
            let coefficient = match ancestors.as_slice() {
                [] => None,
                [ancestor] => {
                    let entries = pangine.get_relevance_map(ancestor);
                    let [(coefficient, _)] = entries.as_slice() else {
                        panic!("coefficient ancestor should contain one weighted operand");
                    };
                    Some(*coefficient)
                }
                _ => panic!("one source row should cross at most one coefficient boundary"),
            };
            (
                pangine.format_concept(event_value, false),
                HistoryRecord {
                    choice: pangine.format_concept(completion.binding(&choice).expect("history choice"), false),
                    role: pangine.format_concept(completion.binding(&role).expect("history role"), false),
                    source_relevance: source_evidence.source_relevance(),
                    coefficient,
                },
            )
        })
        .collect()
}

fn original_events() -> BTreeSet<String> {
    ["[amber-a]", "[amber-b]", "[violet-a]", "[violet-b]"].into_iter().map(str::to_owned).collect()
}

fn source_coefficients() -> BTreeMap<String, Relevance> {
    BTreeMap::from([
        ("[amber-a]".to_owned(), Relevance::new(4)),
        ("[amber-b]".to_owned(), Relevance::new(3)),
        ("[violet-a]".to_owned(), Relevance::new(3)),
        ("[violet-b]".to_owned(), Relevance::new(5)),
    ])
}

fn decision_state(pangine: &mut Pangine, name: &str) -> DecisionState {
    let percept = pangine.reference_percept(name);
    let candidates = pangine
        .get_value(&percept)
        .into_iter()
        .flat_map(|value| pangine.get_relevance_map(&value))
        .map(|(relevance, candidate)| (pangine.format_concept(&candidate, false), relevance))
        .collect();
    let selected =
        pangine.reference_concept(&format!("^['{name}']")).expect("valid decision-record choice").map(|candidate| pangine.format_concept(&candidate, false));
    DecisionState { candidates, selected }
}

fn percept_value(pangine: &mut Pangine, name: &str) -> Option<ConceptId> {
    let percept = pangine.reference_percept(name);
    pangine.get_value(&percept)
}

fn state(entries: &[(&str, i64)], selected: Option<&str>) -> DecisionState {
    DecisionState {
        candidates: entries.iter().map(|(candidate, relevance)| ((*candidate).to_owned(), Relevance::new(*relevance))).collect(),
        selected: selected.map(str::to_owned),
    }
}

fn experience(pangine: &mut Pangine, percept: &str, concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        let input = format!("['{percept}'] ~= {concept}");
        pangine
            .reference_concept(&input)
            .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
            .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"));
    }
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
