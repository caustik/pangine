use pangine::{AnswerView, ConceptId, Pangine};
use std::collections::{BTreeMap, BTreeSet};

const DECISION_QUESTION: &str = "
    ({candidate}->[action]->{action})
    ({candidate}->[tool]->{tool})";

const HELPFUL_QUESTION: &str = "
    ({helpful-episode}->[action]->{helpful-action})
    ({helpful-episode}->[tool]->{helpful-tool})
    ({helpful-episode}->[outcome]->[helpful])";

const FAILED_QUESTION: &str = "
    ({failed-episode}->[action]->{failed-action})
    ({failed-episode}->[tool]->{failed-tool})
    ({failed-episode}->[outcome]->[failed])";

const RETAINED_HELPFUL_QUESTION: &str = "
    ({retained-episode}->[action]->{retained-action})
    ({retained-episode}->[tool]->{retained-tool})
    ({retained-episode}->[outcome]->[helpful])";

const CANDIDATES: [(&str, &str); 3] = [("inspect-symbols", "dumpbin"), ("inspect-symbols", "link-map"), ("reconfigure", "cmake")];

#[test]
fn repeated_outcomes_change_a_later_complete_choice_without_removing_untried_possibilities() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "candidates", "candidate-dumpbin", &[("action", "inspect-symbols"), ("tool", "dumpbin")], None);
    remember(&mut pangine, "candidates", "candidate-map", &[("action", "inspect-symbols"), ("tool", "link-map")], None);
    remember(&mut pangine, "candidates", "candidate-reconfigure", &[("action", "reconfigure"), ("tool", "cmake")], None);
    remember(&mut pangine, "episodes", "episode-dumpbin-helpful", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("helpful"));
    remember(&mut pangine, "episodes", "episode-reconfigure-failed-1", &[("action", "reconfigure"), ("tool", "cmake")], Some("failed"));
    remember(&mut pangine, "episodes", "episode-reconfigure-failed-2", &[("action", "reconfigure"), ("tool", "cmake")], Some("failed"));

    let first = decision_round(&mut pangine);
    assert_eq!(first.selected, must_ref(&mut pangine, "[inspect-symbols]->[dumpbin]"));
    assert_eq!(first.possibilities["[inspect-symbols]->[dumpbin]"].strength, 2);
    assert_eq!(first.top_ties(), BTreeSet::from(["[inspect-symbols]->[dumpbin]".to_owned()]));

    remember(&mut pangine, "episodes", "episode-dumpbin-failed-1", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("failed"));
    let second = decision_round(&mut pangine);
    assert_eq!(second.selected, first.selected);
    assert_eq!(second.possibilities["[inspect-symbols]->[dumpbin]"].strength, 1);
    assert_eq!(second.top_ties().len(), 2);

    remember(&mut pangine, "episodes", "episode-dumpbin-failed-2", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("failed"));
    let third = decision_round(&mut pangine);
    assert_eq!(third.selected, must_ref(&mut pangine, "[inspect-symbols]->[link-map]"));
    assert_eq!(third.possibilities["[inspect-symbols]->[dumpbin]"].strength, 0);
    assert_eq!(third.possibilities.len(), 3);
    assert!(third.possibilities.values().all(|possibility| possibility.complete_rows == 1));

    let dumpbin_sources = &third.possibilities["[inspect-symbols]->[dumpbin]"].sources;
    assert!(dumpbin_sources.iter().any(|source| source.concept.contains("episode-dumpbin-helpful") && source.weight == 1));
    assert!(dumpbin_sources.iter().any(|source| source.concept.contains("episode-dumpbin-failed-1") && source.weight == -1));
    assert!(dumpbin_sources.iter().any(|source| source.concept.contains("episode-dumpbin-failed-2") && source.weight == -1));
    assert!(dumpbin_sources.iter().filter(|source| source.subject == "{episodes}").all(|source| source.relevance == 1));
    assert_eq!(must_ref(&mut pangine, "$({action}->{tool})"), third.selected);
}

