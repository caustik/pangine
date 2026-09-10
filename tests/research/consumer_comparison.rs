//! A bounded consumer comparison over the public Pangine API and ordinary Rust records.
//!
//! This is not a general query adapter. It implements only the two tasks in the
//! research notebook and keeps each side's application conventions visible.

use pangine::{CompletionResult, ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct Report {
    id: &'static str,
    from: &'static str,
    to: &'static str,
}

#[derive(Clone, Copy)]
struct Criterion {
    scope: &'static str,
    report: &'static str,
}

#[derive(Clone, Copy)]
struct Observation {
    id: &'static str,
    outer: &'static str,
    nested: &'static str,
}

const REPORTS: [Report; 3] = [Report { id: "r1", from: "A", to: "B" }, Report { id: "r2", from: "B", to: "C" }, Report { id: "r3", from: "B", to: "D" }];
const CRITERIA: [Criterion; 5] = [
    Criterion { scope: "before", report: "r1" },
    Criterion { scope: "before", report: "r2" },
    Criterion { scope: "before", report: "r3" },
    Criterion { scope: "after", report: "r1" },
    Criterion { scope: "after", report: "r2" },
];
const OBSERVATIONS: [Observation; 2] = [Observation { id: "o-1", outer: "A", nested: "B" }, Observation { id: "o-2", outer: "B", nested: "A" }];

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ReviewPath {
    destination: String,
    first_report: String,
    second_report: String,
    evidence: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ReviewOutcome {
    included_reports: BTreeSet<String>,
    paths: BTreeSet<ReviewPath>,
}

#[derive(Debug, PartialEq, Eq)]
struct ReviewChange {
    excluded_reports: BTreeSet<String>,
    removed_destinations: BTreeSet<String>,
    retained_destinations: BTreeSet<String>,
}

struct ReviewRun {
    before: ReviewOutcome,
    after: ReviewOutcome,
    retained_text_bytes: usize,
    returned_text_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct FailureOutcome {
    recovered_hole: String,
    resumed_pairs: BTreeSet<(String, String)>,
}

struct FailureRun {
    outcome: FailureOutcome,
    attempted_sources: Option<BTreeSet<String>>,
    retained_text_bytes: usize,
    returned_text_bytes: usize,
}

#[test]
#[ignore = "warning: this bounded consumer comparison does not select an inquiry interface or ordinary-record replacement"]
fn changed_questions_compare_consumer_effort_and_retained_context() {
    let pangine_review = run_pangine_review();
    let baseline_review = run_baseline_review();
    assert_eq!(pangine_review.before, baseline_review.before);
    assert_eq!(pangine_review.after, baseline_review.after);

    let pangine_change = explain_review_change(&pangine_review.before, &pangine_review.after);
    let baseline_change = explain_review_change(&baseline_review.before, &baseline_review.after);
    assert_eq!(pangine_change, baseline_change);
    assert_eq!(
        pangine_change,
        ReviewChange { excluded_reports: strings(&["r3"]), removed_destinations: strings(&["D"]), retained_destinations: strings(&["C"]) }
    );

    let pangine_failure = run_pangine_failure();
    let baseline_failure = run_baseline_failure();
    assert_eq!(pangine_failure.outcome, baseline_failure.outcome);
    assert_eq!(pangine_failure.outcome.recovered_hole, "missing");
    assert_eq!(pangine_failure.outcome.resumed_pairs, pairs(&[("o-1", "A"), ("o-2", "B")]));
    assert_eq!(pangine_failure.attempted_sources, None, "an empty public CompletionResult does not retain attempted source records");
    assert_eq!(baseline_failure.attempted_sources, Some(strings(&["o-1", "o-2"])));

    println!(
        "consumer comparison (formatted retained/returned text bytes):\n  Pangine review: {}/{}\n  records review: {}/{}\n  Pangine failure: {}/{}\n  records failure: {}/{}",
        pangine_review.retained_text_bytes,
        pangine_review.returned_text_bytes,
        baseline_review.retained_text_bytes,
        baseline_review.returned_text_bytes,
        pangine_failure.retained_text_bytes,
        pangine_failure.returned_text_bytes,
        baseline_failure.retained_text_bytes,
        baseline_failure.returned_text_bytes,
    );
}

fn explain_review_change(before: &ReviewOutcome, after: &ReviewOutcome) -> ReviewChange {
    let before_destinations = before.paths.iter().map(|path| path.destination.clone()).collect::<BTreeSet<_>>();
    let after_destinations = after.paths.iter().map(|path| path.destination.clone()).collect::<BTreeSet<_>>();
    ReviewChange {
        excluded_reports: &before.included_reports - &after.included_reports,
        removed_destinations: &before_destinations - &after_destinations,
        retained_destinations: &before_destinations & &after_destinations,
    }
}

// BEGIN PANGINE CONSUMER

struct QueryRun {
    result: CompletionResult,
    retained_text_bytes: usize,
    returned_text_bytes: usize,
}

fn run_pangine_review() -> ReviewRun {
    let mut pangine = Pangine::new();
    let reports = pangine.reference_percept("reports");
    let criteria = pangine.reference_percept("criteria");
    let mut source_labels = BTreeMap::new();
    for report in REPORTS {
        let source = format!("[report]->[{}]->(([from]->[{}])([to]->[{}]))", report.id, report.from, report.to);
        add_source(&mut pangine, &reports, &source, format!("report:{}", report.id), &mut source_labels);
    }
    for criterion in CRITERIA {
        let source = format!("[scope]->[{}]->[include]->[{}]", criterion.scope, criterion.report);
        add_source(&mut pangine, &criteria, &source, format!("criterion:{}:{}", criterion.scope, criterion.report), &mut source_labels);
    }

    let input_text_bytes = percept_text_bytes(&pangine, &reports) + percept_text_bytes(&pangine, &criteria);
    let before = run_pangine_review_scope(&mut pangine, "before", &source_labels);
    let after = run_pangine_review_scope(&mut pangine, "after", &source_labels);
    ReviewRun {
        retained_text_bytes: input_text_bytes + before.retained_text_bytes + after.retained_text_bytes,
        returned_text_bytes: before.returned_text_bytes + after.returned_text_bytes,
        before: before.outcome,
        after: after.outcome,
    }
}

struct PangineReviewScope {
    outcome: ReviewOutcome,
    retained_text_bytes: usize,
    returned_text_bytes: usize,
}

fn run_pangine_review_scope(pangine: &mut Pangine, scope: &str, source_labels: &BTreeMap<ConceptId, String>) -> PangineReviewScope {
    let included_output = format!("{scope}-included");
    let included_question = format!("[scope]->[{scope}]->[include]->{{{included_output}}}");
    let included = run_linked_question(pangine, "{criteria}", &included_question, &included_output);

    let first_output = format!("{scope}-first-report");
    let second_output = format!("{scope}-second-report");
    let middle_output = format!("{scope}-middle");
    let destination_output = format!("{scope}-destination");
    let path_question = format!(
        "([scope]->[{scope}]->[include]->{{{first_output}}})
         ([scope]->[{scope}]->[include]->{{{second_output}}})
         ([report]->{{{first_output}}}->(([from]->[A])([to]->{{{middle_output}}})))
         ([report]->{{{second_output}}}->(([from]->{{{middle_output}}})([to]->{{{destination_output}}})))"
    );
    let path_run = run_linked_question(pangine, "{reports}{criteria}", &path_question, &destination_output);

    let included_percept = pangine.reference_percept(&included_output);
    let included_reports =
        included.result.completions().iter().map(|row| concept_name(pangine, row.binding(&included_percept).expect("included report"))).collect();
    let first_percept = pangine.reference_percept(&first_output);
    let second_percept = pangine.reference_percept(&second_output);
    let destination_percept = pangine.reference_percept(&destination_output);
    let paths = path_run
        .result
        .completions()
        .iter()
        .map(|row| ReviewPath {
            destination: concept_name(pangine, row.binding(&destination_percept).expect("path destination")),
            first_report: concept_name(pangine, row.binding(&first_percept).expect("first path report")),
            second_report: concept_name(pangine, row.binding(&second_percept).expect("second path report")),
            evidence: row.evidence().iter().map(|evidence| source_labels.get(evidence.source_concept()).expect("known review source").clone()).collect(),
        })
        .collect();
    PangineReviewScope {
        outcome: ReviewOutcome { included_reports, paths },
        retained_text_bytes: included.retained_text_bytes + path_run.retained_text_bytes,
        returned_text_bytes: path_run.returned_text_bytes,
    }
}

fn run_pangine_failure() -> FailureRun {
    let mut pangine = Pangine::new();
    let observations = pangine.reference_percept("observations");
    for observation in OBSERVATIONS {
        let source = format!("[observation]->[{}]->(([field]->[{}])([nested]->([field]->[{}])))", observation.id, observation.outer, observation.nested);
        let source = must_ref(&mut pangine, &source);
        pangine.perform_experience(&observations, Some(&source));
    }
    let input_text_bytes = percept_text_bytes(&pangine, &observations);

    let failed_question = must_ref(&mut pangine, "[absent]->{missing}");
    let failed = pangine.complete_question(std::slice::from_ref(&observations), &failed_question).expect("valid failed question");
    assert!(failed.completions().is_empty());
    assert_eq!(failed.question(), &failed_question);

    let recovered = pangine.reference_percept("recovered-hole");
    let inspection_question = must_ref(&mut pangine, "[absent]->{recovered-hole}");
    let inspection = pangine.complete_subject(failed.question(), &inspection_question).expect("question inspection");
    let [inspection_row] = inspection.completions() else {
        panic!("one literal question hole should be inspectable");
    };
    let hole = inspection_row.binding(&recovered).expect("literal question hole").clone();
    assert!(pangine.get_percept(&hole).is_some());

    let resumed_id = pangine.reference_percept("resumed-id");
    let resumed_nested = pangine.reference_percept("resumed-nested");
    let field_name = pangine.reference_name("field");
    let nested_name = pangine.reference_name("nested");
    let observation_name = pangine.reference_name("observation");
    let field = ordered(&mut pangine, vec![field_name, hole.clone()]);
    let nested = ordered(&mut pangine, vec![nested_name, resumed_nested]);
    let body = union(&mut pangine, vec![field, nested]);
    let resumed_question = ordered(&mut pangine, vec![observation_name, resumed_id.clone(), body]);
    let resumed = pangine.complete_question(std::slice::from_ref(&observations), &resumed_question).expect("resumed field question");
    let resumed_pairs = resumed
        .completions()
        .iter()
        .map(|row| {
            (
                concept_name(&pangine, row.binding(&resumed_id).expect("resumed observation id")),
                concept_name(&pangine, row.binding(&hole).expect("resumed field value")),
            )
        })
        .collect::<BTreeSet<_>>();
    let returned_text_bytes = render_pairs(&resumed_pairs).len();
    let retained_text_bytes = input_text_bytes + pangine.format_concept(failed.question(), false).len() + "rows=0".len();
    FailureRun {
        outcome: FailureOutcome { recovered_hole: percept_name(&pangine, &hole), resumed_pairs },
        attempted_sources: None,
        retained_text_bytes,
        returned_text_bytes,
    }
}

fn run_linked_question(pangine: &mut Pangine, selector: &str, question: &str, output: &str) -> QueryRun {
    must_ref(pangine, &format!("{selector} @ {question}"));
    let output = pangine.reference_percept(output);
    let answer_value = pangine.linked_answer_value(&output).expect("linked consumer answer");
    let retained_text_bytes = pangine.format_concept(&answer_value, false).len();
    let output_value = pangine.get_value(&output).expect("linked consumer output");
    let returned_text_bytes = pangine.format_concept(&output_value, false).len();
    let result = pangine.answer_snapshot(&output).expect("consumer answer snapshot").result().clone();
    QueryRun { result, retained_text_bytes, returned_text_bytes }
}

fn add_source(pangine: &mut Pangine, percept: &ConceptId, source: &str, label: String, labels: &mut BTreeMap<ConceptId, String>) {
    let source = must_ref(pangine, source);
    pangine.perform_experience(percept, Some(&source));
    labels.insert(source, label);
}

fn ordered(pangine: &mut Pangine, components: Vec<ConceptId>) -> ConceptId {
    pangine.compose_ordered(&components).expect("owned ordered components").expect("nonempty ordered value")
}

fn union(pangine: &mut Pangine, members: Vec<ConceptId>) -> ConceptId {
    let members = members.into_iter().map(|member| (Relevance::DEFAULT, member)).collect::<Vec<_>>();
    pangine.compose_union(&members).expect("owned union members").expect("nonempty union")
}

fn percept_text_bytes(pangine: &Pangine, percept: &ConceptId) -> usize {
    pangine.get_value(percept).map_or(0, |value| pangine.format_concept(&value, false).len())
}

fn concept_name(pangine: &Pangine, concept: &ConceptId) -> String {
    pangine.get_name(concept).expect("consumer value should be an opaque name").to_owned()
}

fn percept_name(pangine: &Pangine, concept: &ConceptId) -> String {
    assert!(pangine.get_percept(concept).is_some(), "consumer value should be a Percept");
    let formatted = pangine.format_concept(concept, false);
    formatted.strip_prefix('{').and_then(|name| name.strip_suffix('}')).expect("compact fixture Percept").to_owned()
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected nonempty Concept for {input:?}"))
}

// END PANGINE CONSUMER

// BEGIN RECORD CONSUMER

fn run_baseline_review() -> ReviewRun {
    let before = baseline_review_scope("before");
    let after = baseline_review_scope("after");
    ReviewRun {
        retained_text_bytes: render_review_inputs().len() + render_review(&before).len() + render_review(&after).len(),
        returned_text_bytes: render_destinations(&before).len() + render_destinations(&after).len(),
        before,
        after,
    }
}

fn baseline_review_scope(scope: &str) -> ReviewOutcome {
    let included_reports = CRITERIA.iter().filter(|criterion| criterion.scope == scope).map(|criterion| criterion.report.to_owned()).collect::<BTreeSet<_>>();
    let mut paths = BTreeSet::new();
    for first in REPORTS.iter().filter(|report| included_reports.contains(report.id) && report.from == "A") {
        for second in REPORTS.iter().filter(|report| included_reports.contains(report.id) && report.from == first.to) {
            paths.insert(ReviewPath {
                destination: second.to.to_owned(),
                first_report: first.id.to_owned(),
                second_report: second.id.to_owned(),
                evidence: strings(&[
                    &format!("report:{}", first.id),
                    &format!("report:{}", second.id),
                    &format!("criterion:{scope}:{}", first.id),
                    &format!("criterion:{scope}:{}", second.id),
                ]),
            });
        }
    }
    ReviewOutcome { included_reports, paths }
}

fn run_baseline_failure() -> FailureRun {
    let failed = run_record_question(RecordQuestion { field: "absent", output: "missing" });
    assert!(failed.rows.is_empty());

    let recovered_hole = failed.question.output.to_owned();
    let resumed = run_record_question(RecordQuestion { field: "outer", output: failed.question.output });
    let retained = format!(
        "{};question={}:{};attempts={};rows={}",
        render_observations(),
        failed.question.field,
        failed.question.output,
        failed.attempted_sources.iter().cloned().collect::<Vec<_>>().join(","),
        failed.rows.len()
    );
    let outcome = FailureOutcome { recovered_hole, resumed_pairs: resumed.rows };
    FailureRun {
        returned_text_bytes: render_pairs(&outcome.resumed_pairs).len(),
        outcome,
        attempted_sources: Some(failed.attempted_sources),
        retained_text_bytes: retained.len(),
    }
}

#[derive(Clone, Copy)]
struct RecordQuestion {
    field: &'static str,
    output: &'static str,
}

struct RecordAttempt {
    question: RecordQuestion,
    attempted_sources: BTreeSet<String>,
    rows: BTreeSet<(String, String)>,
}

fn run_record_question(question: RecordQuestion) -> RecordAttempt {
    let attempted_sources = OBSERVATIONS.iter().map(|observation| observation.id.to_owned()).collect();
    let rows = OBSERVATIONS
        .iter()
        .filter_map(|observation| {
            let value = match question.field {
                "outer" => observation.outer,
                "nested" => observation.nested,
                _ => return None,
            };
            Some((observation.id.to_owned(), value.to_owned()))
        })
        .collect();
    RecordAttempt { question, attempted_sources, rows }
}

fn render_review_inputs() -> String {
    let reports = REPORTS.iter().map(|report| format!("{}:{}>{}", report.id, report.from, report.to));
    let criteria = CRITERIA.iter().map(|criterion| format!("{}:{}", criterion.scope, criterion.report));
    reports.chain(criteria).collect::<Vec<_>>().join("|")
}

fn render_observations() -> String {
    OBSERVATIONS.iter().map(|observation| format!("{}:{}:{}", observation.id, observation.outer, observation.nested)).collect::<Vec<_>>().join("|")
}

fn render_review(outcome: &ReviewOutcome) -> String {
    let included = outcome.included_reports.iter().cloned().collect::<Vec<_>>().join(",");
    let paths = outcome
        .paths
        .iter()
        .map(|path| {
            format!("{}>{}:{}:{}", path.first_report, path.second_report, path.destination, path.evidence.iter().cloned().collect::<Vec<_>>().join(","))
        })
        .collect::<Vec<_>>()
        .join("|");
    format!("included={included};paths={paths}")
}

fn render_destinations(outcome: &ReviewOutcome) -> String {
    outcome.paths.iter().map(|path| path.destination.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>().join(",")
}

// END RECORD CONSUMER

fn render_pairs(values: &BTreeSet<(String, String)>) -> String {
    values.iter().map(|(left, right)| format!("{left}:{right}")).collect::<Vec<_>>().join("|")
}

fn strings(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn pairs(values: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    values.iter().map(|(left, right)| ((*left).to_owned(), (*right).to_owned())).collect()
}
