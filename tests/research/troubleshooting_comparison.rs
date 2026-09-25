//! Bounded next-diagnostic comparison, not a diagnostic agent or a success-rate model.
//!
//! Episodes are controlled fixtures. Both implementations use the same explicit
//! policy: one candidate contribution, plus informative (or resolved) episodes,
//! minus failed episodes. Current inputs restrict questions without adding votes.
//! The current canonical tie break is measured, not endorsed as an action policy.

use pangine::{ConceptId, Pangine};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Context(&'static str, &'static str);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Decision(&'static str, &'static str, &'static str);

impl Decision {
    fn text(self) -> String {
        format!("[{}]->[{}]->[{}]", self.0, self.1, self.2)
    }
}

#[derive(Clone, Copy, Debug)]
struct Record {
    id: &'static str,
    context: Context,
    decision: Decision,
    outcome: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
struct Request {
    context: Context,
    positive: &'static str,
    scope: Option<&'static str>,
}

#[derive(Debug, PartialEq, Eq)]
struct Possibility {
    strength: i64,
    rows: usize,
    sources: BTreeMap<String, i64>,
    top: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Report {
    possibilities: BTreeMap<String, Possibility>,
    selected: Option<String>,
}

struct Run {
    report: Report,
    retained_text_bytes: usize,
}

const LINK: Context = Context("windows", "link-error");
const LOAD: Context = Context("windows", "load-error");
const UNKNOWN: Context = Context("windows", "compile-error");
const SYMBOLS: Decision = Decision("inspect-symbols", "dumpbin", "object");
const MAP: Decision = Decision("inspect-symbols", "link-map", "binary");
const INPUTS: Decision = Decision("trace-inputs", "msbuild-log", "project");
const IMPORTS: Decision = Decision("inspect-imports", "dumpbin", "binary");

const CANDIDATES: [Record; 4] = [
    Record { id: "c-symbols", context: LINK, decision: SYMBOLS, outcome: None },
    Record { id: "c-map", context: LINK, decision: MAP, outcome: None },
    Record { id: "c-inputs", context: LINK, decision: INPUTS, outcome: None },
    Record { id: "c-imports", context: LOAD, decision: IMPORTS, outcome: None },
];

const EPISODES: [Record; 5] = [
    Record { id: "e-symbols", context: LINK, decision: SYMBOLS, outcome: Some("informative") },
    Record { id: "e-map", context: LINK, decision: MAP, outcome: Some("failed") },
    Record { id: "e-inputs", context: LINK, decision: INPUTS, outcome: Some("resolved") },
    Record { id: "e-imports", context: LOAD, decision: IMPORTS, outcome: Some("informative") },
    // Shares action/tool with SYMBOLS and action/scope with MAP, but neither complete decision.
    Record { id: "e-other-target", context: LINK, decision: Decision("inspect-symbols", "dumpbin", "binary"), outcome: Some("informative") },
];

// BEGIN PANGINE CONSUMER

const CANDIDATE_QUERY: &str = "{candidates} @
    ({candidate}->[environment]->${environment-input})
    ({candidate}->[symptom]->${symptom-input})
    ({candidate}->[action]->{action})
    ({candidate}->[tool]->{tool})
    ({candidate}->[scope]->{scope})";

const OUTCOME_QUERY: &str = "{episodes} @
    ({episode}->[environment]->${environment-input})
    ({episode}->[symptom]->${symptom-input})
    ({episode}->[action]->{episode-action})
    ({episode}->[tool]->{episode-tool})
    ({episode}->[scope]->{episode-scope})
    ({episode}->[outcome]->${result-input})";

fn run_pangine(records: &[Record], request: Request) -> Run {
    let mut pangine = Pangine::new();
    let mut labels = BTreeMap::new();
    let mut retained_text_bytes = 0;
    for record in records {
        let mut text = [
            ("environment", record.context.0),
            ("symptom", record.context.1),
            ("action", record.decision.0),
            ("tool", record.decision.1),
            ("scope", record.decision.2),
        ]
        .into_iter()
        .chain(record.outcome.map(|outcome| ("outcome", outcome)))
        .map(|(field, value)| format!("([{}]->[{field}]->[{value}])", record.id))
        .collect::<String>();
        let source = concept(&mut pangine, &text);
        labels.insert(source.clone(), record.id.to_owned());
        retained_text_bytes += pangine.format_concept(&source, false).len();
        let memory = if record.outcome.is_some() { "episodes" } else { "candidates" };
        text = format!("{{{memory}}} ~= {text}");
        pangine.reference_concept(&text).expect("remember complete source");
    }
    for (input, value) in [("environment-input", request.context.0), ("symptom-input", request.context.1)] {
        concept(&mut pangine, &format!("{{{input}}} = [{value}]"));
    }
    let mut query = CANDIDATE_QUERY.to_owned();
    if let Some(scope) = request.scope {
        query.push_str(&format!(" ({{candidate}}->[scope]->[{scope}])"));
    }
    pangine.reference_concept(&query).expect("candidate question");
    let projection = concept(&mut pangine, "{action}->{tool}->{scope}");
    retained_text_bytes += format!("{request:?}").len();
    if pangine.answer_view(&projection).is_none() {
        return Run { report: Report { possibilities: BTreeMap::new(), selected: None }, retained_text_bytes };
    }
    let episode_projection = concept(&mut pangine, "{episode-action}->{episode-tool}->{episode-scope}");
    for (outcome, operator) in [(request.positive, "@+="), ("failed", "@-=")] {
        concept(&mut pangine, &format!("{{result-input}} = [{outcome}]"));
        pangine.reference_concept(OUTCOME_QUERY).expect("outcome question");
        // A zero-row question clears its outputs instead of publishing an empty linked Answer.
        if pangine.answer_view(&episode_projection).is_none() {
            continue;
        }
        let adjustment = format!("{{action}}->{{tool}}->{{scope}} {operator} {{episode-action}}->{{episode-tool}}->{{episode-scope}}");
        pangine.reference_concept(&adjustment).unwrap_or_else(|error| panic!("{adjustment} for {outcome}, request {request:?}: {error}"));
    }
    let answer = pangine.answer_view(&projection).expect("open complete answer");
    let possibilities = answer
        .possibilities(&mut pangine)
        .expect("inspect all alternatives")
        .into_iter()
        .map(|possibility| {
            let sources = possibility.sources().iter().map(|source| (labels[source.concept()].clone(), source.contribution().weight())).collect();
            (
                pangine.format_concept(possibility.value(), false),
                Possibility { strength: possibility.strength().weight(), rows: possibility.complete_rows(), sources, top: possibility.is_top_tie() },
            )
        })
        .collect();
    let selected = answer.choose(&mut pangine).map(|choice| pangine.format_concept(choice.selected(), false));
    let output = pangine.reference_percept("action");
    let encoded = pangine.linked_answer_value(&output).expect("open answer remains after functional choice");
    retained_text_bytes += pangine.format_concept(&encoded, false).len();
    Run { report: Report { possibilities, selected }, retained_text_bytes }
}

fn concept(pangine: &mut Pangine, text: &str) -> ConceptId {
    pangine.reference_concept(text).unwrap_or_else(|error| panic!("{text}: {error}")).expect("nonempty Concept")
}

// END PANGINE CONSUMER
// BEGIN RECORD CONSUMER

fn run_records(records: &[Record], request: Request) -> Run {
    let mut possibilities = BTreeMap::new();
    for candidate in records.iter().filter(|record| record.outcome.is_none() && record.context == request.context) {
        if request.scope.is_some_and(|scope| candidate.decision.2 != scope) {
            continue;
        }
        let possibility = possibilities.entry(candidate.decision.text()).or_insert(Possibility { strength: 0, rows: 0, sources: BTreeMap::new(), top: false });
        possibility.rows += 1;
        possibility.sources.insert(candidate.id.to_owned(), 1);
        for episode in records.iter().filter(|record| record.context == request.context && record.decision == candidate.decision) {
            let contribution = match episode.outcome {
                Some(outcome) if outcome == request.positive => 1,
                Some("failed") => -1,
                _ => continue,
            };
            possibility.sources.insert(episode.id.to_owned(), contribution);
        }
        possibility.strength = possibility.sources.values().sum();
    }
    let maximum = possibilities.values().map(|possibility| possibility.strength).filter(|strength| *strength > 0).max();
    for possibility in possibilities.values_mut() {
        possibility.top = Some(possibility.strength) == maximum;
    }
    // These fixed-label ordered triples have the same order as their Pangine spellings.
    let selected = possibilities.iter().find(|(_, possibility)| possibility.top).map(|(decision, _)| decision.clone());
    let report = Report { possibilities, selected };
    let retained_text_bytes = format!("{records:?}{request:?}{report:?}").len();
    Run { report, retained_text_bytes }
}

// END RECORD CONSUMER

fn compare(records: &[Record], request: Request) -> Report {
    let pangine = run_pangine(records, request).report;
    assert_eq!(pangine, run_records(records, request).report, "{request:?}");
    pangine
}

fn observed_choice(records: &[Record], report: &Report, context: Context, id: &'static str) -> Record {
    let selected = report.selected.as_ref().expect("chosen diagnostic");
    let candidate = records
        .iter()
        .find(|record| record.outcome.is_none() && record.context == context && record.decision.text() == *selected)
        .expect("complete catalog decision in the observed context");
    Record { id, outcome: Some("failed"), ..*candidate }
}

#[test]
#[ignore = "warning: this explicit diagnostic policy is not calibrated confidence or a general decision contract"]
fn repeated_diagnostics_preserve_evidence_through_context_policy_and_history_changes() {
    let mut records = [CANDIDATES.as_slice(), EPISODES.as_slice()].concat();
    let request = Request { context: LINK, positive: "informative", scope: None };
    let first = compare(&records, request);
    assert_eq!(first.selected, Some(SYMBOLS.text()));
    assert_eq!(first.possibilities[&SYMBOLS.text()].sources, BTreeMap::from([("c-symbols".into(), 1), ("e-symbols".into(), 1)]));
    assert_eq!(first.possibilities[&MAP.text()].strength, 0);
    assert!(first.possibilities.values().all(|possibility| possibility.rows == 1));
    records.push(observed_choice(&records, &first, request.context, "live-1"));
    let tied = compare(&records, request);
    assert_eq!(tied.possibilities.values().filter(|possibility| possibility.top).count(), 2);
    assert_eq!(tied.selected, Some(SYMBOLS.text()));
    records.push(observed_choice(&records, &tied, request.context, "live-2"));
    let changed = compare(&records, request);
    assert_eq!(changed.selected, Some(INPUTS.text()));
    assert_eq!(changed.possibilities[&SYMBOLS.text()].strength, 0);
    assert_eq!(changed.possibilities[&SYMBOLS.text()].sources.len(), 4);
    assert_eq!(compare(&records, Request { context: LOAD, ..request }).selected, Some(IMPORTS.text()));
    assert_eq!(compare(&records, Request { positive: "resolved", ..request }).possibilities[&INPUTS.text()].strength, 2);

    // Correct the same observed episode, then retract it. Both sides replay current sources.
    records.iter_mut().find(|record| record.id == "live-2").unwrap().outcome = Some("informative");
    let corrected = compare(&records, request);
    assert_eq!(corrected.selected, Some(SYMBOLS.text()));
    assert_eq!(corrected.possibilities[&SYMBOLS.text()].sources["live-2"], 1);
    records.retain(|record| record.id != "live-2");
    let retracted = compare(&records, request);
    assert_eq!(retracted, tied);
}

#[test]
#[ignore = "warning: candidate availability can cause a choice without any successful outcome evidence"]
fn absence_ties_and_no_positive_support_remain_different_outcomes() {
    let request = Request { context: LINK, positive: "informative", scope: None };
    let untried = compare(&CANDIDATES, request);
    assert_eq!(untried.selected, Some(SYMBOLS.text()));
    assert_eq!(untried.possibilities.values().filter(|possibility| possibility.top).count(), 3);
    assert!(untried.possibilities.values().all(|possibility| possibility.sources.len() == 1));
    let missing = compare(&CANDIDATES, Request { context: UNKNOWN, ..request });
    assert!(missing.possibilities.is_empty());
    assert_eq!(missing.selected, None);
    let mut failed = CANDIDATES.to_vec();
    for (candidate, id) in CANDIDATES[..3].iter().zip(["failed-symbols", "failed-map", "failed-inputs"]) {
        failed.push(Record { id, outcome: Some("failed"), ..*candidate });
    }
    let unsupported = compare(&failed, request);
    assert_eq!(unsupported.selected, None);
    assert_eq!(unsupported.possibilities.len(), 3);
    assert!(unsupported.possibilities.values().all(|possibility| possibility.strength == 0 && possibility.sources.len() == 2));
}

#[test]
#[ignore = "warning: an empty outcome question cannot be composed through language-level answer adjustment"]
fn empty_outcomes_require_a_host_branch_before_answer_adjustment() {
    let mut pangine = Pangine::new();
    concept(&mut pangine, "{candidates} ~= [inspect-symbols]->[dumpbin]");
    concept(&mut pangine, "{candidates} @ {action}->{tool}");
    let action = pangine.reference_percept("action");
    let before = pangine.linked_answer_value(&action).unwrap();
    assert_eq!(pangine.reference_concept("{episodes} @ {past-action}->{past-tool}").expect("valid empty question"), None);
    let past = concept(&mut pangine, "{past-action}->{past-tool}");
    assert!(pangine.answer_view(&past).is_none());
    assert!(matches!(pangine.reference_concept("{action}->{tool} @+= {past-action}->{past-tool}"), Err(pangine::ParseError::InvalidSyntax)));
    assert_eq!(pangine.linked_answer_value(&action), Some(before));
}

#[test]
#[ignore = "warning: controlled diagnostic cases compare implementation cost, not real-world diagnostic accuracy"]
fn diagnostic_matrix_compares_the_same_program_and_record_rule() {
    let original = [CANDIDATES.as_slice(), EPISODES.as_slice()].concat();
    let mut corrected = original.clone();
    corrected.iter_mut().find(|record| record.id == "e-symbols").unwrap().outcome = Some("failed");
    let mut pangine_time = Duration::ZERO;
    let mut records_time = Duration::ZERO;
    let mut cases = 0;
    for records in [CANDIDATES.as_slice(), original.as_slice(), corrected.as_slice()] {
        for context in [LINK, LOAD, UNKNOWN] {
            for positive in ["informative", "resolved"] {
                for scope in [None, Some("object"), Some("binary")] {
                    let request = Request { context, positive, scope };
                    let start = Instant::now();
                    let pangine = run_pangine(records, request);
                    pangine_time += start.elapsed();
                    let start = Instant::now();
                    let baseline = run_records(records, request);
                    records_time += start.elapsed();
                    assert_eq!(pangine.report, baseline.report, "{request:?}");
                    cases += 1;
                }
            }
        }
    }
    // A new tool/context combination is supplied as data; neither implementation changes.
    let novel = Record { id: "c-nm", context: Context("linux", "link-error"), decision: Decision("inspect-symbols", "nm", "object"), outcome: None };
    let mut extended = original.clone();
    extended.push(novel);
    assert_eq!(compare(&extended, Request { context: novel.context, positive: "informative", scope: None }).selected, Some(novel.decision.text()));
    let request = Request { context: LINK, positive: "informative", scope: None };
    println!("diagnostic comparison: {cases} matrix cases plus one new context/tool; end-to-end Pangine={pangine_time:?}, records={records_time:?}");
    println!(
        "initial retained text bytes (inputs, request, open answer/report; not heap): Pangine={}, records={}",
        run_pangine(&original, request).retained_text_bytes,
        run_records(&original, request).retained_text_bytes,
    );
    let source = include_str!("troubleshooting_comparison.rs");
    for label in ["PANGINE", "RECORD"] {
        let begin = format!("// BEGIN {label} CONSUMER");
        let end = format!("// END {label} CONSUMER");
        let body = source.split_once(&begin).unwrap().1.split_once(&end).unwrap().0;
        println!("{label} consumer nonblank lines (including embedded grammar): {}", body.lines().filter(|line| !line.trim().is_empty()).count());
    }
}
