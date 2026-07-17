use pangine::{ConceptId, Pangine, ParseError, Relevance};

// Integration anchors:
// 2.x/pangine/include/pangine/pae_concept_parser.h:63,74,75
// 2.x/pangine/src/test/common/test_pangine.cpp:53,249,274,302
#[test]
fn implicit_union_syntax_and_star_merge_are_distinct_operations() {
    let mut pangine = Pangine::new();

    assert_eq!(
        must_ref(&mut pangine, "[A][B]"),
        must_ref(&mut pangine, "[A]*[B]")
    );
    let merged_simple_pair = must_ref(&mut pangine, "[A]*[B]");
    assert_eq!(
        pangine.format_concept(&merged_simple_pair, false),
        "([A][B])"
    );
    assert_eq!(
        must_ref(&mut pangine, "[A][B]"),
        must_ref(&mut pangine, "[B][A]")
    );
    assert_ne!(
        must_ref(&mut pangine, "[A][A]"),
        must_ref(&mut pangine, "[A]")
    );
    assert_eq!(
        must_ref(&mut pangine, "[A][A]"),
        must_ref(&mut pangine, "<x2[A]>")
    );
    assert_ne!(
        must_ref(&mut pangine, "[A][B][A][B]"),
        must_ref(&mut pangine, "[A][B]")
    );

    assert_eq!(
        must_ref(&mut pangine, "[A][A]*[A][A]"),
        must_ref(&mut pangine, "[A][A][A][A]")
    );

    let implicitly_unioned_groups = must_ref(&mut pangine, "([A][B])([A][B])");
    let merged_groups = must_ref(&mut pangine, "([A][B])*([A][B])");
    let flat_repeated = must_ref(&mut pangine, "[A][B][A][B]");

    assert_ne!(implicitly_unioned_groups, merged_groups);
    assert_eq!(merged_groups, flat_repeated);

    assert!(matches!(
        pangine.reference_concept("*[A]"),
        Err(ParseError::InvalidSyntax)
    ));
    assert!(matches!(
        pangine.reference_concept("[A]*"),
        Err(ParseError::InvalidSyntax)
    ));
}

// Integration anchors:
// 2.x/pangine/src/pangine/common/pae_concept.cpp:56,124
// 2.x/pangine/src/test/common/test_pangine.cpp:279,302
#[test]
fn parenthesized_union_operands_remain_composite_until_merge() {
    let mut pangine = Pangine::new();

    let pair = must_ref(&mut pangine, "[A][B]");
    let repeated_pair = must_ref(&mut pangine, "([A][B])([A][B])");
    let repeated_atoms = must_ref(&mut pangine, "[A][B][A][B]");

    assert_ne!(repeated_pair, pair);
    assert_ne!(repeated_pair, repeated_atoms);
    let repeated_pair_formatted = pangine.format_concept(&repeated_pair, false);
    assert_eq!(repeated_pair_formatted, "<x2([A][B])>");
    assert_eq!(
        pangine.reference_concept(&repeated_pair_formatted).unwrap(),
        Some(repeated_pair)
    );

    let distinct_groups = must_ref(&mut pangine, "([A][B])([B][C])");
    let distinct_groups_formatted = pangine.format_concept(&distinct_groups, false);
    assert_eq!(distinct_groups_formatted, "(([A][B])([B][C]))");
    assert_eq!(
        pangine
            .reference_concept(&distinct_groups_formatted)
            .unwrap(),
        Some(distinct_groups)
    );
    assert_eq!(must_ref(&mut pangine, "([A][B])*([A][B])"), repeated_atoms);
}

