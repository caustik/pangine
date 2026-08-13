//! Warning checks for application decisions that need complete answer rows.
//!
//! Candidate totals cannot recover which surrounding binding accompanied each
//! candidate. Even separate candidate/source, candidate/context, and
//! source/context totals cannot recover their full three-way pairing. The
//! application filters in this file remain external to Pangine.

use pangine::{ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

type Scores = BTreeMap<String, Relevance>;
type PairScores = BTreeMap<(String, String), Relevance>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ChoiceRow {
    source: String,
    context: String,
    candidate: String,
    relevance: Relevance,
}

#[test]
#[ignore = "warning: filtering complete rows by application context is not Pangine semantics"]
fn application_context_must_be_filtered_before_candidate_totals_are_collapsed() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "([case-1]->[route]->[north])([case-1]->[answer]->[A])", 1);
    experience(&mut pangine, "memory", "([case-2]->[route]->[south])([case-2]->[answer]->[A])", 10);
    experience(&mut pangine, "memory", "([case-3]->[route]->[north])([case-3]->[answer]->[B])", 5);

    let rows = complete_choice_rows(&mut pangine, &["memory"]);
    assert_eq!(rows.len(), 3);

    assert_eq!(
        rows,
        BTreeSet::from([
            ChoiceRow { source: "['memory']".to_owned(), context: "[north]".to_owned(), candidate: "[A]".to_owned(), relevance: Relevance::DEFAULT },
            ChoiceRow { source: "['memory']".to_owned(), context: "[north]".to_owned(), candidate: "[B]".to_owned(), relevance: Relevance::new(5) },
            ChoiceRow { source: "['memory']".to_owned(), context: "[south]".to_owned(), candidate: "[A]".to_owned(), relevance: Relevance::new(10) },
        ])
    );

    let premature_totals = candidate_totals(rows.iter());
    assert_eq!(premature_totals, score_map(&[("[A]", 11), ("[B]", 5)]));
    assert_eq!(only_greatest(&premature_totals), Some("[A]".to_owned()));

    let north_totals = candidate_totals(rows.iter().filter(|row| row.context == "[north]"));
    assert_eq!(north_totals, score_map(&[("[A]", 1), ("[B]", 5)]));
    assert_eq!(only_greatest(&north_totals), Some("[B]".to_owned()));
}

#[test]
#[ignore = "warning: source eligibility in an application context is not Pangine semantics"]
fn every_pairwise_total_can_match_while_complete_rows_choose_differently() {
    let first = choice_fixture(
        &[
            ("source-one", "([case-a-one]->[route]->[north])([case-a-one]->[answer]->[A])", 5),
            ("source-one", "([case-a-two]->[route]->[south])([case-a-two]->[answer]->[A])", 1),
            ("source-one", "([case-b-one]->[route]->[south])([case-b-one]->[answer]->[B])", 4),
            ("source-two", "([case-a-three]->[route]->[north])([case-a-three]->[answer]->[A])", 1),
            ("source-two", "([case-a-four]->[route]->[south])([case-a-four]->[answer]->[A])", 5),
            ("source-two", "([case-b-two]->[route]->[north])([case-b-two]->[answer]->[B])", 4),
        ],
        &["source-one", "source-two"],
    );
    let second = choice_fixture(
        &[
            ("source-one", "([case-a-one]->[route]->[north])([case-a-one]->[answer]->[A])", 1),
            ("source-one", "([case-a-two]->[route]->[south])([case-a-two]->[answer]->[A])", 5),
            ("source-one", "([case-b-one]->[route]->[north])([case-b-one]->[answer]->[B])", 4),
            ("source-two", "([case-a-three]->[route]->[north])([case-a-three]->[answer]->[A])", 5),
            ("source-two", "([case-a-four]->[route]->[south])([case-a-four]->[answer]->[A])", 1),
            ("source-two", "([case-b-two]->[route]->[south])([case-b-two]->[answer]->[B])", 4),
        ],
        &["source-one", "source-two"],
    );

    let candidate_source =
        pair_score_map(&[("[A]", "['source-one']", 6), ("[A]", "['source-two']", 6), ("[B]", "['source-one']", 4), ("[B]", "['source-two']", 4)]);
    assert_eq!(pair_totals(&first, |row| (row.candidate.clone(), row.source.clone())), candidate_source);
    assert_eq!(pair_totals(&second, |row| (row.candidate.clone(), row.source.clone())), candidate_source);

    let candidate_context = pair_score_map(&[("[A]", "[north]", 6), ("[A]", "[south]", 6), ("[B]", "[north]", 4), ("[B]", "[south]", 4)]);
    assert_eq!(pair_totals(&first, |row| (row.candidate.clone(), row.context.clone())), candidate_context);
    assert_eq!(pair_totals(&second, |row| (row.candidate.clone(), row.context.clone())), candidate_context);

    let source_context = pair_score_map(&[
        ("['source-one']", "[north]", 5),
        ("['source-one']", "[south]", 5),
        ("['source-two']", "[north]", 5),
        ("['source-two']", "[south]", 5),
    ]);
    assert_eq!(pair_totals(&first, |row| (row.source.clone(), row.context.clone())), source_context);
    assert_eq!(pair_totals(&second, |row| (row.source.clone(), row.context.clone())), source_context);

    let first_totals = candidate_totals(first.iter());
    let second_totals = candidate_totals(second.iter());
    assert_eq!(first_totals, score_map(&[("[A]", 12), ("[B]", 8)]));
    assert_eq!(second_totals, first_totals);
    assert_eq!(only_greatest(&first_totals), Some("[A]".to_owned()));

    let eligible_contexts = BTreeMap::from([("['source-one']".to_owned(), "[north]".to_owned()), ("['source-two']".to_owned(), "[south]".to_owned())]);
    let first_eligible = eligible_totals(&first, &eligible_contexts);
    let second_eligible = eligible_totals(&second, &eligible_contexts);
    assert_eq!(first_eligible, score_map(&[("[A]", 10)]));
    assert_eq!(second_eligible, score_map(&[("[A]", 2), ("[B]", 8)]));
    assert_eq!(only_greatest(&first_eligible), Some("[A]".to_owned()));
    assert_eq!(only_greatest(&second_eligible), Some("[B]".to_owned()));
}

