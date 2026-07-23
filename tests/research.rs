//! Entrypoint for API-level research tests.
//!
//! Keeping the experiment modules below `tests/research/` distinguishes them
//! from regression tests for accepted Pangine behavior while still exercising
//! them in the ordinary test suite.

use pangine::Relevance;

#[path = "research/unified_concept_state.rs"]
mod unified_concept_state;

#[test]
fn signed_relevance_is_not_an_associative_partition_coefficient() {
    let [a, b, c] = [Relevance::new(0.25, -2.0), Relevance::new(0.25, -2.0), Relevance::new(0.5, 2.0)];

    let mut left_grouped = a;
    left_grouped.add(b);
    left_grouped.add(c);

    let mut right_pair = b;
    right_pair.add(c);
    let mut right_grouped = a;
    right_grouped.add(right_pair);

    assert_eq!(left_grouped, Relevance::new(0.0, -2.0));
    assert_eq!(right_grouped, Relevance::new(0.0, -1.0));
}
