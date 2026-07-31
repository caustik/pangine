//! Research-only factorization of a question from its exact source records.
//!
//! The oracle asks whether the current matcher needs to serialize a second,
//! nested match record for every answer. It keeps the question once and unions
//! the exact stored Observations that contribute to any output. Reapplying the
//! question after that union must reproduce the result from combined state.
//!
//! Source filtering here is specific to the current structural evaluator. A
//! future background-sensitive evaluator may require records that do not bind
//! the current question directly, so this is not accepted question semantics.

use super::super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct FactoredQuestionResult {
    question: ConceptId,
    sources: Option<ConceptId>,
}

impl FactoredQuestionResult {
    fn merged(pangine: &mut Pangine, partials: &[Self]) -> Result<Self, &'static str> {
        let question = partials.first().ok_or("cannot merge no question results")?.question.clone();
        let mut records = ConceptMap::new();
        for partial in partials {
            if partial.question != question {
                return Err("cannot merge results from unequal questions");
            }
            let Some(sources) = &partial.sources else {
                continue;
            };
            for (record, relevance) in sources.observation_records().ok_or("factored sources are not Observations")? {
                if relevance != Relevance::DEFAULT {
                    return Err("factored source has structural relevance");
                }
                records.entry(record).or_insert(Relevance::DEFAULT);
            }
        }
        let sources = pangine.reference_observation_set(&records);
        Ok(Self { question, sources })
    }

    fn answers(&self, pangine: &mut Pangine) -> BTreeMap<ConceptId, Option<ConceptId>> {
        let experiences = self.sources.clone().map(|sources| pangine.question_experience_map(sources, &self.question)).unwrap_or_default();
        pangine.get_projection_results(&self.question, &experiences)
    }
}

fn must_reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a Concept from {source:?}"))
}

fn experience_state(pangine: &mut Pangine, memory: &str, experiences: &[&str]) -> ConceptId {
    for experience in experiences {
        must_reference(pangine, &format!("['{memory}'] ~= {experience}"));
    }
    let memory = must_reference(pangine, &format!("['{memory}']"));
    pangine.get_value(&memory).unwrap()
}

fn answers_from_state(pangine: &mut Pangine, state: &ConceptId, question: &ConceptId) -> BTreeMap<ConceptId, Option<ConceptId>> {
    let experiences = pangine.question_experience_map(state.clone(), question);
    pangine.get_projection_results(question, &experiences)
}

fn factor_question_sources(pangine: &mut Pangine, state: &ConceptId, question: &ConceptId) -> Result<FactoredQuestionResult, &'static str> {
    let include_records = matches!(question.0.kind, ConceptKind::Observation { .. });
    let records = state.observation_records().ok_or("question source is not Observation state")?;
    let mut contributing = ConceptMap::new();
    for (record, relevance) in records {
        if relevance != Relevance::DEFAULT {
            return Err("question source has structural relevance");
        }
        let experiences = pangine.question_experiences_from_records([(record.clone(), relevance)], include_records);
        let answers = pangine.get_projection_results(question, &experiences);
        if answers.values().any(Option::is_some) {
            contributing.insert(record, Relevance::DEFAULT);
        }
    }
    let sources = pangine.reference_observation_set(&contributing);
    Ok(FactoredQuestionResult { question: question.clone(), sources })
}

fn assert_factorization(experiences: &[&str], question: &str) {
    let mut pangine = Pangine::new();
    let state = experience_state(&mut pangine, "memory", experiences);
    let question = must_reference(&mut pangine, question);
    let expected = answers_from_state(&mut pangine, &state, &question);
    let factored = factor_question_sources(&mut pangine, &state, &question).unwrap();
    assert_eq!(factored.answers(&mut pangine), expected);
}

#[test]
fn current_matcher_reconstructs_answers_from_the_question_and_contributing_sources() {
    for (experiences, question) in [
        (vec!["?[event-1]:{[weather]->[rain]}", "?[noise]:[unrelated]"], "{[weather]->['answer']}"),
        (
            vec!["?[event-1]:{{{[choice]->[morning]}->[workday]}->[tea]}", "?[event-2]:{{[choice]->[evening]}->[coffee]}"],
            "{{{[choice]->[morning]}->[workday]}->['answer']}",
        ),
        (vec!["?[event-1]:[A]*[B]", "?[noise]:[C]"], "['answer']*[B]"),
        (vec!["?[event-1]:[A][B][C]", "?[noise]:[D]"], "[A][B]['answer']"),
        (vec!["?[event-1]:{[A]->[A]}", "?[noise]:{[A]->[B]}"], "{['same']->['same']}"),
        (vec!["?[event-1]:{[A]->[B]}", "?[noise]:[C]"], "{['left']->['right']}"),
        (vec!["?[camera]:{[weather]->[rain]}", "?[thermometer]:{[temperature]->[warm]}"], "?['observer']:{[weather]->['condition']}"),
    ] {
        assert_factorization(&experiences, question);
    }
}

