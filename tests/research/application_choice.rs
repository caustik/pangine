//! Warning checks for application-supplied choice policies.
//!
//! One application requires support from every selected source. Another
//! supplies candidate costs and a budget. Pangine supplies complete answers and
//! source identity, but it does not infer agreement, trust, cost, authority, or
//! correctness from either rule.

use pangine::{CompletionResult, ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

type SourceInventory = BTreeMap<String, BTreeMap<String, Relevance>>;
type Scores = BTreeMap<String, Relevance>;

#[test]
#[ignore = "warning: requiring every declared source is an application policy"]
fn required_source_agreement_can_override_a_larger_additive_total() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "left-source", "[signal]->[A]", 1);
    experience(&mut pangine, "left-source", "[signal]->[B]", 10);
    experience(&mut pangine, "right-source", "[signal]->[A]", 1);

    let (result, choice, required_sources) = complete(&mut pangine, &["left-source", "right-source"], "[signal]->['choice']");
    assert!(result.completions().iter().all(|completion| completion.evidence().len() == 1));
    let inventory = source_inventory(&pangine, &result, &choice);

    assert_eq!(inventory, expected_inventory(&[("[A]", &[("['left-source']", 1), ("['right-source']", 1)]), ("[B]", &[("['left-source']", 10)]),]));
    let totals = additive_totals(&inventory);
    assert_eq!(totals, score_map(&[("[A]", 2), ("[B]", 10)]));
    assert_eq!(only_greatest(&totals), Some("[B]".to_owned()));
    assert_eq!(only_candidate_with_every_source(&inventory, &required_sources), Some("[A]".to_owned()));
}

#[test]
#[ignore = "warning: carrying required-source agreement across question shapes is an application policy"]
fn the_same_required_source_rule_works_for_a_two_step_path_question() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "left-source", "[root]->[r]->[middle-A]->[s]->[A]", 1);
    experience(&mut pangine, "left-source", "[root]->[r]->[middle-B]->[s]->[B]", 10);
    experience(&mut pangine, "right-source", "[root]->[r]->[middle-C]->[s]->[A]", 1);

    let question = "(['start']->[r]->['middle'])(['middle']->[s]->['choice'])";
    let (result, choice, required_sources) = complete(&mut pangine, &["left-source", "right-source"], question);
    assert!(result.completions().iter().all(|completion| completion.evidence().len() == 2));
    let inventory = source_inventory(&pangine, &result, &choice);

    assert_eq!(inventory, expected_inventory(&[("[A]", &[("['left-source']", 1), ("['right-source']", 1)]), ("[B]", &[("['left-source']", 10)]),]));
    let totals = additive_totals(&inventory);
    assert_eq!(totals, score_map(&[("[A]", 2), ("[B]", 10)]));
    assert_eq!(only_greatest(&totals), Some("[B]".to_owned()));
    assert_eq!(only_candidate_with_every_source(&inventory, &required_sources), Some("[A]".to_owned()));
}

#[test]
#[ignore = "warning: source disagreement is an application-supplied reason to abstain"]
fn required_source_disagreement_can_abstain_despite_a_large_numeric_lead() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "left-source", "[signal]->[A]", 10);
    experience(&mut pangine, "right-source", "[signal]->[B]", 1);

    let (result, choice, required_sources) = complete(&mut pangine, &["left-source", "right-source"], "[signal]->['choice']");
    let inventory = source_inventory(&pangine, &result, &choice);
    assert_eq!(inventory, expected_inventory(&[("[A]", &[("['left-source']", 10)]), ("[B]", &[("['right-source']", 1)])]));

    let totals = additive_totals(&inventory);
    assert_eq!(totals, score_map(&[("[A]", 10), ("[B]", 1)]));
    assert_eq!(only_greatest(&totals), Some("[A]".to_owned()));
    assert_eq!(only_candidate_with_every_source(&inventory, &required_sources), None);
}

#[test]
#[ignore = "warning: filtering candidates by cost and budget is an application policy"]
fn an_external_cost_limit_can_override_a_larger_additive_total() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "[signal]->[A]", 10);
    experience(&mut pangine, "memory", "[signal]->[B]", 4);

    let (result, choice, _) = complete(&mut pangine, &["memory"], "[signal]->['choice']");
    assert!(result.completions().iter().all(|completion| completion.evidence().len() == 1));
    let inventory = source_inventory(&pangine, &result, &choice);
    assert_eq!(inventory, expected_inventory(&[("[A]", &[("['memory']", 10)]), ("[B]", &[("['memory']", 4)])]));

    let totals = additive_totals(&inventory);
    let costs = cost_map(&[("[A]", 5), ("[B]", 2)]);
    assert_eq!(only_greatest(&totals), Some("[A]".to_owned()));
    assert_eq!(only_affordable_greatest(&totals, &costs, 2), Some("[B]".to_owned()));
}

