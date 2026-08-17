use pangine::{AnswerView, ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

const DECISION_QUESTION: &str = "
    (['candidate']->[action]->['action'])
    (['candidate']->[tool]->['tool'])";

const HELPFUL_QUESTION: &str = "
    (['helpful-episode']->[action]->['helpful-action'])
    (['helpful-episode']->[tool]->['helpful-tool'])
    (['helpful-episode']->[outcome]->[helpful])";

const FAILED_QUESTION: &str = "
    (['failed-episode']->[action]->['failed-action'])
    (['failed-episode']->[tool]->['failed-tool'])
    (['failed-episode']->[outcome]->[failed])";

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
    assert_eq!(first.possibilities["{[inspect-symbols]->[dumpbin]}"].strength, 2);
    assert_eq!(first.top_ties(), BTreeSet::from(["{[inspect-symbols]->[dumpbin]}".to_owned()]));

    remember(&mut pangine, "episodes", "episode-dumpbin-failed-1", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("failed"));
    let second = decision_round(&mut pangine);
    assert_eq!(second.selected, first.selected);
    assert_eq!(second.possibilities["{[inspect-symbols]->[dumpbin]}"].strength, 1);
    assert_eq!(second.top_ties().len(), 2);

    remember(&mut pangine, "episodes", "episode-dumpbin-failed-2", &[("action", "inspect-symbols"), ("tool", "dumpbin")], Some("failed"));
    let third = decision_round(&mut pangine);
    assert_eq!(third.selected, must_ref(&mut pangine, "[inspect-symbols]->[link-map]"));
    assert_eq!(third.possibilities["{[inspect-symbols]->[dumpbin]}"].strength, 0);
    assert_eq!(third.possibilities.len(), 3);
    assert!(third.possibilities.values().all(|possibility| possibility.complete_rows == 1));

    let dumpbin_sources = &third.possibilities["{[inspect-symbols]->[dumpbin]}"].sources;
    assert!(dumpbin_sources.iter().any(|source| source.concept.contains("episode-dumpbin-helpful") && source.contribution == 1));
    assert!(dumpbin_sources.iter().any(|source| source.concept.contains("episode-dumpbin-failed-1") && source.contribution == -1));
    assert!(dumpbin_sources.iter().any(|source| source.concept.contains("episode-dumpbin-failed-2") && source.contribution == -1));
    assert!(dumpbin_sources.iter().filter(|source| source.subject == "['episodes']").all(|source| source.relevance == 1));
    assert_eq!(must_ref(&mut pangine, "$(['action']->['tool'])"), third.selected);
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

    must_ref(&mut pangine, &format!("['candidates'] @ {DECISION_QUESTION}"));
    must_ref(&mut pangine, &format!("['episodes'] @ {HELPFUL_QUESTION}"));
    must_ref(&mut pangine, &format!("['episodes'] @ {FAILED_QUESTION}"));
    assert_eq!(
        must_ref(&mut pangine, "['action']->['tool'] @+= ['helpful-action']->['helpful-tool']"),
        must_ref(&mut pangine, "x2([inspect-symbols]->[dumpbin])([inspect-symbols]->[link-map])([reconfigure]->[cmake])")
    );
    assert_eq!(
        must_ref(&mut pangine, "['action']->['tool'] @-= ['failed-action']->['failed-tool']"),
        must_ref(&mut pangine, "([inspect-symbols]->[link-map])([reconfigure]->[cmake])")
    );

    let shape = must_ref(&mut pangine, "['action']->['tool']");
    let answer = pangine.answer_view(&shape).expect("adjusted linked answer");
    let possibilities = inspect(&mut pangine, &answer);
    let dumpbin = &possibilities["{[inspect-symbols]->[dumpbin]}"];

    assert_eq!(possibilities.len(), 3);
    assert_eq!(dumpbin.strength, 0);
    assert_eq!(dumpbin.complete_rows, 1);
    assert!(dumpbin.sources.iter().any(|source| source.concept.contains("candidate-dumpbin") && source.contribution == 1));
    assert!(dumpbin.sources.iter().any(|source| source.concept.contains("episode-dumpbin-helpful") && source.contribution == 1));
    assert!(dumpbin.sources.iter().any(|source| source.concept.contains("episode-dumpbin-failed-1") && source.contribution == -1));
    assert!(dumpbin.sources.iter().any(|source| source.concept.contains("episode-dumpbin-failed-2") && source.contribution == -1));
    assert_eq!(must_ref(&mut pangine, "&['action']"), must_ref(&mut pangine, DECISION_QUESTION));
    assert_eq!(must_ref(&mut pangine, "^(['action']->['tool'])"), must_ref(&mut pangine, "[inspect-symbols]->[link-map]"));
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
    let shape = must_ref(&mut pangine, "([action]->['action'])([tool]->['tool'])([scope]->['scope'])");
    let useful_shape = must_ref(&mut pangine, "([action]->['useful-action'])([tool]->['useful-tool'])([scope]->['useful-scope'])");
    let failed_shape = must_ref(&mut pangine, "([action]->['failed-action'])([tool]->['failed-tool'])([scope]->['failed-scope'])");
    let base = pangine.answer_view(&shape).expect("three-output answer");
    assert_eq!(base.possibilities(&mut pangine).expect("base possibilities").iter().filter(|possibility| possibility.is_top_tie()).count(), 3);

    assert!(pangine.answer_view(&useful_shape).is_some());
    assert!(pangine.answer_view(&failed_shape).is_some());
    must_ref(
        &mut pangine,
        "([action]->['action'])([tool]->['tool'])([scope]->['scope']) @+= ([action]->['useful-action'])([tool]->['useful-tool'])([scope]->['useful-scope'])",
    );
    must_ref(
        &mut pangine,
        "([action]->['action'])([tool]->['tool'])([scope]->['scope']) @-= ([action]->['failed-action'])([tool]->['failed-tool'])([scope]->['failed-scope'])",
    );
    let adjusted = pangine.answer_view(&shape).expect("adjusted three-output answer");
    let possibilities = inspect(&mut pangine, &adjusted);
    let selected = must_ref(&mut pangine, "([action]->[inspect-installed-modes])([tool]->[pkgutil])([scope]->[installed-payload])");
    let selected_text = pangine.format_concept(&selected, false);
    assert_eq!(possibilities[&selected_text].strength, 2);
    assert_eq!(possibilities.values().filter(|possibility| possibility.is_top_tie).count(), 1);

    let choice = adjusted.choose(&mut pangine).expect("positive complete choice");
    assert_eq!(choice.selected(), &selected);
    choice.view().answer().publish(&mut pangine).expect("current three-output answer");
    assert_eq!(must_ref(&mut pangine, "$['action']"), must_ref(&mut pangine, "x2[inspect-installed-modes]"));
    assert_eq!(must_ref(&mut pangine, "$['tool']"), must_ref(&mut pangine, "x2[pkgutil]"));
    assert_eq!(must_ref(&mut pangine, "$['scope']"), must_ref(&mut pangine, "x2[installed-payload]"));
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
    contribution: i64,
}