#[test]
fn factored_source_results_distinguish_replay_from_independent_observations() {
    let mut pangine = Pangine::new();
    let question = must_reference(&mut pangine, "{[weather]->['answer']}");
    let partition_a = experience_state(&mut pangine, "partition-a", &["?[event-1]:{[weather]->[rain]}"]);
    let partition_b = experience_state(&mut pangine, "partition-b", &["?[event-1]:{[weather]->[rain]}"]);
    let partition_c = experience_state(&mut pangine, "partition-c", &["?[event-2]:{[weather]->[rain]}"]);
    let replayed_state = experience_state(&mut pangine, "replayed", &["?[event-1]:{[weather]->[rain]}", "?[event-1]:{[weather]->[rain]}"]);
    let independent_state = experience_state(&mut pangine, "independent", &["?[event-1]:{[weather]->[rain]}", "?[event-2]:{[weather]->[rain]}"]);

    let a = factor_question_sources(&mut pangine, &partition_a, &question).unwrap();
    let b = factor_question_sources(&mut pangine, &partition_b, &question).unwrap();
    let c = factor_question_sources(&mut pangine, &partition_c, &question).unwrap();
    let replayed = FactoredQuestionResult::merged(&mut pangine, &[a.clone(), b]).unwrap();
    let independent = FactoredQuestionResult::merged(&mut pangine, &[a, c]).unwrap();
    let expected_replayed = factor_question_sources(&mut pangine, &replayed_state, &question).unwrap();
    let expected_independent = factor_question_sources(&mut pangine, &independent_state, &question).unwrap();

    assert_eq!(replayed, expected_replayed);
    assert_eq!(independent, expected_independent);
    assert_ne!(replayed.sources, independent.sources);
    assert_eq!(replayed.sources.as_ref().map(|sources| pangine.format_concept(sources, false)), Some("?[event-1]:{[weather]->[rain]}".to_owned()));
    assert_eq!(
        independent.sources.as_ref().map(|sources| pangine.format_concept(sources, false)),
        Some("<?[event-1]:{[weather]->[rain]}, ?[event-2]:{[weather]->[rain]}>".to_owned())
    );
    assert_eq!(replayed.answers(&mut pangine), answers_from_state(&mut pangine, &replayed_state, &question));
    assert_eq!(independent.answers(&mut pangine), answers_from_state(&mut pangine, &independent_state, &question));
}

#[test]
fn merging_factored_sources_is_associative_order_independent_and_idempotent() {
    let mut pangine = Pangine::new();
    let question = must_reference(&mut pangine, "{[weather]->['answer']}");
    let states = [
        experience_state(&mut pangine, "a", &["?[event-1]:{[weather]->[rain]}"]),
        experience_state(&mut pangine, "b", &["?[event-2]:{[weather]->[sun]}"]),
        experience_state(&mut pangine, "c", &["?[event-3]:{[weather]->[rain]}"]),
    ];
    let partials = states.iter().map(|state| factor_question_sources(&mut pangine, state, &question).unwrap()).collect::<Vec<_>>();

    let combined = FactoredQuestionResult::merged(&mut pangine, &partials).unwrap();
    let reversed = FactoredQuestionResult::merged(&mut pangine, &[partials[2].clone(), partials[1].clone(), partials[0].clone()]).unwrap();
    let first_pair = FactoredQuestionResult::merged(&mut pangine, &partials[..2]).unwrap();
    let left = FactoredQuestionResult::merged(&mut pangine, &[first_pair, partials[2].clone()]).unwrap();
    let replayed = FactoredQuestionResult::merged(&mut pangine, &[combined.clone(), partials[0].clone()]).unwrap();

    assert_eq!(reversed, combined);
    assert_eq!(left, combined);
    assert_eq!(replayed, combined);
}

#[test]
fn current_matcher_filter_keeps_broad_and_sibling_matches_but_drops_unrelated_records() {
    let mut pangine = Pangine::new();
    let matching = [
        "?[workday-tea-1]:{{{[choice]->[morning]}->[workday]}->[tea]}",
        "?[morning-coffee]:{{[choice]->[morning]}->[coffee]}",
        "?[root-coffee]:{[choice]->[coffee]}",
        "?[evening-coffee-1]:{{[choice]->[evening]}->[coffee]}",
    ];
    let unrelated = "?[noise]:[unrelated]";
    let state = experience_state(&mut pangine, "memory", &[matching[0], matching[1], matching[2], matching[3], unrelated]);
    let question = must_reference(&mut pangine, "{{{[choice]->[morning]}->[workday]}->['answer']}");
    let factored = factor_question_sources(&mut pangine, &state, &question).unwrap();
    let sources = factored.sources.unwrap().observation_records().unwrap();

    for source in matching {
        assert!(sources.contains_key(&must_reference(&mut pangine, source)));
    }
    assert!(!sources.contains_key(&must_reference(&mut pangine, unrelated)));
}