#[test]
#[ignore = "warning: carrying candidate costs across question shapes is an application policy"]
fn the_same_external_cost_limit_works_for_a_two_step_path_question() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "[root]->[r]->[middle-A]->[s]->[A]", 10);
    experience(&mut pangine, "memory", "[root]->[r]->[middle-B]->[s]->[B]", 4);

    let question = "(['start']->[r]->['middle'])(['middle']->[s]->['choice'])";
    let (result, choice, _) = complete(&mut pangine, &["memory"], question);
    assert!(result.completions().iter().all(|completion| completion.evidence().len() == 2));
    let inventory = source_inventory(&pangine, &result, &choice);
    assert_eq!(inventory, expected_inventory(&[("[A]", &[("['memory']", 10)]), ("[B]", &[("['memory']", 4)])]));

    let totals = additive_totals(&inventory);
    let costs = cost_map(&[("[A]", 5), ("[B]", 2)]);
    assert_eq!(only_greatest(&totals), Some("[A]".to_owned()));
    assert_eq!(only_affordable_greatest(&totals, &costs, 2), Some("[B]".to_owned()));
}

#[test]
#[ignore = "warning: refusing every over-budget candidate is an application policy"]
fn an_external_cost_limit_can_abstain_despite_a_unique_numeric_winner() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "memory", "[signal]->[A]", 10);
    experience(&mut pangine, "memory", "[signal]->[B]", 4);

    let (result, choice, _) = complete(&mut pangine, &["memory"], "[signal]->['choice']");
    let inventory = source_inventory(&pangine, &result, &choice);
    let totals = additive_totals(&inventory);
    let costs = cost_map(&[("[A]", 5), ("[B]", 4)]);

    assert_eq!(only_greatest(&totals), Some("[A]".to_owned()));
    assert_eq!(only_affordable_greatest(&totals, &costs, 3), None);
}

fn complete(pangine: &mut Pangine, source_names: &[&str], question: &str) -> (CompletionResult, ConceptId, BTreeSet<String>) {
    let question = must_ref(pangine, question);
    let choice = pangine.reference_percept("choice");
    let sources = source_names.iter().map(|name| pangine.reference_percept(name)).collect::<Vec<_>>();
    let required_sources = sources.iter().map(|source| pangine.format_concept(source, false)).collect();
    let result = pangine.complete_question(&sources, &question).expect("valid application question");
    (result, choice, required_sources)
}

fn source_inventory(pangine: &Pangine, result: &CompletionResult, choice: &ConceptId) -> SourceInventory {
    let mut inventory = SourceInventory::new();
    for completion in result.completions() {
        let candidate = pangine.format_concept(completion.binding(choice).expect("bound choice"), false);
        let mut completion_sources = BTreeMap::new();
        for evidence in completion.evidence() {
            let source = pangine.format_concept(evidence.source_percept().expect("retained source Percept"), false);
            if let Some(previous) = completion_sources.insert(source, evidence.source_relevance()) {
                assert_eq!(previous, evidence.source_relevance(), "one source must expose one relevance within a completion");
            }
        }

        let candidate_sources = inventory.entry(candidate).or_default();
        for (source, relevance) in completion_sources {
            assert!(candidate_sources.insert(source, relevance).is_none(), "fixture supplied the same candidate from one source more than once");
        }
    }
    inventory
}

fn additive_totals(inventory: &SourceInventory) -> Scores {
    inventory
        .iter()
        .map(|(candidate, sources)| {
            let total = sources.values().try_fold(Relevance::EMPTY, |sum, relevance| sum.checked_add(*relevance)).expect("total within signed relevance range");
            (candidate.clone(), total)
        })
        .collect()
}

fn only_candidate_with_every_source(inventory: &SourceInventory, required_sources: &BTreeSet<String>) -> Option<String> {
    let mut candidates =
        inventory.iter().filter_map(|(candidate, sources)| required_sources.iter().all(|source| sources.contains_key(source)).then_some(candidate));
    let candidate = candidates.next()?.clone();
    candidates.next().is_none().then_some(candidate)
}

fn only_greatest(scores: &Scores) -> Option<String> {
    let greatest = scores.values().max()?;
    let mut candidates = scores.iter().filter_map(|(candidate, score)| (score == greatest).then_some(candidate));
    let candidate = candidates.next()?.clone();
    candidates.next().is_none().then_some(candidate)
}

fn only_affordable_greatest(scores: &Scores, costs: &BTreeMap<String, i64>, budget: i64) -> Option<String> {
    let affordable = scores
        .iter()
        .filter(|(candidate, _)| *costs.get(*candidate).unwrap_or_else(|| panic!("missing cost for {candidate}")) <= budget)
        .collect::<BTreeMap<_, _>>();
    let greatest = affordable.values().copied().max()?;
    let mut candidates = affordable.into_iter().filter_map(|(candidate, score)| (score == greatest).then_some(candidate));
    let candidate = candidates.next()?.clone();
    candidates.next().is_none().then_some(candidate)
}

fn expected_inventory(entries: &[(&str, &[(&str, i64)])]) -> SourceInventory {
    entries
        .iter()
        .map(|(candidate, sources)| {
            let sources = sources.iter().map(|(source, relevance)| ((*source).to_owned(), Relevance::new(*relevance))).collect();
            ((*candidate).to_owned(), sources)
        })
        .collect()
}

fn score_map(entries: &[(&str, i64)]) -> Scores {
    entries.iter().map(|(candidate, score)| ((*candidate).to_owned(), Relevance::new(*score))).collect()
}

fn cost_map(entries: &[(&str, i64)]) -> BTreeMap<String, i64> {
    entries.iter().map(|(candidate, cost)| ((*candidate).to_owned(), *cost)).collect()
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
