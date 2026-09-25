//! Long-lived decision-ledger comparison over current Pangine and typed Rust records.
//!
//! The fixture treats learned judgments as fallible, append-only evidence. It does
//! not run Laya or TypeSafe, interpret probabilities as Relevance, or put action
//! policy inside Pangine. Questions are applied after independently identified
//! request, judgment, outcome, model, schema, and revision sources are recorded.

use pangine::{CompletionResult, ConceptId, Pangine};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Request { source: &'static str, request: &'static str, language: &'static str, workflow: &'static str, schema: &'static str },
    Judgment { source: &'static str, judgment: &'static str, request: &'static str, model: &'static str, choice: &'static str, confidence: &'static str },
    Outcome { source: &'static str, outcome: &'static str, judgment: &'static str, result: &'static str, expected: &'static str },
    Model { source: &'static str, model: &'static str, family: &'static str, calibration: &'static str },
    Schema { source: &'static str, schema: &'static str, option_band: &'static str },
    Revision { source: &'static str, revision: &'static str, before: &'static str, after: &'static str },
}

impl Event {
    fn source(self) -> &'static str {
        match self {
            Self::Request { source, .. }
            | Self::Judgment { source, .. }
            | Self::Outcome { source, .. }
            | Self::Model { source, .. }
            | Self::Schema { source, .. }
            | Self::Revision { source, .. } => source,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Finding {
    values: Vec<String>,
    sources: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueryRun {
    findings: BTreeSet<Finding>,
    retained_text_bytes: usize,
}

const CORE_EVENTS: [Event; 16] = [
    Event::Request { source: "source-request-en", request: "request-en", language: "en", workflow: "support-triage", schema: "support-4" },
    Event::Judgment {
        source: "source-judgment-en-v1",
        judgment: "judgment-en-v1",
        request: "request-en",
        model: "laya-english-v1",
        choice: "billing",
        confidence: "high",
    },
    Event::Outcome { source: "source-outcome-en-v1", outcome: "outcome-en-v1", judgment: "judgment-en-v1", result: "correct", expected: "billing" },
    Event::Request { source: "source-request-ro", request: "request-ro", language: "ro", workflow: "skill-routing", schema: "skills-13" },
    Event::Judgment {
        source: "source-judgment-ro-v1",
        judgment: "judgment-ro-v1",
        request: "request-ro",
        model: "laya-router-v1",
        choice: "godot",
        confidence: "high",
    },
    Event::Outcome { source: "source-outcome-ro-v1", outcome: "outcome-ro-v1", judgment: "judgment-ro-v1", result: "wrong", expected: "video" },
    Event::Judgment {
        source: "source-judgment-ro-v2",
        judgment: "judgment-ro-v2",
        request: "request-ro",
        model: "laya-router-v2",
        choice: "video",
        confidence: "medium",
    },
    Event::Outcome { source: "source-outcome-ro-v2", outcome: "outcome-ro-v2", judgment: "judgment-ro-v2", result: "correct", expected: "video" },
    Event::Request { source: "source-request-km", request: "request-km", language: "km", workflow: "support-triage", schema: "support-4" },
    Event::Judgment {
        source: "source-judgment-km-v1",
        judgment: "judgment-km-v1",
        request: "request-km",
        model: "laya-router-v1",
        choice: "sales",
        confidence: "high",
    },
    Event::Outcome { source: "source-outcome-km-v1", outcome: "outcome-km-v1", judgment: "judgment-km-v1", result: "wrong", expected: "billing" },
    Event::Judgment {
        source: "source-judgment-km-v2",
        judgment: "judgment-km-v2",
        request: "request-km",
        model: "laya-router-v2",
        choice: "billing",
        confidence: "medium",
    },
    Event::Outcome { source: "source-outcome-km-v2", outcome: "outcome-km-v2", judgment: "judgment-km-v2", result: "correct", expected: "billing" },
    Event::Request { source: "source-request-catalog", request: "request-catalog", language: "en", workflow: "catalog-routing", schema: "catalog-77" },
    Event::Judgment {
        source: "source-judgment-catalog-v1",
        judgment: "judgment-catalog-v1",
        request: "request-catalog",
        model: "laya-router-v1",
        choice: "product-a",
        confidence: "high",
    },
    Event::Outcome {
        source: "source-outcome-catalog-v1",
        outcome: "outcome-catalog-v1",
        judgment: "judgment-catalog-v1",
        result: "wrong",
        expected: "product-z",
    },
];

const LATER_METADATA: [Event; 6] = [
    Event::Model { source: "source-model-english-v1", model: "laya-english-v1", family: "laya", calibration: "fitted" },
    Event::Model { source: "source-model-router-v1", model: "laya-router-v1", family: "laya", calibration: "unfitted" },
    Event::Model { source: "source-model-router-v2", model: "laya-router-v2", family: "laya", calibration: "task-fitted" },
    Event::Schema { source: "source-schema-support", schema: "support-4", option_band: "few" },
    Event::Schema { source: "source-schema-skills", schema: "skills-13", option_band: "many" },
    Event::Schema { source: "source-schema-catalog", schema: "catalog-77", option_band: "many" },
];

const LATER_REVISION: Event =
    Event::Revision { source: "source-revision-router", revision: "router-upgrade", before: "laya-router-v1", after: "laya-router-v2" };

fn finding(values: &[&str], sources: &[&str]) -> Finding {
    Finding { values: values.iter().map(|value| (*value).to_owned()).collect(), sources: sources.iter().map(|source| (*source).to_owned()).collect() }
}

// BEGIN PANGINE CONSUMER

const HIGH_CONFIDENCE_ERRORS: &str = "
    ({request}->[kind]->[request])
    ({request}->[language]->{language})
    ({request}->[schema]->{schema})
    ({judgment}->[kind]->[judgment])
    ({judgment}->[request]->{request})
    ({judgment}->[model]->{model})
    ({judgment}->[choice]->{choice})
    ({judgment}->[confidence]->[high])
    ({outcome}->[kind]->[outcome])
    ({outcome}->[judgment]->{judgment})
    ({outcome}->[result]->[wrong])
    ({outcome}->[expected]->{expected})";

const UNFITTED_MANY_OPTION_ERRORS: &str = "
    ({request}->[kind]->[request])
    ({request}->[language]->{language})
    ({request}->[schema]->{schema})
    ({judgment}->[kind]->[judgment])
    ({judgment}->[request]->{request})
    ({judgment}->[model]->{model})
    ({judgment}->[choice]->{choice})
    ({judgment}->[confidence]->[high])
    ({outcome}->[kind]->[outcome])
    ({outcome}->[judgment]->{judgment})
    ({outcome}->[result]->[wrong])
    ({outcome}->[expected]->{expected})
    ({model}->[kind]->[model])
    ({model}->[calibration]->[unfitted])
    ({schema}->[kind]->[schema])
    ({schema}->[option-band]->[many])";

const MODEL_METADATA_EXTENSION: &str = "
    ({model}->[kind]->[model])
    ({model}->[calibration]->{calibration})";

const SCHEMA_METADATA_EXTENSION: &str = "
    ({schema}->[kind]->[schema])
    ({schema}->[option-band]->{option-band})";

const IMPROVED_ACROSS_REVISION: &str = "
    ({request}->[kind]->[request])
    ({request}->[language]->{language})
    ({revision}->[kind]->[revision])
    ({revision}->[before]->{before-model})
    ({revision}->[after]->{after-model})
    ({before-judgment}->[kind]->[judgment])
    ({before-judgment}->[request]->{request})
    ({before-judgment}->[model]->{before-model})
    ({before-judgment}->[choice]->{before-choice})
    ({before-outcome}->[kind]->[outcome])
    ({before-outcome}->[judgment]->{before-judgment})
    ({before-outcome}->[result]->[wrong])
    ({after-judgment}->[kind]->[judgment])
    ({after-judgment}->[request]->{request})
    ({after-judgment}->[model]->{after-model})
    ({after-judgment}->[choice]->{after-choice})
    ({after-outcome}->[kind]->[outcome])
    ({after-outcome}->[judgment]->{after-judgment})
    ({after-outcome}->[result]->[correct])";

impl Event {
    fn concept(self) -> String {
        let (subject, fields) = match self {
            Self::Request { request, language, workflow, schema, .. } => {
                (request, vec![("kind", "request"), ("language", language), ("workflow", workflow), ("schema", schema)])
            }
            Self::Judgment { judgment, request, model, choice, confidence, .. } => {
                (judgment, vec![("kind", "judgment"), ("request", request), ("model", model), ("choice", choice), ("confidence", confidence)])
            }
            Self::Outcome { outcome, judgment, result, expected, .. } => {
                (outcome, vec![("kind", "outcome"), ("judgment", judgment), ("result", result), ("expected", expected)])
            }
            Self::Model { model, family, calibration, .. } => (model, vec![("kind", "model"), ("family", family), ("calibration", calibration)]),
            Self::Schema { schema, option_band, .. } => (schema, vec![("kind", "schema"), ("option-band", option_band)]),
            Self::Revision { revision, before, after, .. } => (revision, vec![("kind", "revision"), ("before", before), ("after", after)]),
        };
        fields.into_iter().map(|(field, value)| format!("([{subject}]->[{field}]->[{value}])")).collect()
    }
}

struct PangineLedger {
    pangine: Pangine,
    sources: Vec<ConceptId>,
    retained_source_bytes: usize,
}

impl PangineLedger {
    fn new(events: &[Event]) -> Self {
        let mut ledger = Self { pangine: Pangine::new(), sources: Vec::new(), retained_source_bytes: 0 };
        ledger.append(events);
        ledger
    }

    fn append(&mut self, events: &[Event]) {
        for event in events {
            let source = self.pangine.reference_percept(event.source());
            let value = concept(&mut self.pangine, &event.concept());
            self.pangine.perform_experience(&source, Some(&value)).expect("remember event source");
            self.retained_source_bytes += self.pangine.format_concept(&source, false).len() + self.pangine.format_concept(&value, false).len();
            self.sources.push(source);
        }
    }

    fn query(&mut self, text: &str, outputs: &[&str]) -> QueryRun {
        let question = concept(&mut self.pangine, text);
        let result = self.pangine.complete_question(&self.sources, &question).expect("valid ledger question");
        let findings = findings(&mut self.pangine, &result, outputs);
        let retained_text_bytes = self.retained_source_bytes + self.pangine.format_concept(&question, false).len() + format!("{findings:?}").len();
        QueryRun { findings, retained_text_bytes }
    }

    fn open(&mut self, text: &str, anchor: &str, outputs: &[&str]) -> QueryRun {
        let selector = self.sources.iter().map(|source| self.pangine.format_concept(source, false)).collect::<String>();
        concept(&mut self.pangine, &format!("{selector} @ {text}"));
        let anchor = self.pangine.reference_percept(anchor);
        let answer = self.pangine.answer_snapshot(&anchor).expect("open ledger Answer");
        let findings = findings(&mut self.pangine, answer.result(), outputs);
        let retained_text_bytes = self.retained_source_bytes + text.len() + format!("{findings:?}").len();
        QueryRun { findings, retained_text_bytes }
    }
}

fn findings(pangine: &mut Pangine, result: &CompletionResult, outputs: &[&str]) -> BTreeSet<Finding> {
    let output_percepts = outputs.iter().map(|output| pangine.reference_percept(output)).collect::<Vec<_>>();
    result
        .completions()
        .iter()
        .map(|row| Finding {
            values: output_percepts
                .iter()
                .map(|output| pangine.get_name(row.binding(output).expect("bound ledger output")).expect("named ledger value").to_owned())
                .collect(),
            sources: row.evidence().iter().map(|evidence| percept_name(pangine, evidence.source_percept().expect("retained ledger source"))).collect(),
        })
        .collect()
}

fn percept_name(pangine: &Pangine, concept: &ConceptId) -> String {
    let formatted = pangine.format_concept(concept, false);
    formatted.strip_prefix('{').and_then(|name| name.strip_suffix('}')).expect("compact fixture Percept").to_owned()
}

fn concept(pangine: &mut Pangine, text: &str) -> ConceptId {
    pangine.reference_concept(text).unwrap_or_else(|error| panic!("{text}: {error}")).expect("nonempty Concept")
}

// END PANGINE CONSUMER
// BEGIN RECORD CONSUMER

fn record_high_confidence_errors(events: &[Event]) -> QueryRun {
    let mut findings = BTreeSet::new();
    for event in events {
        let Event::Judgment { source: judgment_source, judgment, request, model, choice, confidence: "high" } = *event else {
            continue;
        };
        let Some((request_source, language, _schema)) = request_record(events, request) else {
            continue;
        };
        let Some((outcome_source, "wrong", expected_choice)) = outcome_record(events, judgment) else {
            continue;
        };
        findings.insert(finding(&[request, language, judgment, model, choice, expected_choice], &[request_source, judgment_source, outcome_source]));
    }
    record_run(events, findings, HIGH_CONFIDENCE_ERRORS)
}

fn record_enriched_high_confidence_errors(events: &[Event]) -> QueryRun {
    let mut findings = BTreeSet::new();
    for event in events {
        let Event::Judgment { source: judgment_source, judgment, request, model, choice, confidence: "high" } = *event else {
            continue;
        };
        let Some((request_source, language, schema)) = request_record(events, request) else {
            continue;
        };
        let Some((outcome_source, "wrong", expected_choice)) = outcome_record(events, judgment) else {
            continue;
        };
        let Some((model_source, calibration)) = model_record(events, model) else {
            continue;
        };
        let Some((schema_source, option_band)) = schema_record(events, schema) else {
            continue;
        };
        findings.insert(finding(
            &[request, language, judgment, model, schema, choice, expected_choice, calibration, option_band],
            &[request_source, judgment_source, outcome_source, model_source, schema_source],
        ));
    }
    record_run(events, findings, MODEL_METADATA_EXTENSION)
}

fn record_unfitted_many_option_errors(events: &[Event]) -> QueryRun {
    let mut findings = BTreeSet::new();
    for event in events {
        let Event::Judgment { source: judgment_source, judgment, request, model, choice, confidence: "high" } = *event else {
            continue;
        };
        let Some((request_source, language, schema)) = request_record(events, request) else {
            continue;
        };
        let Some((outcome_source, "wrong", expected_choice)) = outcome_record(events, judgment) else {
            continue;
        };
        let Some((model_source, "unfitted")) = model_record(events, model) else {
            continue;
        };
        let Some((schema_source, "many")) = schema_record(events, schema) else {
            continue;
        };
        findings.insert(finding(
            &[request, language, judgment, model, schema, choice, expected_choice],
            &[request_source, judgment_source, outcome_source, model_source, schema_source],
        ));
    }
    record_run(events, findings, UNFITTED_MANY_OPTION_ERRORS)
}

fn record_improved_across_revision(events: &[Event]) -> QueryRun {
    let mut findings = BTreeSet::new();
    for revision in events {
        let Event::Revision { source: revision_source, before, after, .. } = *revision else {
            continue;
        };
        for request_event in events {
            let Event::Request { source: request_source, request, language, .. } = *request_event else {
                continue;
            };
            for before_event in events {
                let Event::Judgment {
                    source: before_judgment_source,
                    judgment: before_judgment,
                    request: before_request,
                    model: before_model,
                    choice: before_choice,
                    ..
                } = *before_event
                else {
                    continue;
                };
                if before_request != request || before_model != before {
                    continue;
                }
                let Some((before_outcome_source, "wrong", _)) = outcome_record(events, before_judgment) else {
                    continue;
                };
                for after_event in events {
                    let Event::Judgment {
                        source: after_judgment_source,
                        judgment: after_judgment,
                        request: after_request,
                        model: after_model,
                        choice: after_choice,
                        ..
                    } = *after_event
                    else {
                        continue;
                    };
                    if after_request != request || after_model != after {
                        continue;
                    }
                    let Some((after_outcome_source, "correct", _)) = outcome_record(events, after_judgment) else {
                        continue;
                    };
                    findings.insert(finding(
                        &[request, language, before_judgment, after_judgment, before_choice, after_choice],
                        &[request_source, revision_source, before_judgment_source, before_outcome_source, after_judgment_source, after_outcome_source],
                    ));
                }
            }
        }
    }
    record_run(events, findings, IMPROVED_ACROSS_REVISION)
}

fn request_record(events: &[Event], selected: &str) -> Option<(&'static str, &'static str, &'static str)> {
    events.iter().find_map(|event| match *event {
        Event::Request { source, request, language, schema, .. } if request == selected => Some((source, language, schema)),
        _ => None,
    })
}

fn outcome_record(events: &[Event], selected: &str) -> Option<(&'static str, &'static str, &'static str)> {
    events.iter().find_map(|event| match *event {
        Event::Outcome { source, judgment, result, expected, .. } if judgment == selected => Some((source, result, expected)),
        _ => None,
    })
}

fn model_record(events: &[Event], selected: &str) -> Option<(&'static str, &'static str)> {
    events.iter().find_map(|event| match *event {
        Event::Model { source, model, calibration, .. } if model == selected => Some((source, calibration)),
        _ => None,
    })
}

fn schema_record(events: &[Event], selected: &str) -> Option<(&'static str, &'static str)> {
    events.iter().find_map(|event| match *event {
        Event::Schema { source, schema, option_band } if schema == selected => Some((source, option_band)),
        _ => None,
    })
}

fn record_run(events: &[Event], findings: BTreeSet<Finding>, question: &str) -> QueryRun {
    let retained_text_bytes = format!("{events:?}{question}{findings:?}").len();
    QueryRun { findings, retained_text_bytes }
}

// END RECORD CONSUMER

#[test]
#[ignore = "warning: synthetic typed judgments test durable relational evidence, not model quality"]
fn later_questions_join_independent_decision_sources_and_keep_their_lineage() {
    let mut ledger = PangineLedger::new(&CORE_EVENTS);
    let pangine = ledger.query(HIGH_CONFIDENCE_ERRORS, &["request", "language", "judgment", "model", "choice", "expected"]);
    let records = record_high_confidence_errors(&CORE_EVENTS);

    assert_eq!(pangine.findings, records.findings);
    assert_eq!(
        pangine.findings,
        BTreeSet::from([
            finding(
                &["request-catalog", "en", "judgment-catalog-v1", "laya-router-v1", "product-a", "product-z"],
                &["source-request-catalog", "source-judgment-catalog-v1", "source-outcome-catalog-v1"],
            ),
            finding(
                &["request-km", "km", "judgment-km-v1", "laya-router-v1", "sales", "billing"],
                &["source-request-km", "source-judgment-km-v1", "source-outcome-km-v1"],
            ),
            finding(
                &["request-ro", "ro", "judgment-ro-v1", "laya-router-v1", "godot", "video"],
                &["source-request-ro", "source-judgment-ro-v1", "source-outcome-ro-v1"],
            ),
        ])
    );
}

#[test]
#[ignore = "warning: late metadata enrichment is append-only and does not establish mutable-source or persistence semantics"]
fn metadata_added_after_ingestion_supports_a_new_question_without_rewriting_earlier_sources() {
    let mut ledger = PangineLedger::new(&CORE_EVENTS);
    let before = ledger.open(HIGH_CONFIDENCE_ERRORS, "request", &["request", "language", "judgment", "model", "choice", "expected"]);
    ledger.append(&LATER_METADATA);
    assert_eq!(before.findings, record_high_confidence_errors(&CORE_EVENTS).findings);

    let mut events = CORE_EVENTS.to_vec();
    events.extend(LATER_METADATA);
    ledger.open(MODEL_METADATA_EXTENSION, "calibration", &["request", "language", "judgment", "model", "schema", "choice", "expected", "calibration"]);
    let extended = ledger.open(
        SCHEMA_METADATA_EXTENSION,
        "option-band",
        &["request", "language", "judgment", "model", "schema", "choice", "expected", "calibration", "option-band"],
    );
    let enriched_records = record_enriched_high_confidence_errors(&events);
    assert_eq!(extended.findings, enriched_records.findings);

    let mut direct_ledger = PangineLedger::new(&events);
    let direct = direct_ledger.query(UNFITTED_MANY_OPTION_ERRORS, &["request", "language", "judgment", "model", "schema", "choice", "expected"]);
    let records = record_unfitted_many_option_errors(&events);
    assert_eq!(direct.findings, records.findings);
    assert_eq!(
        direct.findings,
        BTreeSet::from([
            finding(
                &["request-catalog", "en", "judgment-catalog-v1", "laya-router-v1", "catalog-77", "product-a", "product-z",],
                &["source-request-catalog", "source-judgment-catalog-v1", "source-outcome-catalog-v1", "source-model-router-v1", "source-schema-catalog",],
            ),
            finding(
                &["request-ro", "ro", "judgment-ro-v1", "laya-router-v1", "skills-13", "godot", "video"],
                &["source-request-ro", "source-judgment-ro-v1", "source-outcome-ro-v1", "source-model-router-v1", "source-schema-skills",],
            ),
        ])
    );
}

#[test]
#[ignore = "warning: explicit revision relationships compare immutable evaluations but do not define model rollout or replacement policy"]
fn a_later_revision_question_relates_old_and_new_outcomes_without_erasing_either() {
    let mut events = CORE_EVENTS.to_vec();
    events.extend(LATER_METADATA);
    events.push(LATER_REVISION);

    let mut ledger = PangineLedger::new(&events);
    let pangine = ledger.query(IMPROVED_ACROSS_REVISION, &["request", "language", "before-judgment", "after-judgment", "before-choice", "after-choice"]);
    let records = record_improved_across_revision(&events);
    assert_eq!(pangine.findings, records.findings);
    assert_eq!(
        pangine.findings,
        BTreeSet::from([
            finding(
                &["request-km", "km", "judgment-km-v1", "judgment-km-v2", "sales", "billing"],
                &[
                    "source-request-km",
                    "source-revision-router",
                    "source-judgment-km-v1",
                    "source-outcome-km-v1",
                    "source-judgment-km-v2",
                    "source-outcome-km-v2",
                ],
            ),
            finding(
                &["request-ro", "ro", "judgment-ro-v1", "judgment-ro-v2", "godot", "video"],
                &[
                    "source-request-ro",
                    "source-revision-router",
                    "source-judgment-ro-v1",
                    "source-outcome-ro-v1",
                    "source-judgment-ro-v2",
                    "source-outcome-ro-v2",
                ],
            ),
        ])
    );
}

#[test]
#[ignore = "warning: fixed synthetic cases compare query mechanics and representation cost, not production audit performance"]
fn decision_ledger_reports_implementation_and_retained_text_cost() {
    let mut events = CORE_EVENTS.to_vec();
    events.extend(LATER_METADATA);
    events.push(LATER_REVISION);

    let mut pangine_time = Duration::ZERO;
    let mut records_time = Duration::ZERO;
    let mut ledger = PangineLedger::new(&events);
    let cases = [
        (HIGH_CONFIDENCE_ERRORS, &["request", "language", "judgment", "model", "choice", "expected"][..]),
        (UNFITTED_MANY_OPTION_ERRORS, &["request", "language", "judgment", "model", "schema", "choice", "expected"][..]),
        (IMPROVED_ACROSS_REVISION, &["request", "language", "before-judgment", "after-judgment", "before-choice", "after-choice"][..]),
    ];
    let mut pangine_runs = Vec::new();
    for (question, outputs) in cases {
        let start = Instant::now();
        pangine_runs.push(ledger.query(question, outputs));
        pangine_time += start.elapsed();
    }
    let start = Instant::now();
    let record_runs = [record_high_confidence_errors(&events), record_unfitted_many_option_errors(&events), record_improved_across_revision(&events)];
    records_time += start.elapsed();
    for (pangine, records) in pangine_runs.iter().zip(record_runs.iter()) {
        assert_eq!(pangine.findings, records.findings);
    }

    println!(
        "decision-ledger comparison: three later questions over {} independently identified sources; Pangine={pangine_time:?}, records={records_time:?}",
        events.len()
    );
    println!(
        "retained text proxy for the five-source enriched query (sources, question, report; not heap): Pangine={}, records={}",
        pangine_runs[1].retained_text_bytes, record_runs[1].retained_text_bytes
    );
    let source = include_str!("decision_ledger_comparison.rs");
    for label in ["PANGINE CONSUMER", "RECORD CONSUMER"] {
        let begin = format!("// BEGIN {label}");
        let end = format!("// END {label}");
        let body = source.split_once(&begin).unwrap().1.split_once(&end).unwrap().0;
        println!("{label} nonblank lines: {}", body.lines().filter(|line| !line.trim().is_empty()).count());
    }
}
