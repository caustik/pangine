mod support;

use pangine::{ParseError, Relevance};
use support::{pairs, PangineTest};

#[test]
fn adjacency_composes_complete_operands_while_star_merges_their_members() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A][B]" => "[A]*[B]",
        "[A][B]" => "[B][A]",
        "[A][A]" => "x2[A]",
        "[A][A]*[A][A]" => "[A][A][A][A]",
        "([A][B])*([A][B])" => "[A][B][A][B]",
        "(([A][B])([C][D]))*([E][F])" => "([A][B])([C][D])[E][F]",
    });
    test.assert_distinct(pairs! {
        "[A][A]" => "[A]",
        "[A][B][A][B]" => "[A][B]",
        "([A][B])([A][B])" => "([A][B])*([A][B])",
        "(([A][B])([C][D]))*([E][F])" => "[A][B][C][D][E][F]",
    });
    test.assert_formats(pairs! {
        "[A]*[B]" => "[A][B]",
        "([A][B])*([C][D])" => "[A][B][C][D]",
    });
    test.assert_invalid(["*[A]", "[A]*", "<[A][B]>", "[A]<[B]>"]);
}

#[test]
fn parenthesized_unions_remain_complete_canonical_operands() {
    let mut test = PangineTest::new();

    test.assert_distinct(pairs! {
        "([A][B])([A][B])" => "[A][B][A][B]",
        "([A][B])([B][C])" => "[A][B][B][C]",
        "([A][B])([C][D])" => "[A][B][C][D]",
        "x2([A][B])" => "x2[A]x2[B]",
        "(x2[A]x2[B])(x3[A]x3[B])" => "x5[A]x5[B]",
    });
    test.assert_equivalent(pairs! {
        "([A][B])([A][B])" => "x2([A][B])",
        "x-1([A][B])" => "!([A][B])",
    });
    test.assert_null(["([A][B])x-1([A][B])"]);
    test.assert_formats(pairs! {
        "([A][B])([C][D])" => "([A][B])([C][D])",
        "([A][B])([A][B])" => "x2([A][B])",
        "x-1([A][B])" => "!([A][B])",
    });

    let repeated = test.concept("x2([A][B])");
    assert_eq!(test.engine().debug_console_lines(Some(&repeated)), vec!["  x2([A][B])"]);
    let inverted = test.concept("!([A][B])");
    assert_eq!(test.engine().debug_console_lines(Some(&inverted)), vec!["  !([A][B])"]);
}

#[test]
fn signed_integer_coefficient_syntax_round_trips_exactly() {
    let mut test = PangineTest::new();

    test.assert_relevance(pairs! {
        "x9223372036854775807[A]" => Relevance::new(i64::MAX),
        "x-9223372036854775808[A]" => Relevance::new(i64::MIN),
    });
    test.assert_formats(pairs! {
        "x9223372036854775807[A]" => "x9223372036854775807[A]",
        "x-9223372036854775808[A]" => "x-9223372036854775808[A]",
    });
    test.assert_distinct(pairs! {
        "x9223372036854775807[A]" => "x9223372036854775806[A]",
    });
    test.assert_invalid(["x[A]", "x2.3[A]", "x-2.3[A]", "50.5[A]"]);

    for source in ["x9223372036854775808[A]", "x9223372036854775807[A][A]", "x9223372036854775807x2[A]", "!x-9223372036854775808[A]"] {
        assert!(matches!(test.engine_mut().reference_concept(source), Err(ParseError::RelevanceOverflow)), "expected relevance overflow for {source}");
    }
}

#[test]
fn coefficient_prefixes_bind_one_complete_operand() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "x2[A]x3[B]" => "[A][A][B][B][B]",
        "(x2[A]x3[B])" => "x2[A]x3[B]",
        "x2{[A]->[B]}" => "{[A]->[B]}{[A]->[B]}",
    });
    test.assert_distinct(pairs! {
        "x2[A][B]" => "x2([A][B])",
    });
    test.assert_formats(pairs! {
        "x2[A]x3[B]" => "x3[B]x2[A]",
        "x2([A][B])" => "x2([A][B])",
    });
}

#[test]
fn nested_coefficients_multiply_during_union_normalization() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "x2x3[A]" => "x6[A]",
        "x2x-3[A]" => "x-6[A]",
        "x2x-3[A]x2x3[B]x2x-3[A]" => "x-12[A]x6[B]",
    });
    test.assert_null(["x2x-3[A]x-2x-3[A]"]);
    test.assert_relevance(pairs! {
        "x2x3[A]" => Relevance::new(6),
    });
    test.assert_formats(pairs! {
        "x2x3[A]" => "x6[A]",
        "x2x-3[A]" => "x-6[A]",
        "x2x-3[A]x2x3[B]x2x-3[A]" => "x-12[A]x6[B]",
    });
}
