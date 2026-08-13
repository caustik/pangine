//! Warning checks for host-side choice calculations over complete Pangine
//! results.
//!
//! These examples use ordinary source relevance as input because that is what
//! the current engine exposes. Addition, multiplication, source grouping, and
//! ranking remain application choices rather than Pangine semantics.

use pangine::{ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

type FactorInventory = BTreeMap<String, BTreeMap<String, Relevance>>;
type Scores = BTreeMap<String, Relevance>;

#[test]
#[ignore = "warning: adding or multiplying source relevance is an application decision"]
fn additive_and_factor_product_adapters_can_choose_different_answers() {
    let inventory = factor_inventory(&["left-source", "right-source"], &["left-mark", "right-mark"], &["A", "B"], &[[9, 5], [1, 3]]);

    assert_eq!(
        inventory,
        BTreeMap::from([
            ("[A]".to_owned(), BTreeMap::from([("['left-source']".to_owned(), Relevance::new(9)), ("['right-source']".to_owned(), Relevance::DEFAULT),]),),
            ("[B]".to_owned(), BTreeMap::from([("['left-source']".to_owned(), Relevance::new(5)), ("['right-source']".to_owned(), Relevance::new(3)),]),),
        ])
    );

    let sums = factor_sums(&inventory);
    let products = factor_products(&inventory);
    assert_eq!(sums, score_map(&[("[A]", 10), ("[B]", 8)]));
    assert_eq!(products, score_map(&[("[A]", 9), ("[B]", 15)]));
    assert_eq!(only_greatest(&sums), Some("[A]".to_owned()));
    assert_eq!(only_greatest(&products), Some("[B]".to_owned()));
}

#[test]
#[ignore = "warning: whether factor rescaling means more experience or merely a unit change remains unresolved"]
fn rescaling_one_complete_factor_can_reverse_addition_without_reversing_products() {
    let small = factor_inventory(&["first", "second"], &["one", "two"], &["A", "B"], &[[1, 9], [4, 1]]);
    let scaled = factor_inventory(&["first", "second"], &["one", "two"], &["A", "B"], &[[1, 9], [40, 10]]);

    let small_sums = factor_sums(&small);
    let scaled_sums = factor_sums(&scaled);
    assert_eq!(small_sums, score_map(&[("[A]", 5), ("[B]", 10)]));
    assert_eq!(scaled_sums, score_map(&[("[A]", 41), ("[B]", 19)]));
    assert_eq!(only_greatest(&small_sums), Some("[B]".to_owned()));
    assert_eq!(only_greatest(&scaled_sums), Some("[A]".to_owned()));

    let small_products = factor_products(&small);
    let scaled_products = factor_products(&scaled);
    assert_eq!(small_products, score_map(&[("[A]", 4), ("[B]", 9)]));
    assert_eq!(scaled_products, score_map(&[("[A]", 40), ("[B]", 90)]));
    assert_eq!(only_greatest(&small_products), Some("[B]".to_owned()));
    assert_eq!(only_greatest(&scaled_products), Some("[B]".to_owned()));
}

#[test]
#[ignore = "warning: opaque renaming does not make factor-product semantics universal"]
fn adapters_receive_the_same_amounts_after_every_application_name_changes() {
    let original = factor_inventory(&["first", "second"], &["one", "two"], &["A", "B"], &[[9, 5], [1, 3]]);
    let renamed = factor_inventory(&["east-memory", "west-memory"], &["copper", "violet"], &["quartz", "cedar"], &[[9, 5], [1, 3]]);

    let original_products = factor_products(&original);
    let renamed_products = factor_products(&renamed);
    assert_eq!(sorted_values(&original_products), sorted_values(&renamed_products));
    assert_eq!(only_greatest(&original_products), Some("[B]".to_owned()));
    assert_eq!(only_greatest(&renamed_products), Some("[cedar]".to_owned()));
}

#[test]
#[ignore = "warning: abstaining on equal external scores is an application decision"]
fn an_adapter_can_leave_two_complete_tied_answers_unselected() {
    let inventory = factor_inventory(&["first", "second"], &["one", "two"], &["A", "B"], &[[2, 2], [3, 3]]);
    assert_eq!(inventory.keys().cloned().collect::<BTreeSet<_>>(), BTreeSet::from(["[A]".to_owned(), "[B]".to_owned()]));

    let sums = factor_sums(&inventory);
    let products = factor_products(&inventory);
    assert_eq!(sums, score_map(&[("[A]", 5), ("[B]", 5)]));
    assert_eq!(products, score_map(&[("[A]", 6), ("[B]", 6)]));
    assert_eq!(only_greatest(&sums), None);
    assert_eq!(only_greatest(&products), None);
}

#[test]
#[ignore = "warning: equal totals do not settle how evidence from separate sources should combine"]
fn one_repeated_source_and_two_equal_sources_remain_distinguishable() {
    let mut repeated = Pangine::new();
    experience(&mut repeated, "repeated-source", "[signal]->[A]", 2);
    let repeated_proofs = proof_sources(&mut repeated, &["repeated-source"], "[signal]->['choice']");

    let mut separate = Pangine::new();
    experience(&mut separate, "left-source", "[signal]->[A]", 1);
    experience(&mut separate, "right-source", "[signal]->[A]", 1);
    let separate_proofs = proof_sources(&mut separate, &["left-source", "right-source"], "[signal]->['choice']");

    assert_eq!(repeated_proofs, BTreeSet::from([BTreeMap::from([("['repeated-source']".to_owned(), Relevance::new(2))])]));
    assert_eq!(
        separate_proofs,
        BTreeSet::from([
            BTreeMap::from([("['left-source']".to_owned(), Relevance::DEFAULT)]),
            BTreeMap::from([("['right-source']".to_owned(), Relevance::DEFAULT)]),
        ])
    );
    assert_eq!(proof_total(&repeated_proofs), Relevance::new(2));
    assert_eq!(proof_total(&separate_proofs), Relevance::new(2));
    assert_ne!(repeated_proofs, separate_proofs);
}

#[test]
#[ignore = "warning: grouping clause evidence by source is an adapter contract, not Pangine semantics"]
fn one_declared_factor_can_supply_two_question_clauses_without_becoming_two_factors() {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "path-source", "[root]->[r]->[middle-A]->[s]->[A]", 3);
    experience(&mut pangine, "path-source", "[root]->[r]->[middle-B]->[s]->[B]", 4);
    experience(&mut pangine, "tag-source", "[tag]->[A]", 5);
    experience(&mut pangine, "tag-source", "[tag]->[B]", 3);

    let question = must_ref(&mut pangine, "(['start']->[r]->['middle'])(['middle']->[s]->['choice'])([tag]->['choice'])");
    let choice = pangine.reference_percept("choice");
    let path_source = pangine.reference_percept("path-source");
    let sources = [path_source.clone(), pangine.reference_percept("tag-source")];
    let result = pangine.complete_question(&sources, &question).expect("valid path and tag question");
    assert_eq!(result.completions().len(), 2);

    let mut fragment_products = Scores::new();
    let mut source_products = Scores::new();
    for completion in result.completions() {
        let candidate = pangine.format_concept(completion.binding(&choice).expect("bound choice"), false);
        let path_fragments = completion.evidence().iter().filter(|evidence| evidence.source_percept() == Some(&path_source)).count();
        assert_eq!(path_fragments, 2, "one retained path answers two clauses");

        let fragment_product = completion
            .evidence()
            .iter()
            .try_fold(Relevance::DEFAULT, |product, evidence| product.checked_mul(evidence.source_relevance()))
            .expect("fragment product within signed relevance range");
        fragment_products.insert(candidate.clone(), fragment_product);

        let factors = completion
            .evidence()
            .iter()
            .map(|evidence| {
                let source = pangine.format_concept(evidence.source_percept().expect("retained source Percept"), false);
                (source, evidence.source_relevance())
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(factors.len(), 2, "the adapter declared two source factors");
        let source_product = factors
            .values()
            .try_fold(Relevance::DEFAULT, |product, factor| product.checked_mul(*factor))
            .expect("source product within signed relevance range");
        source_products.insert(candidate, source_product);
    }

    assert_eq!(fragment_products, score_map(&[("[A]", 45), ("[B]", 48)]));
    assert_eq!(source_products, score_map(&[("[A]", 15), ("[B]", 12)]));
    assert_eq!(only_greatest(&fragment_products), Some("[B]".to_owned()));
    assert_eq!(only_greatest(&source_products), Some("[A]".to_owned()));
}

fn factor_inventory(factor_names: &[&str], relation_names: &[&str], candidates: &[&str; 2], repetitions: &[[usize; 2]]) -> FactorInventory {
    assert_eq!(factor_names.len(), relation_names.len());
    assert_eq!(factor_names.len(), repetitions.len());

    let mut pangine = Pangine::new();
    for ((factor, relation), amounts) in factor_names.iter().zip(relation_names).zip(repetitions) {
        for (candidate, amount) in candidates.iter().zip(amounts) {
            experience(&mut pangine, factor, &format!("[{relation}]->[{candidate}]"), *amount);
        }
    }

    let question_text = relation_names.iter().map(|relation| format!("([{relation}]->['choice'])")).collect::<String>();
    let question = must_ref(&mut pangine, &question_text);
    let choice = pangine.reference_percept("choice");
    let sources = factor_names.iter().map(|name| pangine.reference_percept(name)).collect::<Vec<_>>();
    let result = pangine.complete_question(&sources, &question).expect("valid factor question");
    assert_eq!(result.completions().len(), candidates.len());

    result
        .completions()
        .iter()
        .map(|completion| {
            let candidate = pangine.format_concept(completion.binding(&choice).expect("bound choice"), false);
            let factors = completion
                .evidence()
                .iter()
                .map(|evidence| {
                    let source = pangine.format_concept(evidence.source_percept().expect("retained source Percept"), false);
                    (source, evidence.source_relevance())
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(factors.len(), factor_names.len());
            (candidate, factors)
        })
        .collect()
}

fn proof_sources(pangine: &mut Pangine, source_names: &[&str], question: &str) -> BTreeSet<BTreeMap<String, Relevance>> {
    let question = must_ref(pangine, question);
    let choice = pangine.reference_percept("choice");
    let sources = source_names.iter().map(|name| pangine.reference_percept(name)).collect::<Vec<_>>();
    let result = pangine.complete_question(&sources, &question).expect("valid source question");

    result
        .completions()
        .iter()
        .map(|completion| {
            assert_eq!(pangine.format_concept(completion.binding(&choice).expect("bound choice"), false), "[A]");
            completion
                .evidence()
                .iter()
                .map(|evidence| {
                    let source = pangine.format_concept(evidence.source_percept().expect("retained source Percept"), false);
                    (source, evidence.source_relevance())
                })
                .collect()
        })
        .collect()
}

fn factor_sums(inventory: &FactorInventory) -> Scores {
    inventory
        .iter()
        .map(|(candidate, factors)| {
            let score = factors.values().try_fold(Relevance::EMPTY, |sum, factor| sum.checked_add(*factor)).expect("sum within signed relevance range");
            (candidate.clone(), score)
        })
        .collect()
}

fn factor_products(inventory: &FactorInventory) -> Scores {
    inventory
        .iter()
        .map(|(candidate, factors)| {
            let score =
                factors.values().try_fold(Relevance::DEFAULT, |product, factor| product.checked_mul(*factor)).expect("product within signed relevance range");
            (candidate.clone(), score)
        })
        .collect()
}

fn proof_total(proofs: &BTreeSet<BTreeMap<String, Relevance>>) -> Relevance {
    proofs
        .iter()
        .flat_map(BTreeMap::values)
        .try_fold(Relevance::EMPTY, |sum, evidence| sum.checked_add(*evidence))
        .expect("proof total within signed relevance range")
}

fn only_greatest(scores: &Scores) -> Option<String> {
    let greatest = scores.values().max()?;
    let mut winners = scores.iter().filter_map(|(candidate, score)| (score == greatest).then_some(candidate));
    let winner = winners.next()?.clone();
    winners.next().is_none().then_some(winner)
}

fn sorted_values(scores: &Scores) -> Vec<Relevance> {
    let mut values = scores.values().copied().collect::<Vec<_>>();
    values.sort();
    values
}

fn score_map(entries: &[(&str, i64)]) -> Scores {
    entries.iter().map(|(candidate, score)| ((*candidate).to_owned(), Relevance::new(*score))).collect()
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
