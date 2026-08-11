mod support;

use pangine::Relevance;
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
fn signed_x_coefficient_syntax_round_trips() {
    let mut test = PangineTest::new();

    test.assert_relevance(pairs! {
        "x2.3[A]" => Relevance::new(2.3),
        "x-2.3[A]" => Relevance::new(-2.3),
    });
    test.assert_formats(pairs! {
        "x2.3[A]" => "x2.3[A]",
        "x-2.3[A]" => "x-2.3[A]",
    });
    test.assert_distinct(pairs! {
        "x2.3[A]" => "x2.5[A]",
    });
    test.assert_invalid(["50.5[A]"]);
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
        "x2x3[A]" => Relevance::new(6.0),
    });
    test.assert_formats(pairs! {
        "x2x3[A]" => "x6[A]",
        "x2x-3[A]" => "x-6[A]",
        "x2x-3[A]x2x3[B]x2x-3[A]" => "x-12[A]x6[B]",
    });
}
