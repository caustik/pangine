use pangine::{ConceptId, Pangine, ParseError, Relevance};

// Parity anchors:
// 1.x/pangine/include/pangine/pae_relevance.h:30,52
// 1.x/pangine/src/test/common/test_relevance.cpp:22
#[test]
fn relevance_add_and_subtract_match_1x() {
    let mut relevance = Relevance::new(1.0, 1.0);

    relevance.add(Relevance::new(1.0, -1.0));
    relevance.add(Relevance::new(1.0, 1.0));
    assert_eq!(relevance, Relevance::new(1.0, 1.0));

    relevance.sub(Relevance::new(1.0, -1.0));
    relevance.sub(Relevance::new(1.0, 1.0));
    assert_eq!(relevance, Relevance::new(1.0, 1.0));
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:137,179
// 1.x/pangine/src/test/common/test_reference_concept.cpp:28,89,116,137,164,191
#[test]
fn references_namespaces_params_and_parentheses() {
    let mut pangine = Pangine::new();
    let concept_a = must_ref(&mut pangine, "[A]");
    let concept_b = must_ref(&mut pangine, "[B]");
    let longer_name = must_ref(&mut pangine, "[LONGER_NAME]");
    let longer_dash = must_ref(&mut pangine, "[LONGER-NAME]");
    let even_longer = must_ref(&mut pangine, "[EVEN LONGER NAME]");

    assert_ne!(concept_a, concept_b);
    assert_ne!(concept_a, longer_name);
    assert_ne!(longer_name, longer_dash);
    assert_ne!(longer_dash, even_longer);
    assert_eq!(pangine.get_name(&concept_a), Some("A"));

    let test1 = must_ref(&mut pangine, "[test1]");
    let test2 = must_ref(&mut pangine, "[test2]");
    let with_params = pangine
        .reference_concept_with_params("{[test3]->[%]}*[%]", &[test1, test2])
        .unwrap()
        .unwrap();
    let without_params = must_ref(&mut pangine, "{[test3]->[test1]}*[test2]");
    assert_eq!(with_params, without_params);

    assert_eq!(must_ref(&mut pangine, "     [A]"), concept_a);
    assert_eq!(must_ref(&mut pangine, " \t\r\n[B]"), concept_b);

    let percept_a = must_ref(&mut pangine, "['A']");
    let percept_b = must_ref(&mut pangine, "['B']");
    let question = must_ref(&mut pangine, "[?]");
    let question_again = must_ref(&mut pangine, "[?]");
    let question_long = must_ref(&mut pangine, "[?LONGER_NAME]");
    let question_dash = must_ref(&mut pangine, "[?LONGER-NAME]");

    assert_ne!(percept_a, percept_b);
    assert_eq!(question, question_again);
    assert_ne!(question, question_long);
    assert_ne!(question_long, question_dash);
    assert!(matches!(
        pangine.reference_concept("[??BAD_NAME]"),
        Err(ParseError::InvalidSyntax)
    ));
    assert_ne!(concept_a, percept_a);
    assert_ne!(concept_a, must_ref(&mut pangine, "[?A]"));

    assert_eq!(must_ref(&mut pangine, "([A])"), concept_a);
    assert_eq!(must_ref(&mut pangine, "(([A]))"), concept_a);
    assert_eq!(must_ref(&mut pangine, "(['A'])"), percept_a);
    assert_eq!(must_ref(&mut pangine, "((['A']))"), percept_a);
}

// Parity anchors:
// 1.x/pangine/include/pangine/pae_pangine.h:55,67,73,80,83
// 1.x/pangine/include/pangine/pae_concept.h:69,74,79
// 1.x/pangine/include/pangine/pae_percept.h:30,35,38
#[test]
fn public_api_surface_matches_1x_boundaries() {
    let mut pangine = Pangine::new();

    let percept = pangine.reference_percept("direct");
    assert_eq!(
        pangine.reference_concept("['direct']").unwrap(),
        Some(percept.clone())
    );
    assert_eq!(pangine.get_percept(&percept), Some(percept.clone()));
    assert_eq!(pangine.get_value(&percept), None);

    let a = must_ref(&mut pangine, "[A]");
    let b = must_ref(&mut pangine, "[B]");
    assert!(pangine.set_percept_value(&percept, Some(a.clone())));
    assert_eq!(pangine.get_value(&percept), Some(a.clone()));
    assert_eq!(pangine.recurse(&percept, false), "['direct']");
    assert_eq!(pangine.recurse(&percept, true), "[A]");

    let merged = pangine.perform_merge(&percept, Some(&b));
    assert_eq!(merged, Some(must_ref(&mut pangine, "[A]*[B]")));
    assert_eq!(pangine.get_value(&percept), Some(a));
    assert!(pangine.set_percept_value(&percept, merged.clone()));
    assert_eq!(pangine.get_value(&percept), merged);

    let memory = pangine.reference_percept("memory");
    let experience = must_ref(&mut pangine, "{[A]->[B]}");
    let experienced = pangine.perform_experience(&memory, Some(&experience));
    assert_eq!(
        experienced,
        Some(must_ref(&mut pangine, "<100%{[A]->[B]}, 100%[A], 100%[B]>"))
    );
    assert_eq!(pangine.get_value(&memory), None);

    let left = pangine.reference_percept("left");
    let right = pangine.reference_percept("right");
    let correlation = must_ref(&mut pangine, "{['left']->['right']}");
    assert_eq!(pangine.get_percept_a(&correlation), Some(left));
    assert_eq!(pangine.get_percept_b(&correlation), Some(right));
}

// Parity anchors:
// 1.x/pangine/src/test/common/test_reference_concept.cpp:218,244,272,313
#[test]
fn correlations_dependencies_and_relevance_are_canonical() {
    let mut pangine = Pangine::new();

    let a = must_ref(&mut pangine, "[A]");
    let b_percept = must_ref(&mut pangine, "['B']");
    let correlation = must_ref(&mut pangine, "{[A]->['B']}");
    assert_eq!(pangine.get_correlation_a(&correlation), Some(a.clone()));
    assert_eq!(pangine.get_correlation_b(&correlation), Some(b_percept));

    let nested = must_ref(&mut pangine, "{{[A]->['B']}->{[?]->[D]}}");
    let question_to_d = must_ref(&mut pangine, "{[?]->[D]}");
    assert_eq!(
        pangine.get_correlation_a(&nested),
        Some(correlation.clone())
    );
    assert_eq!(pangine.get_correlation_b(&nested), Some(question_to_d));
    assert!(pangine
        .reference_concept("{{[A]->[B]}->[C]}")
        .unwrap()
        .is_some());
    assert!(pangine
        .reference_concept("{[C]->{[A]->[B]}}")
        .unwrap()
        .is_some());

    let dependency = must_ref(&mut pangine, "?[A]:[B]");
    let b = must_ref(&mut pangine, "[B]");
    assert_eq!(pangine.get_dependency_a(&dependency), Some(a));
    assert_eq!(pangine.get_dependency_b(&dependency), Some(b));
    let nested_dependency = must_ref(&mut pangine, "?(?[A]:[B]):(?[C]:[D])");
    assert_eq!(
        pangine.get_dependency_a(&nested_dependency),
        Some(dependency)
    );

    let relevance1 = must_ref(&mut pangine, "<50%[A], 50%[B]>");
    let relevance2 = must_ref(&mut pangine, "<25%[A], 25%[B]>");
    let relevance3 = must_ref(&mut pangine, "<25%[A], 50%[B], 50%[B]>");
    let relevance4 = must_ref(&mut pangine, "<25%[A], 25%[B], 75%[B]>");
    let relevance5 = must_ref(
        &mut pangine,
        "<25%[A]*[B], 25%[B]*[A], 10%{[C]->[D]}, 10%{[C]->[D]}>",
    );
    let relevance6 = must_ref(
        &mut pangine,
        "<15%[B]*[A], 35%[B]*[A], 5%{[C]->[D]}, 15%{[C]->[D]}>",
    );

    assert_ne!(relevance1, relevance2);
    assert_ne!(relevance2, relevance3);
    assert_eq!(relevance3, relevance4);
    assert_eq!(relevance5, relevance6);
    assert!(pangine
        .reference_concept("{<25%[A], 25%[B]>->[C]}")
        .unwrap()
        .is_some());
}

// Parity anchors:
// 1.x/pangine/src/test/common/test_reference_concept.cpp:338,390,421,591,613,630
#[test]
fn union_inversion_normalization_and_null_removal_match_1x() {
    let mut pangine = Pangine::new();

    let a = must_ref(&mut pangine, "[A]");
    let union_ab = must_ref(&mut pangine, "[A]*[B]");
    let b = must_ref(&mut pangine, "[B]");
    assert_ne!(a, union_ab);
    assert_ne!(a, b);
    assert_ne!(union_ab, b);
    assert_eq!(union_ab, must_ref(&mut pangine, "[B]*[A]"));
    assert_eq!(
        must_ref(&mut pangine, "([B]*[A])*([C]*[D])"),
        must_ref(&mut pangine, "([D]*[C])*([A]*[B])")
    );
    assert_eq!(
        must_ref(&mut pangine, "([D]*[C])*(([A]*[B])*[E]*[F]*([A]*[B]))"),
        must_ref(
            &mut pangine,
            "<100%x2[A], 100%x2[B], 100%[C], 100%[D], 100%[E], 100%[F]>"
        )
    );

    let inverted_a = must_ref(&mut pangine, "![A]");
    assert_ne!(a, inverted_a);
    assert_eq!(a, must_ref(&mut pangine, "!![A]"));
    assert_eq!(a, must_ref(&mut pangine, "!(!([A]))"));
    assert_eq!(
        must_ref(&mut pangine, "!([A]*[B])"),
        must_ref(&mut pangine, "!(!!([B]*[A]))")
    );
    assert_ne!(must_ref(&mut pangine, "!([A]*[B])"), union_ab);

    assert_eq!(must_ref(&mut pangine, "<x-1[A]>"), inverted_a);
    assert_eq!(
        must_ref(&mut pangine, "<x-1[A], x-1[B]>"),
        must_ref(&mut pangine, "!([A]*[B])")
    );
    assert_eq!(
        must_ref(&mut pangine, "<x-2[A], x-2[B]>*([A]*[B])"),
        must_ref(&mut pangine, "!([A]*[B])")
    );
    assert_eq!(
        pangine
            .reference_concept("<x-2([A]*[B])>*<x2([A]*[B])>")
            .unwrap(),
        None
    );

    assert_eq!(
        must_ref(&mut pangine, "!([A])*!([B])"),
        must_ref(&mut pangine, "![A]*(![B])")
    );
    assert_eq!(
        must_ref(&mut pangine, "![A]*(![B])"),
        must_ref(&mut pangine, "![A]*![B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "![A]*![B]"),
        must_ref(&mut pangine, "(![A])*(![B])")
    );
    assert_eq!(
        must_ref(&mut pangine, "?![A]*[B]:[C]"),
        must_ref(&mut pangine, "?(![A]*[B]):[C]")
    );

    assert_eq!(
        must_ref(&mut pangine, "!([A]*[B])*!([C]*[D])"),
        must_ref(&mut pangine, "(![A]*![B]*![C]*![D])")
    );
    assert_eq!(
        pangine.reference_concept("!([A]*[B])*([A]*[B])").unwrap(),
        None
    );
    assert_eq!(
        must_ref(&mut pangine, "([A]*[B]*[C])*!([A]*[B])"),
        must_ref(&mut pangine, "[C]")
    );

    assert_eq!(pangine.reference_concept("[A]*![A]").unwrap(), None);
    assert_eq!(
        pangine.reference_concept("([A]*[B])*!([A]*[B])").unwrap(),
        None
    );
    assert!(matches!(
        pangine.reference_concept("([A]*![B})*(![A]*[B])"),
        Err(ParseError::InvalidSyntax)
    ));
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:755,810,852,1226
// 1.x/pangine/src/test/common/test_reference_concept.cpp:421,448,554,630,674
#[test]
fn percept_merge_experience_recursion_questions_and_decisions_match_1x() {
    let mut pangine = Pangine::new();

    assert_eq!(
        must_ref(&mut pangine, "['test'] *= [A]"),
        must_ref(&mut pangine, "[A]")
    );
    assert_eq!(
        must_ref(&mut pangine, "['test'] *= [B]"),
        must_ref(&mut pangine, "[A]*[B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "['test'] *= ![A]"),
        must_ref(&mut pangine, "[B]")
    );
    assert_eq!(
        must_ref(&mut pangine, "$['test']"),
        must_ref(&mut pangine, "[B]")
    );

    let experience1 = must_ref(&mut pangine, "['mind01'] ~= {[tigger]->[meows]}");
    let experience2 = must_ref(&mut pangine, "['mind01'] ~= {[tigger]->[purrs]}");
    let expected2 = must_ref(
        &mut pangine,
        "<100%{[tigger]->[meows]}, 100%{[tigger]->[purrs]}, 100%x2.0[tigger], 100%[meows], 100%[purrs]>",
    );
    let experience3 = must_ref(&mut pangine, "['mind01'] ~= {[tigger]->[purrs]}");
    let expected3a = must_ref(
        &mut pangine,
        "<100%{[tigger]->[meows]}, 100%x2{[tigger]->[purrs]}, 100%[tigger], 100%x2[tigger], 100%[meows], 100%x2[purrs]>",
    );
    let expected3b = must_ref(
        &mut pangine,
        "<100%{[tigger]->[meows]}, 100%x2{[tigger]->[purrs]}, 100%x3.0[tigger], 100%[meows], 100%x2[purrs]>",
    );

    assert_ne!(experience1, experience2);
    assert_eq!(experience2, expected2);
    assert_ne!(experience3, experience2);
    assert_eq!(experience3, expected3a);
    assert_eq!(experience3, expected3b);
    assert_ne!(
        expected3b,
        must_ref(
            &mut pangine,
            "<100%{[tigger]->[meows]}, 100%x2.001{[tigger]->[purrs]}>"
        )
    );
    let single_b = must_ref(&mut pangine, "['A'] ~= [B]");
    let double_b = must_ref(&mut pangine, "['A'] ~= [B]");
    assert_ne!(single_b, double_b);
    assert_eq!(double_b, must_ref(&mut pangine, "[B]*[B]"));

    let recursive = must_ref(&mut pangine, "['dog_eats_dog'] = {{[dog]->[eats]}->[dog]}");
    must_ref(&mut pangine, "['wisdom'] ~= ['dog_eats_dog']");
    must_ref(
        &mut pangine,
        "['obvious'] ~= {{['dog_eats_dog']->[is]}->['obvious']}",
    );
    let recursive_a = pangine.get_correlation_a(&recursive).unwrap();
    assert_eq!(
        pangine.get_correlation_a(&recursive_a),
        pangine.get_correlation_b(&recursive)
    );

    let okay = must_ref(&mut pangine, "['okay'] ~= {[A]->[B]}");
    let okay_twice = must_ref(&mut pangine, "['okay'] ~= {[A]->[B]}");
    let still_okay = must_ref(&mut pangine, "['okay'] ~= !{[A]->[B]}");
    assert_ne!(okay, okay_twice);
    assert_eq!(still_okay, okay);

    assert!(pangine
        .reference_concept("['mind'] ~= {{[A]->[species_is]}->[cat]}")
        .unwrap()
        .is_some());
    assert!(pangine
        .reference_concept("['mind'] ~= {{[B]->[species_is]}->[dog]}")
        .unwrap()
        .is_some());
    assert!(pangine
        .reference_concept("['mind'] ~= {{[A]->[sound_is]}->[meow]}")
        .unwrap()
        .is_some());
    assert!(pangine
        .reference_concept("['mind'] ~= {{[B]->[sound_is]}->[bark]}")
        .unwrap()
        .is_some());
    assert!(pangine
        .reference_concept("['mind'] ~= {{[C]->[species_is]}->[cat]}")
        .unwrap()
        .is_some());
    assert!(pangine
        .reference_concept("['mind'] ~= {{[C]->[sound_is]}->['output']}")
        .unwrap()
        .is_some());
    assert!(pangine.reference_concept("[meow]").unwrap().is_some());
}

#[test]
fn correlation_questions_search_all_question_correlations() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['test'] ~= {[A]->[B]}");

    assert_eq!(
        must_ref(&mut pangine, "['test'] @ {['1']->[C]}*{['2']->[B]}"),
        must_ref(&mut pangine, "[A]")
    );
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:109
// 1.x/pangine/src/test/common/test_parse_text.cpp:20
#[test]
fn parse_text_matches_1x_word_union_behavior() {
    let mut pangine = Pangine::new();
    let text1 = pangine.parse_text("simple parse text test").unwrap();
    let text2 = pangine
        .parse_text("simple text test %%%%% $$parse")
        .unwrap();
    let expected = must_ref(&mut pangine, "[simple]*[parse]*[text]*[test]");

    assert_eq!(text1, text2);
    assert_eq!(text2, expected);
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_concept.cpp:21,113
// 1.x/pangine/src/test/common/test_pangine.cpp:41,110
#[test]
fn formatting_round_trips_and_relevance_map_orders_like_1x() {
    let mut pangine = Pangine::new();
    let concept = must_ref(&mut pangine, "['test'] ~= {[test]->[okay]}");
    let printed = pangine.format_concept(&concept, false);
    let reparsed = must_ref(&mut pangine, &printed);
    assert_eq!(concept, reparsed);

    let relevance = must_ref(&mut pangine, "[A]*[A]*[C]*[C]*[B]*[C]");
    let strengths: Vec<_> = pangine
        .get_relevance_map(&relevance)
        .into_iter()
        .map(|(relevance, _)| relevance.strength)
        .collect();
    assert_eq!(strengths, vec![3.0, 2.0, 1.0]);

    let equal_relevance = must_ref(&mut pangine, "[A]*[C]*[B]");
    let strengths: Vec<_> = pangine
        .get_relevance_map(&equal_relevance)
        .into_iter()
        .map(|(relevance, _)| relevance.strength)
        .collect();
    assert_eq!(strengths, vec![1.0, 1.0, 1.0]);

    let single = must_ref(&mut pangine, "[A]");
    let single_relevance = pangine.get_relevance_map(&single);
    assert_eq!(single_relevance.len(), 1);
    assert_eq!(single_relevance[0].0.strength, 1.0);
}

// Parity anchor:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:1311
#[test]
fn debug_console_rows_match_1x_display_rules() {
    let mut pangine = Pangine::new();

    let a = must_ref(&mut pangine, "[A]");
    assert_eq!(pangine.debug_console_lines(Some(&a), false), vec!["  [A]"]);
    assert_eq!(pangine.debug_console_lines(None, false), vec!["  []"]);

    let relevance = must_ref(&mut pangine, "<50%[A], x2[B], x-1[C]>");
    assert_eq!(
        pangine.debug_console_lines(Some(&relevance), false),
        vec!["  50% [A]", "  x2 [B]", "  ![C]"]
    );

    let combined_relevance = must_ref(&mut pangine, "<50%x2[a]>");
    assert_eq!(
        pangine.debug_console_lines(Some(&combined_relevance), false),
        vec!["  50%x2 [a]"]
    );

    must_ref(&mut pangine, "['test'] = [A]");
    let percept = must_ref(&mut pangine, "['test']");
    assert_eq!(
        pangine.debug_console_lines(Some(&percept), true),
        vec!["  [A]"]
    );
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:39
// 1.x/pangine/src/test/common/test_parse_script.cpp:20,34,48,62,76,90,104,118
#[test]
fn historical_1x_scripts_return_success() {
    let scripts = [
        include_str!("fixtures/1x/test_pangine.pae"),
        include_str!("fixtures/1x/test_merge.pae"),
        include_str!("fixtures/1x/test_syllogism.pae"),
        include_str!("fixtures/1x/test_counting.pae"),
        include_str!("fixtures/1x/test_experience.pae"),
        include_str!("fixtures/1x/test_question.pae"),
        include_str!("fixtures/1x/test_decision.pae"),
        include_str!("fixtures/1x/test_rule110.pae"),
    ];

    for script in scripts {
        let mut pangine = Pangine::new();
        let result = pangine.parse_script_text(script).unwrap();
        let success = pangine.reference_concept("[success]").unwrap();
        assert_eq!(result, success);
    }
}

fn must_ref(pangine: &mut Pangine, script: &str) -> ConceptId {
    pangine
        .reference_concept(script)
        .unwrap()
        .unwrap_or_else(|| panic!("failed to reference concept: {script}"))
}
