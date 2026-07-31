//! Research-only characterization of floating-point Relevance identity.
//!
//! These tests preserve counterexamples rather than accepted semantics. They
//! ask whether the current coefficient representation can safely participate
//! in canonical Concept identity and partition-independent combination.

use pangine::{Pangine, Relevance};

fn add(left: Relevance, right: Relevance) -> Relevance {
    let mut result = left;
    result.add(right);
    result
}

#[test]
fn positive_decimal_relevance_and_concepts_can_differ_by_grouping() {
    let a = Relevance::new(0.01, 1.0);
    let b = Relevance::new(0.01, 1.0);
    let c = Relevance::new(0.04, 1.0);
    let left_grouped = add(add(a, b), c);
    let right_grouped = add(a, add(b, c));

    assert_eq!(left_grouped.strength, right_grouped.strength);
    assert_ne!(left_grouped.probability.to_bits(), right_grouped.probability.to_bits());

    let mut pangine = Pangine::new();
    let left = pangine.parse_script_text("['left'] = 1%[A]; ['left'] += 1%[A]; ['left'] += 4%[A]; $['left']").unwrap().unwrap();
    let right = pangine.parse_script_text("['pair'] = 1%[A]; ['pair'] += 4%[A]; ['right'] = 1%[A]; ['right'] += $['pair']; $['right']").unwrap().unwrap();

    assert_ne!(left, right);
    assert_eq!(pangine.format_concept(&left, false), "2%x3[A]");
    assert_eq!(pangine.format_concept(&right, false), "1.9999998%x3[A]");
}

#[test]
fn oversized_decimal_syntax_constructs_nonfinite_noncanonical_relevance() {
    let mut pangine = Pangine::new();
    let source = format!("{}%[A]", "9".repeat(100));
    let concept = pangine.reference_concept(&source).unwrap().expect("oversized coefficient currently constructs a Concept");
    let entries = pangine.get_relevance_map(&concept);
    let [(relevance, _)] = entries.as_slice() else {
        panic!("expected one Relevance entry");
    };

    assert!(!relevance.probability.is_finite());
    let canonical = pangine.format_concept(&concept, false);
    assert_eq!(canonical, "inf%[A]");
    assert!(pangine.reference_concept(&canonical).is_err());
}
