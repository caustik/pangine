mod support;

use pangine::Relevance;
use support::{pairs, PangineTest};

// Integration anchors:
// 2.x/pangine/include/pangine/pae_concept_parser.h:63,74,75
// 2.x/pangine/src/test/common/test_pangine.cpp:53,249,274,302
#[test]
fn implicit_union_syntax_and_star_merge_are_distinct_operations() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A][B]" => "[A]*[B]",
        "[A][B]" => "[B][A]",
        "[A][A]" => "<x2[A]>",
        "[A][A]*[A][A]" => "[A][A][A][A]",
        "([A][B])*([A][B])" => "[A][B][A][B]",
    });
    test.assert_distinct(pairs! {
        "[A][A]" => "[A]",
        "[A][B][A][B]" => "[A][B]",
        "([A][B])([A][B])" => "([A][B])*([A][B])",
    });
    test.assert_formats(pairs! {
        "[A]*[B]" => "([A][B])",
    });
    test.assert_invalid(["*[A]", "[A]*"]);
}

// Integration anchors:
// 2.x/pangine/src/pangine/common/pae_concept.cpp:56,124
// 2.x/pangine/src/test/common/test_pangine.cpp:279,302
#[test]
fn parenthesized_union_operands_remain_composite_until_merge() {
    let mut test = PangineTest::new();

    test.assert_distinct(pairs! {
        "([A][B])([A][B])" => "[A][B]",
        "([A][B])([A][B])" => "[A][B][A][B]",
    });
    test.assert_formats(pairs! {
        "([A][B])([A][B])" => "<x2([A][B])>",
        "([A][B])([B][C])" => "(([A][B])([B][C]))",
    });
    test.assert_equivalent(pairs! {
        "([A][B])*([A][B])" => "[A][B][A][B]",
    });
}

// Integration anchors:
// 2.x/pangine/include/pangine/pae_concept_parser.h:126,127
// 2.x/pangine/src/test/common/test_pangine.cpp:59,61,62,64,65,66,68,69,70
#[test]
fn richer_numeric_relevance_grammar_preserves_components() {
    let mut test = PangineTest::new();

    test.assert_relevance(pairs! {
        "<50.5%[A]>" => Relevance::new(0.505, 1.0),
        "<x2.3[A]>" => Relevance::new(1.0, 2.3),
        "<50.5%x2.3[A]>" => Relevance::new(0.505, 2.3),
        "<-50.5%x-2.3[A]>" => Relevance::new(-0.505, -2.3),
    });
    test.assert_formats(pairs! {
        "<50.5%[A]>" => "<50.5%[A]>",
        "<x2.3[A]>" => "<x2.3[A]>",
        "<50.5%x2.3[A]>" => "<50.5%x2.3[A]>",
        "<-50.5%x-2.3[A]>" => "<-50.5%x-2.3[A]>",
    });
    test.assert_distinct(pairs! {
        "<50.5%[A]>" => "<50%[A]>",
    });
    test.assert_invalid(["<%[A]>", "<50.5[A]>"]);
}

// Integration anchors:
// 2.x/pangine/src/pangine/common/pae_relevance.cpp:52
// 2.x/pangine/src/pangine/common/pae_concept.cpp:56,64
// 2.x/pangine/src/test/common/test_pangine.cpp:412,413,414,415,416
#[test]
fn embedded_relevance_multiplies_when_single_child_bubbles_up() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "<x2<x3[A]>>" => "<x6[A]>",
        "<x2<x-3[A]>>" => "<x-6[A]>",
        "<x2<x-3[A]>, x2<x3[B]>, x2<x-3[A]>>" => "<x-12[A], x6[B]>",
    });
    test.assert_null(["<x2<x-3[A]>, x-2<x-3[A]>>"]);
    test.assert_relevance(pairs! {
        "<50%x2<25%x3[A]>>" => Relevance::new(0.125, 6.0),
    });
    test.assert_formats(pairs! {
        "<x2<x3[A]>>" => "<x6[A]>",
        "<x2<x-3[A]>>" => "<x-6[A]>",
        "<x2<x-3[A]>, x2<x3[B]>, x2<x-3[A]>>" => "<x-12[A], x6[B]>",
        "<50%x2<25%x3[A]>>" => "<12.5%x6[A]>",
    });
}
