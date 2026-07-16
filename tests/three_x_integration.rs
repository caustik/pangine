use pangine::{ConceptId, Pangine, ParseError};

// Integration anchors:
// 3.x/pangine/include/pangine/pae_concept_parser.h:69
// 3.x/pangine/src/test/common/test_pangine.cpp:564,567,568,569
#[test]
fn semicolon_separated_input_returns_the_last_statement() {
    let mut pangine = Pangine::new();

    assert_eq!(pangine.reference_concept("[];").unwrap(), None);
    assert_eq!(
        must_ref(&mut pangine, "[A][B];[C][D]"),
        must_ref(&mut pangine, "[C][D]")
    );
    assert_eq!(
        must_ref(&mut pangine, "['A'] = [A][B];[C][D];$['A']"),
        must_ref(&mut pangine, "[A][B]")
    );
    assert_eq!(
        pangine
            .reference_concept("['A'] = [];[C][D];$['A']")
            .unwrap(),
        None
    );
}

// Integration anchors:
// 3.x/pangine/include/pangine/pae_concept_parser.h:41
// 3.x/pangine/src/test/common/test_pangine.cpp:575,576,577,583,584,585
#[test]
fn c_style_and_cpp_style_comments_are_ignored() {
    let mut pangine = Pangine::new();

    assert_eq!(
        must_ref(&mut pangine, "[A] /* comment */ [B]"),
        must_ref(&mut pangine, "[A][B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "[A] /* comment \n */ [B]"),
        must_ref(&mut pangine, "[A][B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "[A] // comment [B]"),
        must_ref(&mut pangine, "[A]")
    );
    assert_eq!(
        must_ref(&mut pangine, "[A] // comment \n[B]"),
        must_ref(&mut pangine, "[A][B]")
    );
}

// Integration anchor:
// 3.x/pangine/src/test/common/test_pangine.cpp:546
#[test]
fn multiline_whitespace_remains_union_in_direct_concept_parsing() {
    let mut pangine = Pangine::new();

    assert_eq!(
        must_ref(&mut pangine, "[A]\n[B]"),
        must_ref(&mut pangine, "[A][B]")
    );
}

// Integration anchors:
// 3.x/pangine/src/test/common/test_pangine.cpp:53,159
#[test]
fn direct_semantics_are_right_associative_and_share_legacy_correlation_identity() {
    let mut pangine = Pangine::new();

    assert_eq!(
        must_ref(&mut pangine, "[A]->[B]"),
        must_ref(&mut pangine, "{[A]->[B]}")
    );
    assert_eq!(
        must_ref(&mut pangine, "[A]->[B]->[C]"),
        must_ref(&mut pangine, "[A]->([B]->[C])")
    );
    assert_eq!(
        must_ref(&mut pangine, "[A]->[B][C]->[D]"),
        must_ref(&mut pangine, "[A]->([B][C]->[D])")
    );

    let semantic = must_ref(&mut pangine, "[A]->[B]");
    assert_eq!(
        pangine.get_correlation_a(&semantic),
        Some(must_ref(&mut pangine, "[A]"))
    );
    assert_eq!(
        pangine.get_correlation_b(&semantic),
        Some(must_ref(&mut pangine, "[B]"))
    );
    assert_eq!(pangine.format_concept(&semantic, false), "{[A]->[B]}");

    let grouped_semantic = must_ref(&mut pangine, "{[a]->[b][c][d]}");
    let grouped_semantic_formatted = pangine.format_concept(&grouped_semantic, false);
    assert_eq!(grouped_semantic_formatted, "{[a]->[b][c][d]}");
    assert_eq!(
        pangine
            .reference_concept(&grouped_semantic_formatted)
            .unwrap(),
        Some(grouped_semantic)
    );
}

// Integration anchors:
// 3.x/pangine/src/test/common/test_pangine.cpp:602,604,606
#[test]
fn canonical_formatting_orders_relevance_shape_and_name_collisions() {
    let mut pangine = Pangine::new();

    let relevance_order = must_ref(&mut pangine, "([a]-><x2[b]>)*([a]-><x3[c]>)");
    let relevance_formatted = pangine.format_concept(&relevance_order, false);
    assert_eq!(relevance_formatted, "({[a]-><x3[c]>}{[a]-><x2[b]>})");
    assert_eq!(
        pangine.reference_concept(&relevance_formatted).unwrap(),
        Some(relevance_order)
    );

    let fewer_subconcepts = must_ref(&mut pangine, "([a]->[b])*([a]->[b][c])");
    let fewer_formatted = pangine.format_concept(&fewer_subconcepts, false);
    assert_eq!(fewer_formatted, "({[a]->[b]}{[a]->[b][c]})");
    assert_eq!(
        pangine.reference_concept(&fewer_formatted).unwrap(),
        Some(fewer_subconcepts)
    );

    let name_collision = must_ref(&mut pangine, "['X']([B]->[F])['Y'][X][Y]");
    let name_collision_formatted = pangine.format_concept(&name_collision, false);
    assert_eq!(name_collision_formatted, "(['X']['Y'][X][Y]{[B]->[F]})");
    assert_eq!(
        pangine
            .reference_concept(&name_collision_formatted)
            .unwrap(),
        Some(name_collision)
    );
}

#[test]
fn script_text_accepts_semicolons_and_legacy_line_statements() {
    let mut semicolon_script = Pangine::new();
    let semicolon_result = semicolon_script
        .parse_script_text(
            "
            ['A'] = [A];
            ['A'] *= [B];
            $['A'];
        ",
        )
        .unwrap();
    let semicolon_expected = must_ref(&mut semicolon_script, "[A][B]");
    assert_eq!(semicolon_result, Some(semicolon_expected));

    let mut legacy_script = Pangine::new();
    let legacy_result = legacy_script
        .parse_script_text(
            "
            ['A'] = [A]
            ['A'] *= [B]
            $['A']
        ",
        )
        .unwrap();
    let legacy_expected = must_ref(&mut legacy_script, "[A][B]");
    assert_eq!(legacy_result, Some(legacy_expected));

    let mut legacy_null_script = Pangine::new();
    let legacy_null_result = legacy_null_script.parse_script_text("[A]\n[]").unwrap();
    let legacy_null_expected = must_ref(&mut legacy_null_script, "[A]");
    assert_eq!(legacy_null_result, Some(legacy_null_expected));

    let mut semicolon_null_script = Pangine::new();
    assert_eq!(
        semicolon_null_script.parse_script_text("[A];[];").unwrap(),
        None
    );
}

// Integration anchors:
// 3.x/pangine/include/pangine/pae_concept_parser.h:84,102
// 3.x/pangine/src/test/common/test_pangine.cpp:97,158,233,234
#[test]
fn binary_inverse_merge_inverts_rhs_merge_operands() {
    let mut pangine = Pangine::new();

    assert_eq!(
        must_ref(&mut pangine, "[A]/[B]"),
        must_ref(&mut pangine, "[A]*![B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "([A]/[B])"),
        must_ref(&mut pangine, "[A]*![B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "![A]/[B]"),
        must_ref(&mut pangine, "![A]*![B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "[C]/[B]/[A]"),
        must_ref(&mut pangine, "[C]*![B]*![A]")
    );
    assert_eq!(
        must_ref(&mut pangine, "([A][B])/([B][C])"),
        must_ref(&mut pangine, "[A]*![C]")
    );

    assert!(matches!(
        pangine.reference_concept("/[A]"),
        Err(ParseError::InvalidSyntax)
    ));
    assert!(matches!(
        pangine.reference_concept("[A]/"),
        Err(ParseError::InvalidSyntax)
    ));
}

// Integration anchors:
// 3.x/pangine/include/pangine/pae_concept_parser.h:111,112,113,114,115
// 3.x/pangine/src/test/common/test_pangine.cpp:453,456,475,476,480,501
#[test]
fn explicit_percept_mutation_operators_are_required() {
    let mut pangine = Pangine::new();

    assert!(matches!(
        pangine.reference_concept("['A'] + [A]"),
        Err(ParseError::InvalidSyntax)
    ));
    assert!(matches!(
        pangine.reference_concept("['A'] ~ [A]"),
        Err(ParseError::InvalidSyntax)
    ));

    assert_eq!(
        must_ref(&mut pangine, "['A'] += [A]"),
        must_ref(&mut pangine, "[A]")
    );
    assert_eq!(
        must_ref(&mut pangine, "['A'] += [B]"),
        must_ref(&mut pangine, "[A][B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "['A'] -= [A]"),
        must_ref(&mut pangine, "[B]")
    );
    assert_eq!(pangine.reference_concept("['A'] -= [B]").unwrap(), None);

    let mut experience = Pangine::new();
    assert_eq!(
        must_ref(&mut experience, "['A'] ~= [A]"),
        must_ref(&mut experience, "[A]")
    );
    assert_eq!(
        must_ref(&mut experience, "['A'] ~= [B]"),
        must_ref(&mut experience, "[A][B]")
    );
    assert_eq!(
        must_ref(&mut experience, "['A'] ~= ![A]"),
        must_ref(&mut experience, "[B]")
    );
    assert_eq!(experience.reference_concept("['A'] ~= ![B]").unwrap(), None);
}

// Integration anchors:
// 3.x/pangine/src/test/common/test_pangine.cpp:484,486,487,488,489,491
#[test]
fn percept_addition_preserves_groups_and_merge_flattens() {
    let mut pangine = Pangine::new();

    assert_eq!(
        must_ref(&mut pangine, "['A'] += [A][B]"),
        must_ref(&mut pangine, "[A][B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "['M'] *= [A][B]"),
        must_ref(&mut pangine, "[A][B]")
    );

    let grouped = must_ref(&mut pangine, "['A'] += [B][C]");
    let flattened = must_ref(&mut pangine, "['M'] *= [B][C]");
    assert_eq!(grouped, must_ref(&mut pangine, "([A][B])([B][C])"));
    assert_eq!(flattened, must_ref(&mut pangine, "[A]*[B]*[B]*[C]"));
    assert_ne!(grouped, flattened);

    assert_eq!(
        must_ref(&mut pangine, "['A'] -= [B][C]"),
        must_ref(&mut pangine, "[A][B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "['M'] /= [B][C]"),
        must_ref(&mut pangine, "[A][B]")
    );
    assert_eq!(pangine.reference_concept("['A'] -= [A][B]").unwrap(), None);
    assert_eq!(pangine.reference_concept("['M'] /= [A][B]").unwrap(), None);
}

fn must_ref(pangine: &mut Pangine, script: &str) -> ConceptId {
    pangine
        .reference_concept(script)
        .unwrap()
        .unwrap_or_else(|| panic!("failed to reference concept: {script}"))
}