#[test]
fn language_adjustment_keeps_zero_strength_rows_and_their_sources_in_the_linked_answer() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "candidates", "candidate-dumpbin", &[("action", "inspect-symbols"), ("tool", "dumpbin")], None);
    remember(&mut pangine, "candidates", "candidate-map", &[("action", "inspect-symbols"), ("tool", "link-map")], None);
    remember(&mut pangine, "candidates", "candidate-reconfigure", &[("action", "reconfigure"), ("tool", "cmake")], None);
    remember(&mut pangine, "episodes", "episode-dumpbin-helpful", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("helpful"));
    remember(&mut pangine, "episodes", "episode-dumpbin-failed-1", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("failed"));
    remember(&mut pangine, "episodes", "episode-dumpbin-failed-2", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("failed"));

    must_ref(&mut pangine, &format!("{{candidates}} @ {DECISION_QUESTION}"));
    must_ref(&mut pangine, &format!("{{episodes}} @ {HELPFUL_QUESTION}"));
    must_ref(&mut pangine, &format!("{{episodes}} @ {FAILED_QUESTION}"));
    assert_eq!(
        must_ref(&mut pangine, "{action}->{tool} @+= {helpful-action}->{helpful-tool}"),
        must_ref(&mut pangine, "x2([inspect-symbols]->[dumpbin])([inspect-symbols]->[link-map])([reconfigure]->[cmake])")
    );
    assert_eq!(
        must_ref(&mut pangine, "{action}->{tool} @-= {failed-action}->{failed-tool}"),
        must_ref(&mut pangine, "([inspect-symbols]->[link-map])([reconfigure]->[cmake])")
    );

    let shape = must_ref(&mut pangine, "{action}->{tool}");
    let answer = pangine.answer_view(&shape).expect("adjusted linked answer");
    let possibilities = inspect(&mut pangine, &answer);
    let dumpbin = &possibilities["[inspect-symbols]->[dumpbin]"];

    assert_eq!(possibilities.len(), 3);
    assert_eq!(dumpbin.strength, 0);
    assert_eq!(dumpbin.complete_rows, 1);
    assert!(dumpbin.sources.iter().any(|source| source.concept.contains("candidate-dumpbin") && source.weight == 1));
    assert!(dumpbin.sources.iter().any(|source| source.concept.contains("episode-dumpbin-helpful") && source.weight == 1));
    assert!(dumpbin.sources.iter().any(|source| source.concept.contains("episode-dumpbin-failed-1") && source.weight == -1));
    assert!(dumpbin.sources.iter().any(|source| source.concept.contains("episode-dumpbin-failed-2") && source.weight == -1));
    assert_eq!(must_ref(&mut pangine, "&{action}"), must_ref(&mut pangine, DECISION_QUESTION));
    assert_eq!(must_ref(&mut pangine, "^({action}->{tool})"), must_ref(&mut pangine, "[inspect-symbols]->[link-map]"));
}

