mod support;

use support::{pairs, PangineTest};

// Integration anchors:
// 3.x/pangine/include/pangine/pae_concept_parser.h:69
// 3.x/pangine/src/test/common/test_pangine.cpp:564,567,568,569
#[test]
fn semicolon_separated_input_returns_the_last_statement() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A][B];[C][D]" => "[C][D]",
        "['A'] = [A][B];[C][D];$['A']" => "[A][B]",
    });
    test.assert_null(["[];", "['A'] = [];[C][D];$['A']"]);
}

// Integration anchors:
// 3.x/pangine/include/pangine/pae_concept_parser.h:41
// 3.x/pangine/src/test/common/test_pangine.cpp:575,576,577,583,584,585
#[test]
fn c_style_and_cpp_style_comments_are_ignored() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A] /* comment */ [B]" => "[A][B]",
        "[A] /* comment \n */ [B]" => "[A][B]",
        "[A] // comment [B]" => "[A]",
        "[A] // comment \n[B]" => "[A][B]",
    });
}

// Integration anchor:
// 3.x/pangine/src/test/common/test_pangine.cpp:546
#[test]
fn multiline_whitespace_remains_union_in_direct_concept_parsing() {
    let mut test = PangineTest::new();
    test.assert_equivalent(pairs! {
        "[A]\n[B]" => "[A][B]",
    });
}

// Integration anchors:
// 3.x/pangine/src/test/common/test_pangine.cpp:53,159
#[test]
fn direct_semantics_are_right_associative_and_share_legacy_correlation_identity() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A]->[B]" => "{[A]->[B]}",
        "[A]->[B]->[C]" => "[A]->([B]->[C])",
        "[A]->[B][C]->[D]" => "[A]->([B][C]->[D])",
    });
    test.assert_formats(pairs! {
        "[A]->[B]" => "{[A]->[B]}",
        "{[a]->[b][c][d]}" => "{[a]->[b][c][d]}",
        "(?[]:[A])->[target]" => "{(?[]:[A])->[target]}",
        "x2(?[]:[A])->[target]" => "{(x2?[]:[A])->[target]}",
    });

    let semantic = test.concept("[A]->[B]");
    let a = test.concept("[A]");
    let b = test.concept("[B]");
    assert_eq!(test.engine().get_correlation_a(&semantic), Some(a));
    assert_eq!(test.engine().get_correlation_b(&semantic), Some(b));
}

// Integration anchors:
// 3.x/pangine/src/test/common/test_pangine.cpp:602,604,606
#[test]
fn canonical_formatting_orders_relevance_shape_and_name_collisions() {
    let mut test = PangineTest::new();

    test.assert_formats(pairs! {
        "([a]->x2[b])*([a]->x3[c])" => "{[a]->x3[c]}{[a]->x2[b]}",
        "([a]->[b])*([a]->[b][c])" => "{[a]->[b]}{[a]->[b][c]}",
        "['X']([B]->[F])['Y'][X][Y]" => "['X']['Y'][X][Y]{[B]->[F]}",
        "(?[a]:[b])*({[a]->[b]})" => "(?[a]:[b]){[a]->[b]}",
    });
}

#[test]
fn script_text_accepts_semicolons_and_legacy_line_statements() {
    PangineTest::assert_script_results(pairs! {
        "
            ['A'] = [A];
            ['A'] *= [B];
            $['A'];
            " => "[A][B]",
        "
            ['A'] = [A]
            ['A'] *= [B]
            $['A']
            " => "[A][B]",
        "[A]\n[]" => "[A]",
        "[A];[];" => "[]",
    });
}

// Integration anchors:
// 3.x/pangine/include/pangine/pae_concept_parser.h:84,102
// 3.x/pangine/src/test/common/test_pangine.cpp:97,158,233,234
#[test]
fn binary_inverse_merge_inverts_rhs_merge_operands() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A]/[B]" => "[A]*![B]",
        "([A]/[B])" => "[A]*![B]",
        "![A]/[B]" => "![A]*![B]",
        "[C]/[B]/[A]" => "[C]*![B]*![A]",
        "([A][B])/([B][C])" => "[A]*![C]",
    });
    test.assert_invalid(["/[A]", "[A]/"]);
}

// Integration anchors:
// 3.x/pangine/include/pangine/pae_concept_parser.h:111,112,113,114,115
// 3.x/pangine/src/test/common/test_pangine.cpp:453,456,475,476,480,501
#[test]
fn explicit_percept_mutation_operators_are_required() {
    let mut test = PangineTest::new();

    test.assert_invalid(["['A'] + [A]", "['A'] ~ [A]"]);
    test.assert_equivalent(pairs! {
        "['A'] += [A]" => "[A]",
        "['A'] += [B]" => "[A][B]",
        "['A'] -= [A]" => "[B]",
    });
    test.assert_null(["['A'] -= [B]"]);

    let mut experience = PangineTest::new();
    experience.assert_equivalent(pairs! {
        "['A'] ~= [A]" => "?[]:[A]",
        "['A'] ~= [B]" => "<?[]:[A], ?[]:[B]>",
        "['A'] ~= ![A]" => "<?[]:[A], ?[]:[B], ?[]:![A]>",
        "['A'] ~= ![B]" => "<?[]:[A], ?[]:[B], ?[]:![A], ?[]:![B]>",
    });
}

// Integration anchors:
// 3.x/pangine/src/test/common/test_pangine.cpp:484,486,487,488,489,491
#[test]
fn percept_addition_preserves_groups_and_merge_flattens() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "['A'] += [A][B]" => "[A][B]",
        "['M'] *= [A][B]" => "[A][B]",
        "['A'] += [B][C]" => "([A][B])([B][C])",
        "['M'] *= [B][C]" => "[A]*[B]*[B]*[C]",
        "['A'] -= [B][C]" => "[A][B]",
        "['M'] /= [B][C]" => "[A][B]",
    });
    test.assert_distinct(pairs! {
        "([A][B])([B][C])" => "[A]*[B]*[B]*[C]",
    });
    test.assert_null(["['A'] -= [A][B]", "['M'] /= [A][B]"]);
}
