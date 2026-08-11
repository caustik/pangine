use super::super::{Completion, Pangine};
use crate::Relevance;
use std::collections::BTreeMap;

#[test]
#[ignore = "warning: factor multiplication is an external Bayesian oracle, not Pangine semantics"]
fn explicit_factor_conjunction_retains_enough_provenance_for_a_scale_invariant_oracle() {
    let small = declared_factor_products(1);
    let scaled = declared_factor_products(10);

    assert_eq!(small, weights(&[("disease", 4), ("healthy", 9)]));
    assert_eq!(scaled, weights(&[("disease", 40), ("healthy", 90)]));
    assert_eq!(small["[disease]"].checked_mul(scaled["[healthy]"]), small["[healthy]"].checked_mul(scaled["[disease]"]));
}

fn declared_factor_products(likelihood_scale: usize) -> BTreeMap<String, Relevance> {
    let mut pangine = Pangine::new();
    experience(&mut pangine, "prior-factor", "[prior]->[disease]", 1);
    experience(&mut pangine, "prior-factor", "[prior]->[healthy]", 9);
    experience(&mut pangine, "positive-factor", "[positive]->[disease]", 4 * likelihood_scale);
    experience(&mut pangine, "positive-factor", "[positive]->[healthy]", likelihood_scale);

    let hypothesis = must_ref(&mut pangine, "['hypothesis']");
    let question = must_ref(&mut pangine, "([prior]->['hypothesis'])([positive]->['hypothesis'])");
    let sources = [pangine.reference_percept("prior-factor"), pangine.reference_percept("positive-factor")];
    let result = pangine.complete_question(&sources, &question).expect("valid structural question");
    assert_eq!(result.completions().len(), 2);

    result
        .completions()
        .iter()
        .map(|completion| {
            let candidate = pangine.format_concept(completion.binding(&hypothesis).expect("bound hypothesis"), false);
            (candidate, factor_product(completion))
        })
        .collect()
}

fn factor_product(completion: &Completion) -> Relevance {
    completion
        .evidence()
        .iter()
        .try_fold(Relevance::DEFAULT, |product, evidence| product.checked_mul(evidence.source_relevance()))
        .expect("factor product within signed relevance range")
}

fn weights(entries: &[(&str, i64)]) -> BTreeMap<String, Relevance> {
    entries.iter().map(|(candidate, weight)| (format!("[{candidate}]"), Relevance::new(*weight))).collect()
}

fn experience(pangine: &mut Pangine, percept: &str, concept: &str, repetitions: usize) {
    for _ in 0..repetitions {
        must_ref(pangine, &format!("['{percept}'] ~= {concept}"));
    }
}

fn must_ref(pangine: &mut Pangine, input: &str) -> super::super::ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}