#[test]
fn the_same_answer_cycle_handles_three_outputs_in_an_unordered_shape() {
    let mut pangine = Pangine::new();
    remember(&mut pangine, "candidates", "candidate-signature", &[("action", "inspect-signature"), ("tool", "codesign"), ("scope", "app-bundle")], None);
    remember(
        &mut pangine,
        "candidates",
        "candidate-modes",
        &[("action", "inspect-installed-modes"), ("tool", "pkgutil"), ("scope", "installed-payload")],
        None,
    );
    remember(&mut pangine, "candidates", "candidate-architecture", &[("action", "inspect-architecture"), ("tool", "file"), ("scope", "app-bundle")], None);
    remember(
        &mut pangine,
        "outcomes",
        "episode-modes-useful",
        &[("action", "inspect-installed-modes"), ("tool", "pkgutil"), ("scope", "installed-payload")],
        Some("useful"),
    );
    remember(
        &mut pangine,
        "outcomes",
        "episode-signature-failed",
        &[("action", "inspect-signature"), ("tool", "codesign"), ("scope", "app-bundle")],
        Some("failed"),
    );

    ask_three_output_answers(&mut pangine);
    let shape = must_ref(&mut pangine, "([action]->{action})([tool]->{tool})([scope]->{scope})");
    let useful_shape = must_ref(&mut pangine, "([action]->{useful-action})([tool]->{useful-tool})([scope]->{useful-scope})");
    let failed_shape = must_ref(&mut pangine, "([action]->{failed-action})([tool]->{failed-tool})([scope]->{failed-scope})");
    let base = pangine.answer_view(&shape).expect("three-output answer");
    assert_eq!(base.possibilities(&mut pangine).expect("base possibilities").iter().filter(|possibility| possibility.is_top_tie()).count(), 3);

    assert!(pangine.answer_view(&useful_shape).is_some());
    assert!(pangine.answer_view(&failed_shape).is_some());
    must_ref(
        &mut pangine,
        "([action]->{action})([tool]->{tool})([scope]->{scope}) @+= ([action]->{useful-action})([tool]->{useful-tool})([scope]->{useful-scope})",
    );
    must_ref(
        &mut pangine,
        "([action]->{action})([tool]->{tool})([scope]->{scope}) @-= ([action]->{failed-action})([tool]->{failed-tool})([scope]->{failed-scope})",
    );
    let adjusted = pangine.answer_view(&shape).expect("adjusted three-output answer");
    let possibilities = inspect(&mut pangine, &adjusted);
    let selected = must_ref(&mut pangine, "([action]->[inspect-installed-modes])([tool]->[pkgutil])([scope]->[installed-payload])");
    let selected_text = pangine.format_concept(&selected, false);
    assert_eq!(possibilities[&selected_text].strength, 2);
    assert_eq!(possibilities.values().filter(|possibility| possibility.is_top_tie).count(), 1);

    let choice = adjusted.choose(&mut pangine).expect("positive complete choice");
    assert_eq!(choice.selected(), &selected);
    assert_eq!(must_ref(&mut pangine, "^(([action]->{action})([tool]->{tool})([scope]->{scope}))"), selected);
    assert_eq!(must_ref(&mut pangine, "${action}"), must_ref(&mut pangine, "x2[inspect-installed-modes]"));
    assert_eq!(must_ref(&mut pangine, "${tool}"), must_ref(&mut pangine, "x2[pkgutil]"));
    assert_eq!(must_ref(&mut pangine, "${scope}"), must_ref(&mut pangine, "x2[installed-payload]"));
}

#[test]
fn answers_carried_across_append_only_cycles_match_answers_asked_again_over_all_history() {
    const CYCLES: usize = 200;
    let mut replay = Pangine::new();
    let mut carried = Pangine::new();
    for pangine in [&mut replay, &mut carried] {
        remember_candidates(pangine);
    }
    must_ref(&mut carried, &format!("{{candidates}} @ {DECISION_QUESTION}"));

    let mut choices = BTreeSet::new();
    for cycle in 0..CYCLES {
        // Each cycle appends one new episode. In each block of ten cycles one
        // candidate fails and the others help, so the choice keeps moving.
        let candidate = cycle % CANDIDATES.len();
        let outcome = if (cycle / 10) % CANDIDATES.len() == candidate { "failed" } else { "helpful" };
        let (action, tool) = CANDIDATES[candidate];
        let id = format!("episode-{cycle}");
        let fields = [("action", action), ("tool", tool)];

        remember(&mut replay, "episodes", &id, &fields, Some(outcome));
        let expected = replayed_reading(&mut replay, "episodes");

        // The carried answer takes only the new episode's evidence.
        carried.reference_concept("{new} = []").expect("cleared new episodes");
        remember(&mut carried, "new", &id, &fields, Some(outcome));
        adjust_by_outcomes(&mut carried, "new");

        assert_eq!(reading(&mut carried), expected, "cycle {cycle}");
        choices.extend(expected.selected);
    }
    assert!(choices.len() > 1, "the outcomes must change the choice: {choices:?}");
}

