//! API-level characterization oracles for the unified Concept-state research.
//!
//! Some checks intentionally preserve known limitations so competing designs
//! can be compared against the same bounded examples. They are research
//! evidence, not approval of the observed behavior as Pangine's final contract.

use pangine::{ConceptId, Pangine};

fn must_reference(pangine: &mut Pangine, script: &str) -> ConceptId {
    pangine.reference_concept(script).unwrap().unwrap_or_else(|| panic!("expected a concept from {script:?}"))
}

fn candidate_weight(pangine: &Pangine, candidates: &ConceptId, name: &str) -> f32 {
    pangine
        .get_relevance_map(candidates)
        .into_iter()
        .find_map(|(relevance, candidate)| (pangine.get_name(&candidate) == Some(name)).then_some(relevance.weight()))
        .unwrap_or_else(|| panic!("missing candidate {name:?}"))
}

// These are read-only design oracles over existing behavior. Union
// addition is used as a proxy for retaining direct experience roots; it is not
// a proposed replacement spelling for experience.
#[test]
fn a_top_level_concept_union_can_preserve_a_composite_root_boundary() {
    let mut pangine = Pangine::new();

    must_reference(&mut pangine, "['one-root'] += [A]*[B]");
    must_reference(&mut pangine, "['one-root'] @ ['one-output']*[B]");
    let one_output = must_reference(&mut pangine, "$['one-output']");
    assert_eq!(candidate_weight(&pangine, &one_output, "A"), 2.0);

    must_reference(&mut pangine, "['two-roots'] += [A]");
    must_reference(&mut pangine, "['two-roots'] += [B]");
    must_reference(&mut pangine, "['two-roots'] @ ['two-output']*[B]");
    assert!(pangine.reference_concept("$['two-output']").unwrap().is_none());
}

#[test]
fn singleton_relevance_still_collapses_structural_multiplicity_and_repetition() {
    let mut pangine = Pangine::new();

    must_reference(&mut pangine, "['one-root'] += x2[A]");
    must_reference(&mut pangine, "['two-roots'] += [A]");
    must_reference(&mut pangine, "['two-roots'] += [A]");

    let one_percept = pangine.reference_percept("one-root");
    let two_percept = pangine.reference_percept("two-roots");
    let one_root = pangine.get_value(&one_percept).unwrap();
    let two_roots = pangine.get_value(&two_percept).unwrap();
    assert_eq!(one_root, two_roots);
}

#[test]
fn direct_root_storage_preserves_a_finite_full_vs_partial_crossover() {
    let mut pangine = Pangine::new();

    must_reference(&mut pangine, "['memory'] += {[C]->[A]}*{[B]->[D]}");
    for partial in ["{[E]->[A]}*{[P1]->[Q1]}", "{[E]->[A]}*{[P2]->[Q2]}", "{[E]->[A]}*{[P3]->[Q3]}"] {
        must_reference(&mut pangine, &format!("['memory'] += {partial}"));
    }

    must_reference(&mut pangine, "['memory'] @ {['output']->[A]}*{[B]->[D]}");
    let candidates = must_reference(&mut pangine, "$['output']");
    assert_eq!(candidate_weight(&pangine, &candidates, "C"), 10.0);
    assert_eq!(candidate_weight(&pangine, &candidates, "E"), 12.0);
}

#[test]
fn direct_root_storage_alone_does_not_prevent_generic_swamping() {
    let mut pangine = Pangine::new();

    for index in 0..8 {
        must_reference(&mut pangine, &format!("['memory'] += [P{index}]*[B]"));
    }

    must_reference(&mut pangine, "['memory'] @ ['output']*[B]");
    let candidates = must_reference(&mut pangine, "$['output']");
    assert_eq!(candidate_weight(&pangine, &candidates, "B"), 8.0);
    assert_eq!(candidate_weight(&pangine, &candidates, "P0"), 2.0);
}