// Integration anchors:
// 2.x/pangine/include/pangine/pae_concept_parser.h:126,127
// 2.x/pangine/src/test/common/test_pangine.cpp:59,61,62,64,65,66,68,69,70
#[test]
fn richer_numeric_relevance_grammar_preserves_components() {
    let mut pangine = Pangine::new();

    let decimal_probability = must_ref(&mut pangine, "<50.5%[A]>");
    assert_relevance(&pangine, &decimal_probability, 0.505, 1.0);
    assert_round_trip(&mut pangine, &decimal_probability, "<50.5%[A]>");
    assert_ne!(decimal_probability, must_ref(&mut pangine, "<50%[A]>"));

    let decimal_strength = must_ref(&mut pangine, "<x2.3[A]>");
    assert_relevance(&pangine, &decimal_strength, 1.0, 2.3);
    assert_round_trip(&mut pangine, &decimal_strength, "<x2.3[A]>");

    let decimal_both = must_ref(&mut pangine, "<50.5%x2.3[A]>");
    assert_relevance(&pangine, &decimal_both, 0.505, 2.3);
    assert_round_trip(&mut pangine, &decimal_both, "<50.5%x2.3[A]>");

    let negative_both = must_ref(&mut pangine, "<-50.5%x-2.3[A]>");
    assert_relevance(&pangine, &negative_both, -0.505, -2.3);
    assert_round_trip(&mut pangine, &negative_both, "<-50.5%x-2.3[A]>");

    assert!(matches!(
        pangine.reference_concept("<%[A]>"),
        Err(ParseError::InvalidSyntax)
    ));
    assert!(matches!(
        pangine.reference_concept("<50.5[A]>"),
        Err(ParseError::InvalidSyntax)
    ));
}

// Integration anchors:
// 2.x/pangine/src/pangine/common/pae_relevance.cpp:52
// 2.x/pangine/src/pangine/common/pae_concept.cpp:56,64
// 2.x/pangine/src/test/common/test_pangine.cpp:412,413,414,415,416
#[test]
fn embedded_relevance_multiplies_when_single_child_bubbles_up() {
    let mut pangine = Pangine::new();

    let positive = must_ref(&mut pangine, "<x2<x3[A]>>");
    assert_eq!(positive, must_ref(&mut pangine, "<x6[A]>"));
    assert_round_trip(&mut pangine, &positive, "<x6[A]>");

    let negative = must_ref(&mut pangine, "<x2<x-3[A]>>");
    assert_eq!(negative, must_ref(&mut pangine, "<x-6[A]>"));
    assert_round_trip(&mut pangine, &negative, "<x-6[A]>");

    assert_eq!(
        pangine
            .reference_concept("<x2<x-3[A]>, x-2<x-3[A]>>")
            .unwrap(),
        None
    );

    let combined = must_ref(&mut pangine, "<x2<x-3[A]>, x2<x3[B]>, x2<x-3[A]>>");
    assert_eq!(combined, must_ref(&mut pangine, "<x-12[A], x6[B]>"));
    assert_round_trip(&mut pangine, &combined, "<x-12[A], x6[B]>");

    let probability_and_strength = must_ref(&mut pangine, "<50%x2<25%x3[A]>>");
    assert_relevance(&pangine, &probability_and_strength, 0.125, 6.0);
    assert_round_trip(&mut pangine, &probability_and_strength, "<12.5%x6[A]>");
}

fn must_ref(pangine: &mut Pangine, script: &str) -> ConceptId {
    pangine
        .reference_concept(script)
        .unwrap()
        .unwrap_or_else(|| panic!("failed to reference concept: {script}"))
}

fn assert_relevance(pangine: &Pangine, concept: &ConceptId, probability: f32, strength: f32) {
    let map = pangine.get_relevance_map(concept);
    assert_eq!(map.len(), 1);
    assert_near(map[0].0, Relevance::new(probability, strength));
}

fn assert_near(actual: Relevance, expected: Relevance) {
    assert!((actual.probability - expected.probability).abs() < 0.0001);
    assert!((actual.strength - expected.strength).abs() < 0.0001);
}

fn assert_round_trip(pangine: &mut Pangine, concept: &ConceptId, expected: &str) {
    let formatted = pangine.format_concept(concept, false);
    assert_eq!(formatted, expected);
    assert_eq!(
        pangine.reference_concept(&formatted).unwrap(),
        Some(concept.clone())
    );
}
