mod support;

use pangine::Relevance;
use support::{pairs, PangineTest};

#[test]
fn implicit_union_syntax_and_star_merge_are_distinct_operations() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A][B]" => "[A]*[B]",
        "[A][B]" => "[B][A]",
        "[A][A]" => "x2[A]",
        "[A][A]*[A][A]" => "[A][A][A][A]",
        "([A][B])*([A][B])" => "[A][B][A][B]",
    });
    test.assert_distinct(pairs! {
        "[A][A]" => "[A]",
        "[A][B][A][B]" => "[A][B]",
        "([A][B])([A][B])" => "([A][B])*([A][B])",
    });
    test.assert_formats(pairs! {
        "[A]*[B]" => "[A][B]",
    });
    test.assert_invalid(["*[A]", "[A]*"]);
}

#[test]
fn parenthesized_union_operands_remain_composite_until_merge() {
    let mut test = PangineTest::new();

    test.assert_distinct(pairs! {
        "([A][B])([A][B])" => "[A][B]",
        "([A][B])([A][B])" => "[A][B][A][B]",
    });
    test.assert_formats(pairs! {
        "([A][B])([A][B])" => "x2([A][B])",
        "([A][B])([B][C])" => "([A][B])([B][C])",
    });
    test.assert_equivalent(pairs! {
        "([A][B])*([A][B])" => "[A][B][A][B]",
    });
}

#[test]
fn richer_numeric_relevance_grammar_preserves_components() {
    let mut test = PangineTest::new();

    test.assert_relevance(pairs! {
        "50.5%[A]" => Relevance::new(0.505, 1.0),
        "x2.3[A]" => Relevance::new(1.0, 2.3),
        "50.5%x2.3[A]" => Relevance::new(0.505, 2.3),
        "-50.5%x-2.3[A]" => Relevance::new(-0.505, -2.3),
    });
    test.assert_formats(pairs! {
        "50.5%[A]" => "50.5%[A]",
        "x2.3[A]" => "x2.3[A]",
        "50.5%x2.3[A]" => "50.5%x2.3[A]",
        "-50.5%x-2.3[A]" => "-50.5%x-2.3[A]",
    });
    test.assert_distinct(pairs! {
        "50.5%[A]" => "50%[A]",
    });
    test.assert_invalid(["%[A]", "50.5[A]", "(50%[A], 50%[B])"]);
}

#[test]
fn relevance_prefixes_bind_one_union_operand() {
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
fn embedded_relevance_multiplies_when_single_child_bubbles_up() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "x2x3[A]" => "x6[A]",
        "x2x-3[A]" => "x-6[A]",
        "x2x-3[A]x2x3[B]x2x-3[A]" => "x-12[A]x6[B]",
    });
    test.assert_null(["x2x-3[A]x-2x-3[A]"]);
    test.assert_relevance(pairs! {
        "50%x2(25%x3[A])" => Relevance::new(0.125, 6.0),
    });
    test.assert_formats(pairs! {
        "x2x3[A]" => "x6[A]",
        "x2x-3[A]" => "x-6[A]",
        "x2x-3[A]x2x3[B]x2x-3[A]" => "x-12[A]x6[B]",
        "50%x2(25%x3[A])" => "12.5%x6[A]",
    });
}