#[test]
fn a_carried_answer_keeps_cancelled_proof_when_a_source_is_replaced() {
    let original = relations("revisable-episode", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("helpful"));
    let replacement = relations("revisable-episode", &[("action", "inspect-symbols"), ("tool", "link-map")], Some("failed"));
    let mut replay = Pangine::new();
    let mut carried = Pangine::new();
    for pangine in [&mut replay, &mut carried] {
        remember_candidates(pangine);
    }

    // The carried answer counts the original episode and keeps its evidence
    // under separate outputs, so it can withdraw that evidence later.
    must_ref(&mut carried, &format!("{{revisable}} = {original}"));
    must_ref(&mut carried, &format!("{{candidates}} @ {DECISION_QUESTION}"));
    adjust_by_outcomes(&mut carried, "revisable");
    must_ref(&mut carried, &format!("{{revisable}} @ {RETAINED_HELPFUL_QUESTION}"));

    // After the replacement, replay asks again over the replaced source, while
    // the carried answer withdraws the old evidence and adds the new.
    must_ref(&mut carried, &format!("{{revisable}} = {replacement}"));
    carried.reference_concept("{action}->{tool} @-= {retained-action}->{retained-tool}").expect("withdrawn evidence");
    adjust_by_outcomes(&mut carried, "revisable");
    must_ref(&mut replay, &format!("{{revisable}} = {replacement}"));
    assert_eq!(reading(&mut carried), replayed_reading(&mut replay, "revisable"));

    // The readings match, but the carried answer still holds the withdrawn
    // proof, once added and once subtracted, so replay stays the exact operation.
    let dumpbin_support = |pangine: &mut Pangine| {
        let shape = must_ref(pangine, "{action}->{tool}");
        let answer = pangine.answer_view(&shape).expect("decision answer");
        let possibilities = inspect(pangine, &answer);
        let dumpbin = &possibilities["[inspect-symbols]->[dumpbin]"];
        dumpbin.sources.iter().filter(|source| source.concept.contains("revisable-episode")).map(|source| source.weight).collect::<Vec<_>>()
    };
    assert_eq!(dumpbin_support(&mut replay), Vec::<i64>::new());
    let mut carried_weights = dumpbin_support(&mut carried);
    carried_weights.sort_unstable();
    assert_eq!(carried_weights, vec![-1, 1]);
}

/// What a program reads from the decision answer: its `$` value, each
/// possibility's strength, probability, and tie, and what `^` would choose.
#[derive(Debug, PartialEq)]
struct Reading {
    value: String,
    possibilities: Vec<(String, i64, String, bool)>,
    selected: Option<String>,
}

fn reading(pangine: &mut Pangine) -> Reading {
    let value = pangine.reference_concept("$({action}->{tool})").expect("readable decision answer");
    let value = value.map_or_else(|| "[]".to_owned(), |value| pangine.format_concept(&value, false));
    let shape = must_ref(pangine, "{action}->{tool}");
    let answer = pangine.answer_view(&shape).expect("decision answer");
    let possibilities = answer
        .possibilities(pangine)
        .expect("inspectable decision answer")
        .iter()
        .map(|possibility| {
            (
                pangine.format_concept(possibility.value(), false),
                possibility.strength().count(),
                possibility.probability().to_string(),
                possibility.is_top_tie(),
            )
        })
        .collect();
    let selected = answer.choose(pangine).map(|choice| pangine.format_concept(choice.selected(), false));
    Reading { value, possibilities, selected }
}

/// Asks the decision question again and adjusts it by every outcome in `source`.
fn replayed_reading(pangine: &mut Pangine, source: &str) -> Reading {
    must_ref(pangine, &format!("{{candidates}} @ {DECISION_QUESTION}"));
    adjust_by_outcomes(pangine, source);
    reading(pangine)
}

/// Adds the helpful outcomes in `source` to the decision answer and subtracts
/// the failed ones. A question that finds nothing leaves no answer to adjust by.
fn adjust_by_outcomes(pangine: &mut Pangine, source: &str) {
    if pangine.reference_concept(&format!("{{{source}}} @ {HELPFUL_QUESTION}")).expect("helpful question").is_some() {
        pangine.reference_concept("{action}->{tool} @+= {helpful-action}->{helpful-tool}").expect("helpful outcomes");
    }
    if pangine.reference_concept(&format!("{{{source}}} @ {FAILED_QUESTION}")).expect("failed question").is_some() {
        pangine.reference_concept("{action}->{tool} @-= {failed-action}->{failed-tool}").expect("failed outcomes");
    }
}

