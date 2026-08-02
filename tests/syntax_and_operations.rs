mod support;

use pangine::{Pangine, ParseError};
use support::{pairs, PangineTest};

#[test]
fn semicolon_separated_input_returns_the_last_statement() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A][B];[C][D]" => "[C][D]",
        "['A'] = [A][B];[C][D];$['A']" => "[A][B]",
    });
    test.assert_null(["[];", "['A'] = [];[C][D];$['A']"]);
}

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

#[test]
fn hash_comments_are_limited_to_legacy_script_input() {
    let mut test = PangineTest::new();
    test.assert_invalid(["# comment", "[A] # comment"]);

    PangineTest::assert_script_results(pairs! {
        "# historical comment\n[A]" => "[A]",
        "[A] # historical comment\n[B]" => "[B]",
        "[A]; # historical comment\n[B];" => "[B]",
    });
}

#[test]
fn multiline_whitespace_remains_union_in_direct_concept_parsing() {
    let mut test = PangineTest::new();
    test.assert_equivalent(pairs! {
        "[A]\n[B]" => "[A][B]",
    });
}

#[test]
fn ordered_compositions_are_flat_canonical_and_explicitly_nestable() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "[A]->[B]" => "{[A]->[B]}",
        "[A]->[B]->[C]" => "{[A]->[B]->[C]}",
        "[A]->[B][C]->[D]" => "{[A]->[B][C]->[D]}",
    });
    test.assert_distinct(pairs! {
        "[A]->[B]->[C]" => "([A]->[B])->[C]",
        "[A]->[B]->[C]" => "[A]->([B]->[C])",
        "[A]->[B]->[C]" => "[C]->[B]->[A]",
        "[A]->[B]->[A]" => "[A]->[B]",
    });
    test.assert_formats(pairs! {
        "[A]->[B]" => "{[A]->[B]}",
        "[A]->[B]->[C]" => "{[A]->[B]->[C]}",
        "{[a]->[b][c][d]}" => "{[a]->[b][c][d]}",
        "([A][B])->[target]" => "{[A][B]->[target]}",
        "x2([A][B])->[target]" => "{x2([A][B])->[target]}",
        "([A]->[B])->[C]" => "{{[A]->[B]}->[C]}",
        "[A]->([B]->[C])" => "{[A]->{[B]->[C]}}",
    });

    let semantic = test.concept("[A]->[B]");
    let a = test.concept("[A]");
    let b = test.concept("[B]");
    assert_eq!(test.engine().get_ordered_components(&semantic), Some(vec![a, b]));
}

#[test]
fn canonical_formatting_orders_relevance_shape_and_name_collisions() {
    let mut test = PangineTest::new();

    test.assert_formats(pairs! {
        "([a]->x2[b])*([a]->x3[c])" => "{[a]->x3[c]}{[a]->x2[b]}",
        "([a]->[b])*([a]->[b][c])" => "{[a]->[b]}{[a]->[b][c]}",
        "['X']([B]->[F])['Y'][X][Y]" => "['X']['Y'][X][Y]{[B]->[F]}",
        "([a][b])*({[a]->[b]})" => "[a][b]{[a]->[b]}",
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

#[test]
fn greedy_choice_selects_the_greatest_positive_relevance_weight() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "['choice'] = x2[tea]x3[coffee]; ^['choice']" => "[coffee]",
        "['single'] = [tea]; ^['single']" => "[tea]",
        "['mixed'] = x2[tea]![coffee]; ^['mixed']" => "[tea]",
        "['zero-negative'] = 0%[tea]![coffee]; $['zero-negative']" => "![coffee]",
    });
    test.assert_null([
        "['zero'] = 0%[tea]0%[coffee]; ^['zero']",
        "['zero-negative'] = 0%[tea]![coffee]; ^['zero-negative']",
        "['negative'] = ![tea]![coffee]; ^['negative']",
    ]);
}

#[test]
fn greedy_choice_breaks_exact_ties_by_canonical_concept_spelling() {
    PangineTest::assert_script_results(pairs! {
        "['keep-a']=[A];['keep-b']=[B];['choice']=$['keep-a']*$['keep-b'];^['choice']" => "[A]",
        "['keep-b']=[B];['keep-a']=[A];['choice']=$['keep-a']*$['keep-b'];^['choice']" => "[A]",
        "['a']=[A];['b']=[B];['c']=[C];['left']=$['a']*$['b'];['right']=$['c'];['choice']=$['left']*$['right'];^['choice']" => "[A]",
        "['c']=[C];['b']=[B];['a']=[A];['right']=$['c'];['left']=$['a']*$['b'];['choice']=$['right']*$['left'];^['choice']" => "[A]",
    });
}

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

#[test]
fn ordinary_percept_mutation_operators_are_explicit() {
    let mut test = PangineTest::new();

    test.assert_invalid(["['A'] + [A]", "['A'] ~ [A]"]);
    test.assert_equivalent(pairs! {
        "['A'] += [A]" => "[A]",
        "['A'] += [B]" => "[A][B]",
        "['A'] -= [A]" => "[B]",
    });
    test.assert_null(["['A'] -= [B]"]);
}

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

#[test]
fn null_concepts_and_invalid_syntax_have_distinct_results() {
    let mut pangine = Pangine::new();

    assert_eq!(pangine.reference_concept("[]").unwrap(), None);
    assert_eq!(pangine.parse_script_text("[A];[];").unwrap(), None);

    assert!(matches!(pangine.reference_concept("[A"), Err(ParseError::InvalidSyntax)));
    assert!(matches!(pangine.parse_script_text("[A];[B"), Err(ParseError::InvalidSyntax)));
    for script in ["!", "[A]*", "[A]/", "[A]->"] {
        assert!(matches!(pangine.reference_concept(script), Err(ParseError::InvalidSyntax)), "expected invalid syntax: {script}");
    }
}

#[test]
fn parse_details_distinguish_invalid_syntax_from_a_null_result() {
    let mut pangine = Pangine::new();
    let mut null_details = Vec::new();
    let mut error_details = Vec::new();

    assert_eq!(pangine.parse_script_text_with_details("[]", &mut null_details).unwrap(), None);
    assert!(matches!(pangine.parse_script_text_with_details("[A", &mut error_details), Err(ParseError::InvalidSyntax)));

    assert_eq!(String::from_utf8(null_details).unwrap(), "ps> []\nps=   []\n");
    assert_eq!(String::from_utf8(error_details).unwrap(), "ps> [A\nps!   invalid Pangine syntax\n");
}
