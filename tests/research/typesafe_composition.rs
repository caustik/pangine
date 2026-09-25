//! TypeSafe-inspired composition comparison over captured-response-shaped fixtures.
//!
//! The fixtures reproduce the fields and strict thresholds in TypeSafe's
//! speculative fan-out support example, plus the documented low-confidence
//! route-to-human pattern. They are not Jev outputs and say nothing about model
//! accuracy or calibration. Numeric interpretation stays in Rust. Pangine sees
//! only the resulting categorical facts and uses no probability-like Relevance.

use pangine::{ConceptId, Pangine};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    BugReport,
    Billing,
    FeatureRequest,
}

#[derive(Clone, Copy, Debug)]
struct CapturedResponse {
    id: &'static str,
    category: Category,
    category_confidence_basis_points: u16,
    bug_severity_hundredths: u16,
    reproducible_basis_points: u16,
    refund_basis_points: u16,
    frustration_hundredths: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Question {
    Category,
    CategoryConfidence,
    BugSeverity,
    Reproducible,
    RefundRequested,
    Frustration,
}

impl Question {
    const fn label(self) -> &'static str {
        match self {
            Self::Category => "category",
            Self::CategoryConfidence => "category-confidence",
            Self::BugSeverity => "bug-severity",
            Self::Reproducible => "has-reproducible-steps",
            Self::RefundRequested => "refund-requested",
            Self::Frustration => "frustration",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    Any,
    Accepted,
    Low,
    BugReport,
    Billing,
    FeatureRequest,
    High,
    NotHigh,
    Yes,
    No,
}

impl Value {
    const fn label(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Accepted => "accepted",
            Self::Low => "low",
            Self::BugReport => "bug-report",
            Self::Billing => "billing",
            Self::FeatureRequest => "feature-request",
            Self::High => "high",
            Self::NotHigh => "not-high",
            Self::Yes => "yes",
            Self::No => "no",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Predicate {
    question: Question,
    value: Value,
}

const fn predicate(question: Question, value: Value) -> Predicate {
    Predicate { question, value }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Action {
    HumanReview,
    EscalateEngineering,
    BugBacklog,
    BillingWithRefundFlag,
    Billing,
    LogFeature,
    PriorityResponse,
}

impl Action {
    const fn label(self) -> &'static str {
        match self {
            Self::HumanReview => "human-review",
            Self::EscalateEngineering => "escalate-engineering",
            Self::BugBacklog => "bug-backlog",
            Self::BillingWithRefundFlag => "billing-with-refund-flag",
            Self::Billing => "billing",
            Self::LogFeature => "log-feature",
            Self::PriorityResponse => "priority-response",
        }
    }
}

#[derive(Clone, Copy)]
struct Rule {
    id: &'static str,
    action: Action,
    predicates: &'static [Predicate],
}

const LOW_CONFIDENCE: [Predicate; 1] = [predicate(Question::CategoryConfidence, Value::Low)];
const ESCALATE_BUG: [Predicate; 4] = [
    predicate(Question::CategoryConfidence, Value::Accepted),
    predicate(Question::Category, Value::BugReport),
    predicate(Question::BugSeverity, Value::High),
    predicate(Question::Reproducible, Value::Yes),
];
const BACKLOG_LOW_SEVERITY: [Predicate; 3] = [
    predicate(Question::CategoryConfidence, Value::Accepted),
    predicate(Question::Category, Value::BugReport),
    predicate(Question::BugSeverity, Value::NotHigh),
];
const BACKLOG_NO_REPRO: [Predicate; 4] = [
    predicate(Question::CategoryConfidence, Value::Accepted),
    predicate(Question::Category, Value::BugReport),
    predicate(Question::BugSeverity, Value::High),
    predicate(Question::Reproducible, Value::No),
];
const BILLING_FLAGGED: [Predicate; 3] =
    [predicate(Question::CategoryConfidence, Value::Accepted), predicate(Question::Category, Value::Billing), predicate(Question::RefundRequested, Value::Yes)];
const BILLING_PLAIN: [Predicate; 3] =
    [predicate(Question::CategoryConfidence, Value::Accepted), predicate(Question::Category, Value::Billing), predicate(Question::RefundRequested, Value::No)];
const FEATURE: [Predicate; 2] = [predicate(Question::CategoryConfidence, Value::Accepted), predicate(Question::Category, Value::FeatureRequest)];
const PRIORITY: [Predicate; 1] = [predicate(Question::Frustration, Value::High)];

const RULES: [Rule; 8] = [
    Rule { id: "low-confidence", action: Action::HumanReview, predicates: &LOW_CONFIDENCE },
    Rule { id: "escalate-bug", action: Action::EscalateEngineering, predicates: &ESCALATE_BUG },
    Rule { id: "backlog-low-severity", action: Action::BugBacklog, predicates: &BACKLOG_LOW_SEVERITY },
    Rule { id: "backlog-no-repro", action: Action::BugBacklog, predicates: &BACKLOG_NO_REPRO },
    Rule { id: "billing-flagged", action: Action::BillingWithRefundFlag, predicates: &BILLING_FLAGGED },
    Rule { id: "billing-plain", action: Action::Billing, predicates: &BILLING_PLAIN },
    Rule { id: "feature", action: Action::LogFeature, predicates: &FEATURE },
    Rule { id: "priority", action: Action::PriorityResponse, predicates: &PRIORITY },
];

#[derive(Debug, PartialEq, Eq)]
struct Explanation {
    rule: &'static str,
    response: &'static str,
    judgments: BTreeSet<Predicate>,
}

type Decision = BTreeMap<Action, Explanation>;

struct Run {
    decision: Decision,
    retained_text_bytes: usize,
    unmatched_rules: BTreeSet<&'static str>,
}

fn response_facts(response: CapturedResponse) -> BTreeMap<Question, Value> {
    BTreeMap::from([
        (
            Question::Category,
            match response.category {
                Category::BugReport => Value::BugReport,
                Category::Billing => Value::Billing,
                Category::FeatureRequest => Value::FeatureRequest,
            },
        ),
        (Question::CategoryConfidence, if response.category_confidence_basis_points < 5_000 { Value::Low } else { Value::Accepted }),
        (Question::BugSeverity, if response.bug_severity_hundredths > 150 { Value::High } else { Value::NotHigh }),
        (Question::Reproducible, if response.reproducible_basis_points > 6_000 { Value::Yes } else { Value::No }),
        (Question::RefundRequested, if response.refund_basis_points > 7_000 { Value::Yes } else { Value::No }),
        (Question::Frustration, if response.frustration_hundredths > 150 { Value::High } else { Value::NotHigh }),
    ])
}

fn current_response<'a>(responses: &'a [CapturedResponse], current: &str) -> &'a CapturedResponse {
    responses.iter().find(|response| response.id == current).unwrap_or_else(|| panic!("missing response {current}"))
}

// BEGIN DIRECT TYPESAFE-STYLE ROUTER

fn direct_actions(response: CapturedResponse) -> BTreeSet<Action> {
    let mut actions = BTreeSet::new();
    if response.category_confidence_basis_points < 5_000 {
        actions.insert(Action::HumanReview);
    } else {
        match response.category {
            Category::BugReport if response.bug_severity_hundredths > 150 && response.reproducible_basis_points > 6_000 => {
                actions.insert(Action::EscalateEngineering);
            }
            Category::BugReport => {
                actions.insert(Action::BugBacklog);
            }
            Category::Billing if response.refund_basis_points > 7_000 => {
                actions.insert(Action::BillingWithRefundFlag);
            }
            Category::Billing => {
                actions.insert(Action::Billing);
            }
            Category::FeatureRequest => {
                actions.insert(Action::LogFeature);
            }
        }
    }
    if response.frustration_hundredths > 150 {
        actions.insert(Action::PriorityResponse);
    }
    actions
}

// END DIRECT TYPESAFE-STYLE ROUTER
// BEGIN RECORD CONSUMER

fn run_records(responses: &[CapturedResponse], current: &str) -> Run {
    let response = *current_response(responses, current);
    let facts = response_facts(response);
    let mut decision = BTreeMap::new();
    let mut unmatched_rules = BTreeSet::new();
    for rule in RULES {
        if rule.predicates.iter().all(|required| facts.get(&required.question) == Some(&required.value)) {
            let explanation = Explanation { rule: rule.id, response: response.id, judgments: rule.predicates.iter().copied().collect() };
            assert!(decision.insert(rule.action, explanation).is_none(), "rules must be mutually exclusive for one action");
        } else {
            unmatched_rules.insert(rule.id);
        }
    }
    let retained_text_bytes = format!("{responses:?}{facts:?}{decision:?}").len();
    Run { decision, retained_text_bytes, unmatched_rules }
}

fn mismatches(response: CapturedResponse, rule: Rule) -> Vec<(Question, Value, Value)> {
    let facts = response_facts(response);
    rule.predicates
        .iter()
        .filter_map(|required| {
            let actual = facts[&required.question];
            (actual != required.value).then_some((required.question, required.value, actual))
        })
        .collect()
}

// END RECORD CONSUMER
// BEGIN PANGINE CONSUMER

#[derive(Clone, Copy)]
enum SourceLabel {
    Judgment { response: &'static str, predicate: Predicate },
    Wildcard(&'static str),
    Selection,
    Rule(&'static str),
}

fn run_pangine(responses: &[CapturedResponse], current: &str) -> Run {
    let mut pangine = Pangine::new();
    let mut labels = BTreeMap::new();
    let mut retained_text_bytes = format!("{responses:?}").len();
    for response in responses {
        for (question, value) in response_facts(*response) {
            let predicate = Predicate { question, value };
            let text = format!("[{}]->[{}]->[{}]", response.id, question.label(), value.label());
            let source = concept(&mut pangine, &text);
            labels.insert(source, SourceLabel::Judgment { response: response.id, predicate });
            concept(&mut pangine, &format!("{{program}} ~= {text}"));
            retained_text_bytes += text.len();
            let wildcard = format!("[{}]->[{}]->[any]", response.id, question.label());
            let source = concept(&mut pangine, &wildcard);
            labels.insert(source, SourceLabel::Wildcard(response.id));
            concept(&mut pangine, &format!("{{program}} ~= {wildcard}"));
            retained_text_bytes += wildcard.len();
        }
    }
    const QUESTIONS: [Question; 6] =
        [Question::Category, Question::CategoryConfidence, Question::BugSeverity, Question::Reproducible, Question::RefundRequested, Question::Frustration];
    for rule in RULES {
        let requirements = rule.predicates.iter().map(|required| (required.question, required.value)).collect::<BTreeMap<_, _>>();
        let mut text = QUESTIONS
            .iter()
            .map(|question| {
                let value = requirements.get(question).copied().unwrap_or(Value::Any);
                format!("([{}]->[{}]->[{}])", rule.id, question.label(), value.label())
            })
            .collect::<String>();
        text.push_str(&format!("([{}]->[action]->[{}])", rule.id, rule.action.label()));
        let source = concept(&mut pangine, &text);
        labels.insert(source, SourceLabel::Rule(rule.id));
        concept(&mut pangine, &format!("{{program}} ~= {text}"));
        retained_text_bytes += text.len();
    }
    let selection_text = format!("[selection]->[current]->[{current}]");
    let selection = concept(&mut pangine, &selection_text);
    labels.insert(selection, SourceLabel::Selection);
    concept(&mut pangine, &format!("{{program}} ~= {selection_text}"));
    retained_text_bytes += selection_text.len();

    let query = "{program} @
        ([selection]->[current]->{response})
        ({response}->[category]->{category})
        ({response}->[category-confidence]->{category-confidence})
        ({response}->[bug-severity]->{bug-severity})
        ({response}->[has-reproducible-steps]->{reproducible})
        ({response}->[refund-requested]->{refund})
        ({response}->[frustration]->{frustration})
        ({rule}->[category]->{category})
        ({rule}->[category-confidence]->{category-confidence})
        ({rule}->[bug-severity]->{bug-severity})
        ({rule}->[has-reproducible-steps]->{reproducible})
        ({rule}->[refund-requested]->{refund})
        ({rule}->[frustration]->{frustration})
        ({rule}->[action]->{action})";
    retained_text_bytes += query.len();
    concept(&mut pangine, query);
    let action = pangine.reference_percept("action");
    for possibility in pangine.answer_view(&action).expect("action projection").possibilities(&mut pangine).expect("inspect actions") {
        assert!(possibility.sources().iter().all(|source| matches!(labels[source.concept()], SourceLabel::Rule(_))));
    }
    let projection =
        concept(&mut pangine, "{response}->{category}->{category-confidence}->{bug-severity}->{reproducible}->{refund}->{frustration}->{rule}->{action}");
    let answer = pangine.answer_view(&projection).expect("at least one routing rule");
    let mut decision = BTreeMap::new();
    let mut matched_rules = BTreeSet::new();
    for possibility in answer.possibilities(&mut pangine).expect("inspect routing evidence") {
        let rendered_sources = possibility.sources().iter().map(|source| pangine.format_concept(source.concept(), false)).collect::<Vec<_>>();
        let mut found_rule = None;
        let mut found_response = None;
        let mut judgments = BTreeSet::new();
        let mut wildcards = 0;
        for source in possibility.sources() {
            match labels[source.concept()] {
                SourceLabel::Judgment { response, predicate } => {
                    assert!(found_response.replace(response).is_none_or(|previous| previous == response));
                    judgments.insert(predicate);
                }
                SourceLabel::Wildcard(response) => {
                    assert!(found_response.replace(response).is_none_or(|previous| previous == response));
                    wildcards += 1;
                }
                SourceLabel::Selection => {}
                SourceLabel::Rule(id) => {
                    assert!(found_rule.replace(id).is_none());
                }
            }
        }
        let rule_id = found_rule.expect("policy source");
        let rule = RULES.iter().find(|rule| rule.id == rule_id).copied().expect("known rule");
        assert_eq!(judgments.len() + wildcards, 6, "one actual or wildcard source per response field");
        let explanation =
            Explanation { rule: rule_id, response: found_response.unwrap_or_else(|| panic!("missing response sources: {rendered_sources:?}")), judgments };
        assert!(decision.insert(rule.action, explanation).is_none(), "rules must be mutually exclusive for one action");
        matched_rules.insert(rule_id);
    }
    let unmatched_rules = RULES.iter().map(|rule| rule.id).filter(|id| !matched_rules.contains(id)).collect();
    retained_text_bytes += pangine.format_concept(&pangine.linked_answer_value(&projection).expect("linked routing Answer"), false).len();
    Run { decision, retained_text_bytes, unmatched_rules }
}

fn concept(pangine: &mut Pangine, text: &str) -> ConceptId {
    pangine.reference_concept(text).unwrap_or_else(|error| panic!("{text}: {error}")).expect("nonempty Concept")
}

// END PANGINE CONSUMER

const BUG_ESCALATE: CapturedResponse = CapturedResponse {
    id: "bug-escalate",
    category: Category::BugReport,
    category_confidence_basis_points: 9_000,
    bug_severity_hundredths: 200,
    reproducible_basis_points: 8_000,
    refund_basis_points: 9_000,
    frustration_hundredths: 180,
};

const BUG_BACKLOG: CapturedResponse = CapturedResponse {
    id: "bug-backlog",
    category: Category::BugReport,
    category_confidence_basis_points: 9_000,
    bug_severity_hundredths: 151,
    reproducible_basis_points: 6_000,
    refund_basis_points: 9_000,
    frustration_hundredths: 150,
};

const BILLING_FLAG: CapturedResponse = CapturedResponse {
    id: "billing-flag",
    category: Category::Billing,
    category_confidence_basis_points: 8_000,
    bug_severity_hundredths: 200,
    reproducible_basis_points: 8_000,
    refund_basis_points: 7_001,
    frustration_hundredths: 100,
};

const FEATURE_PRIORITY: CapturedResponse = CapturedResponse {
    id: "feature-priority",
    category: Category::FeatureRequest,
    category_confidence_basis_points: 7_500,
    bug_severity_hundredths: 200,
    reproducible_basis_points: 8_000,
    refund_basis_points: 9_000,
    frustration_hundredths: 151,
};

const LOW_CONFIDENCE_BUG: CapturedResponse = CapturedResponse {
    id: "low-confidence-bug",
    category: Category::BugReport,
    category_confidence_basis_points: 4_999,
    bug_severity_hundredths: 200,
    reproducible_basis_points: 8_000,
    refund_basis_points: 9_000,
    frustration_hundredths: 100,
};

#[test]
#[ignore = "warning: captured-response-shaped fixtures test composition mechanics, not Jev accuracy or calibrated action thresholds"]
fn fan_out_routes_match_while_irrelevant_speculative_judgments_stay_out_of_reasons() {
    for response in [BUG_ESCALATE, BUG_BACKLOG, BILLING_FLAG, FEATURE_PRIORITY, LOW_CONFIDENCE_BUG] {
        let pangine = run_pangine(&[response], response.id);
        let records = run_records(&[response], response.id);
        assert_eq!(pangine.decision, records.decision, "{response:?}");
        assert_eq!(pangine.decision.keys().copied().collect::<BTreeSet<_>>(), direct_actions(response), "{response:?}");
    }

    let bug = run_pangine(&[BUG_ESCALATE], BUG_ESCALATE.id).decision;
    assert!(!bug[&Action::EscalateEngineering].judgments.iter().any(|judgment| judgment.question == Question::RefundRequested));
    let billing = run_pangine(&[BILLING_FLAG], BILLING_FLAG.id).decision;
    assert!(!billing[&Action::BillingWithRefundFlag]
        .judgments
        .iter()
        .any(|judgment| matches!(judgment.question, Question::BugSeverity | Question::Reproducible)));
}

#[test]
#[ignore = "warning: version identities preserve captured judgments but do not define replacement, retention, or audit policy"]
fn changing_one_judgment_revises_the_route_while_both_versions_remain_addressable() {
    let first = CapturedResponse { id: "ticket-9-v1", refund_basis_points: 7_000, ..BILLING_FLAG };
    let revised = CapturedResponse { id: "ticket-9-v2", refund_basis_points: 7_001, ..first };
    let responses = [first, revised];

    let before = run_pangine(&responses, first.id);
    let after = run_pangine(&responses, revised.id);
    assert_eq!(before.decision, run_records(&responses, first.id).decision);
    assert_eq!(after.decision, run_records(&responses, revised.id).decision);
    assert_eq!(before.decision.keys().copied().collect::<BTreeSet<_>>(), BTreeSet::from([Action::Billing]));
    assert_eq!(after.decision.keys().copied().collect::<BTreeSet<_>>(), BTreeSet::from([Action::BillingWithRefundFlag]));
    assert!(before.decision.values().all(|explanation| explanation.response == first.id));
    assert!(after.decision.values().all(|explanation| explanation.response == revised.id));
}

#[test]
#[ignore = "warning: an unmatched Pangine policy row has no near-miss explanation; the adapter must replay its typed predicates to answer why not"]
fn why_not_requires_the_same_host_policy_that_the_pangine_adapter_serializes() {
    let pangine = run_pangine(&[BUG_BACKLOG], BUG_BACKLOG.id);
    assert!(pangine.unmatched_rules.contains("escalate-bug"));
    let escalation = RULES.iter().find(|rule| rule.id == "escalate-bug").copied().unwrap();
    assert_eq!(
        mismatches(BUG_BACKLOG, escalation),
        vec![(Question::Reproducible, Value::Yes, Value::No)],
        "the Pangine result omits this policy row; this why-not fact comes from replaying the host's typed rule"
    );
}

#[test]
#[ignore = "warning: deterministic fixtures compare routing mechanics and implementation cost, not model behavior"]
fn routing_matrix_matches_documented_control_flow_and_reports_adapter_cost() {
    let mut pangine_time = Duration::ZERO;
    let mut records_time = Duration::ZERO;
    let mut cases = 0;
    for category in [Category::BugReport, Category::Billing, Category::FeatureRequest] {
        for category_confidence_basis_points in [4_999, 5_000] {
            for bug_severity_hundredths in [150, 151] {
                for reproducible_basis_points in [6_000, 6_001] {
                    for refund_basis_points in [7_000, 7_001] {
                        for frustration_hundredths in [150, 151] {
                            let response = CapturedResponse {
                                id: "matrix",
                                category,
                                category_confidence_basis_points,
                                bug_severity_hundredths,
                                reproducible_basis_points,
                                refund_basis_points,
                                frustration_hundredths,
                            };
                            let start = Instant::now();
                            let pangine = run_pangine(&[response], response.id);
                            pangine_time += start.elapsed();
                            let start = Instant::now();
                            let records = run_records(&[response], response.id);
                            records_time += start.elapsed();
                            assert_eq!(pangine.decision, records.decision, "{response:?}");
                            assert_eq!(pangine.decision.keys().copied().collect::<BTreeSet<_>>(), direct_actions(response), "{response:?}");
                            cases += 1;
                        }
                    }
                }
            }
        }
    }

    let pangine = run_pangine(&[BUG_ESCALATE], BUG_ESCALATE.id);
    let records = run_records(&[BUG_ESCALATE], BUG_ESCALATE.id);
    println!("TypeSafe composition comparison: {cases} boundary cases; end-to-end Pangine={pangine_time:?}, records={records_time:?}");
    println!(
        "retained text proxy (raw fixture plus encoded state/query/answer or facts/report): Pangine={}, records={}",
        pangine.retained_text_bytes, records.retained_text_bytes
    );
    let source = include_str!("typesafe_composition.rs");
    for label in ["DIRECT TYPESAFE-STYLE ROUTER", "RECORD CONSUMER", "PANGINE CONSUMER"] {
        let begin = format!("// BEGIN {label}");
        let end = format!("// END {label}");
        let body = source.split_once(&begin).unwrap().1.split_once(&end).unwrap().0;
        println!("{label} nonblank lines: {}", body.lines().filter(|line| !line.trim().is_empty()).count());
    }
}