fn remember_candidates(pangine: &mut Pangine) {
    for (index, (action, tool)) in CANDIDATES.iter().enumerate() {
        remember(pangine, "candidates", &format!("candidate-{index}"), &[("action", action), ("tool", tool)], None);
    }
}

struct Round {
    selected: ConceptId,
    possibilities: BTreeMap<String, Possibility>,
}

impl Round {
    fn top_ties(&self) -> BTreeSet<String> {
        self.possibilities.iter().filter_map(|(value, possibility)| possibility.is_top_tie.then_some(value.clone())).collect()
    }
}

struct Possibility {
    strength: i64,
    complete_rows: usize,
    sources: Vec<Source>,
    is_top_tie: bool,
}

struct Source {
    subject: String,
    concept: String,
    relevance: i64,
    weight: i64,
}

fn decision_round(pangine: &mut Pangine) -> Round {
    must_ref(pangine, &format!("{{candidates}} @ {DECISION_QUESTION}"));
    must_ref(pangine, &format!("{{episodes}} @ {HELPFUL_QUESTION}"));
    must_ref(pangine, &format!("{{episodes}} @ {FAILED_QUESTION}"));

    pangine.reference_concept("{action}->{tool} @+= {helpful-action}->{helpful-tool}").expect("matching helpful outcomes");
    pangine.reference_concept("{action}->{tool} @-= {failed-action}->{failed-tool}").expect("matching failed outcomes");
    let shape = must_ref(pangine, "{action}->{tool}");
    let adjusted = pangine.answer_view(&shape).expect("adjusted candidate answer");
    let possibilities = inspect(pangine, &adjusted);
    let selected = must_ref(pangine, "^({action}->{tool})");
    Round { selected, possibilities }
}

fn inspect(pangine: &mut Pangine, answer: &AnswerView) -> BTreeMap<String, Possibility> {
    answer
        .possibilities(pangine)
        .expect("inspectable answer")
        .into_iter()
        .map(|possibility| {
            let value = pangine.format_concept(possibility.value(), false);
            let sources = possibility
                .support()
                .iter()
                .flat_map(|support| {
                    support.sources().iter().map(|source| Source {
                        subject: pangine.format_concept(source.subject(), false),
                        concept: pangine.format_concept(source.concept(), false),
                        relevance: source.relevance().count(),
                        weight: support.weight().count(),
                    })
                })
                .collect();
            (
                value,
                Possibility {
                    strength: possibility.strength().count(),
                    complete_rows: possibility.complete_rows(),
                    sources,
                    is_top_tie: possibility.is_top_tie(),
                },
            )
        })
        .collect()
}

fn ask_three_output_answers(pangine: &mut Pangine) {
    must_ref(pangine, "{candidates} @ ({candidate}->[action]->{action})({candidate}->[tool]->{tool})({candidate}->[scope]->{scope})");
    must_ref(
        pangine,
        "{outcomes} @
            ({useful-episode}->[action]->{useful-action})
            ({useful-episode}->[tool]->{useful-tool})
            ({useful-episode}->[scope]->{useful-scope})
            ({useful-episode}->[outcome]->[useful])",
    );
    must_ref(
        pangine,
        "{outcomes} @
            ({failed-episode}->[action]->{failed-action})
            ({failed-episode}->[tool]->{failed-tool})
            ({failed-episode}->[scope]->{failed-scope})
            ({failed-episode}->[outcome]->[failed])",
    );
}

fn remember(pangine: &mut Pangine, owner: &str, id: &str, fields: &[(&str, &str)], outcome: Option<&str>) {
    must_ref(pangine, &format!("{{{owner}}} ~= {}", relations(id, fields, outcome)));
}

fn relations(id: &str, fields: &[(&str, &str)], outcome: Option<&str>) -> String {
    let mut relations = fields.iter().map(|(name, value)| format!("([{id}]->[{name}]->[{value}])")).collect::<String>();
    if let Some(outcome) = outcome {
        relations.push_str(&format!("([{id}]->[outcome]->[{outcome}])"));
    }
    relations
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