fn decision_round(pangine: &mut Pangine) -> Round {
    must_ref(pangine, &format!("['candidates'] @ {DECISION_QUESTION}"));
    must_ref(pangine, &format!("['episodes'] @ {HELPFUL_QUESTION}"));
    must_ref(pangine, &format!("['episodes'] @ {FAILED_QUESTION}"));

    let shape = must_ref(pangine, "['action']->['tool']");
    let helpful_shape = must_ref(pangine, "['helpful-action']->['helpful-tool']");
    let failed_shape = must_ref(pangine, "['failed-action']->['failed-tool']");
    let base = pangine.answer_view(&shape).expect("complete candidate answer");
    let helpful = pangine.answer_view(&helpful_shape).expect("complete helpful answer");
    let failed = pangine.answer_view(&failed_shape).expect("complete failed answer");
    let adjusted = base.adjusted_by(pangine, &helpful, Relevance::DEFAULT).expect("matching helpful outcomes");
    let adjusted = adjusted.adjusted_by(pangine, &failed, Relevance::new(-1)).expect("matching failed outcomes");
    let possibilities = inspect(pangine, &adjusted);
    let choice = adjusted.choose(pangine).expect("positive complete choice");
    let selected = choice.selected().clone();
    choice.view().answer().publish(pangine).expect("current candidate answer");
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
                .sources()
                .iter()
                .map(|source| Source {
                    subject: pangine.format_concept(source.subject(), false),
                    concept: pangine.format_concept(source.concept(), false),
                    relevance: source.relevance().weight(),
                    contribution: source.contribution().weight(),
                })
                .collect();
            (
                value,
                Possibility {
                    strength: possibility.strength().weight(),
                    complete_rows: possibility.complete_rows(),
                    sources,
                    is_top_tie: possibility.is_top_tie(),
                },
            )
        })
        .collect()
}

fn ask_three_output_answers(pangine: &mut Pangine) {
    must_ref(pangine, "['candidates'] @ (['candidate']->[action]->['action'])(['candidate']->[tool]->['tool'])(['candidate']->[scope]->['scope'])");
    must_ref(
        pangine,
        "['outcomes'] @
            (['useful-episode']->[action]->['useful-action'])
            (['useful-episode']->[tool]->['useful-tool'])
            (['useful-episode']->[scope]->['useful-scope'])
            (['useful-episode']->[outcome]->[useful])",
    );
    must_ref(
        pangine,
        "['outcomes'] @
            (['failed-episode']->[action]->['failed-action'])
            (['failed-episode']->[tool]->['failed-tool'])
            (['failed-episode']->[scope]->['failed-scope'])
            (['failed-episode']->[outcome]->[failed])",
    );
}

fn remember(pangine: &mut Pangine, owner: &str, id: &str, fields: &[(&str, &str)], outcome: Option<&str>) {
    let mut relations = fields.iter().map(|(name, value)| format!("([{id}]->[{name}]->[{value}])")).collect::<String>();
    if let Some(outcome) = outcome {
        relations.push_str(&format!("([{id}]->[outcome]->[{outcome}])"));
    }
    must_ref(pangine, &format!("['{owner}'] ~= {relations}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