fn choice_fixture(experiences: &[(&str, &str, usize)], source_names: &[&str]) -> BTreeSet<ChoiceRow> {
    let mut pangine = Pangine::new();
    for (percept, concept, repetitions) in experiences {
        experience(&mut pangine, percept, concept, *repetitions);
    }
    complete_choice_rows(&mut pangine, source_names)
}

fn complete_choice_rows(pangine: &mut Pangine, source_names: &[&str]) -> BTreeSet<ChoiceRow> {
    let case = pangine.reference_percept("case");
    let context = pangine.reference_percept("context");
    let choice = pangine.reference_percept("choice");
    let question = must_ref(pangine, "(['case']->[route]->['context'])(['case']->[answer]->['choice'])");
    let sources = source_names.iter().map(|name| pangine.reference_percept(name)).collect::<Vec<_>>();
    let result = pangine.complete_question(&sources, &question).expect("valid contextual choice question");

    let rows = result
        .completions()
        .iter()
        .map(|completion| {
            assert!(completion.binding(&case).is_some());
            let context = pangine.format_concept(completion.binding(&context).expect("bound context"), false);
            let candidate = pangine.format_concept(completion.binding(&choice).expect("bound choice"), false);
            let participations = completion
                .evidence()
                .iter()
                .map(|evidence| {
                    (evidence.source_percept().expect("retained source Percept").clone(), evidence.source_concept().clone(), evidence.source_relevance())
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(participations.len(), 1, "the two clauses came from one complete remembered row");
            let (source, _, relevance) = participations.into_iter().next().unwrap();
            let source = pangine.format_concept(&source, false);
            ChoiceRow { source, context, candidate, relevance }
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(rows.len(), result.completions().len(), "fixture completions must remain distinct rows");
    rows
}

fn candidate_totals<'a>(rows: impl IntoIterator<Item = &'a ChoiceRow>) -> Scores {
    let mut totals = Scores::new();
    for row in rows {
        let previous = totals.get(&row.candidate).copied().unwrap_or(Relevance::EMPTY);
        let total = previous.checked_add(row.relevance).expect("candidate total within signed relevance range");
        totals.insert(row.candidate.clone(), total);
    }
    totals
}

fn pair_totals(rows: &BTreeSet<ChoiceRow>, key: impl Fn(&ChoiceRow) -> (String, String)) -> PairScores {
    let mut totals = PairScores::new();
    for row in rows {
        let key = key(row);
        let previous = totals.get(&key).copied().unwrap_or(Relevance::EMPTY);
        let total = previous.checked_add(row.relevance).expect("pair total within signed relevance range");
        totals.insert(key, total);
    }
    totals
}

fn eligible_totals(rows: &BTreeSet<ChoiceRow>, eligible_contexts: &BTreeMap<String, String>) -> Scores {
    candidate_totals(rows.iter().filter(|row| eligible_contexts.get(&row.source) == Some(&row.context)))
}

fn only_greatest(scores: &Scores) -> Option<String> {
    let greatest = scores.values().max()?;
    let mut candidates = scores.iter().filter_map(|(candidate, score)| (score == greatest).then_some(candidate));
    let candidate = candidates.next()?.clone();
    candidates.next().is_none().then_some(candidate)
}

fn score_map(entries: &[(&str, i64)]) -> Scores {
    entries.iter().map(|(candidate, score)| ((*candidate).to_owned(), Relevance::new(*score))).collect()
}

fn pair_score_map(entries: &[(&str, &str, i64)]) -> PairScores {
    entries.iter().map(|(left, right, score)| (((*left).to_owned(), (*right).to_owned()), Relevance::new(*score))).collect()
}

fn experience(pangine: &mut Pangine, percept: &str, concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        must_ref(pangine, &format!("['{percept}'] ~= {concept}"));
    }
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
