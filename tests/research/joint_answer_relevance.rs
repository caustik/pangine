//! Warning checks for the source witnesses retained by a shared answer.
//!
//! The answer state now keeps complete correlated possibilities until `^`
//! filters them. Adding distinct source relevance remains a provisional way to
//! turn those witnesses into the integer coefficients exposed by `$` and `@`.

use pangine::{ConceptId, Pangine};

#[test]
#[ignore = "warning: additive source relevance is not the final Relevance model"]
fn one_source_witness_is_not_counted_again_when_projection_hides_distinct_rows() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['memory'] ~= ([one]->[animal]->[cat])([one]->[food]->[fish])([two]->[animal]->[cat])([two]->[food]->[milk])");
    must_ref(&mut pangine, "['memory'] @ (['case']->[animal]->['animal'])(['case']->[food]->['food'])");

    assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "[cat]"));
    assert_eq!(must_ref(&mut pangine, "$(['animal']->['food'])"), must_ref(&mut pangine, "{[cat]->[fish]}{[cat]->[milk]}"));
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
