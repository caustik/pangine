mod support;

use pangine::{Pangine, ParseError, Relevance};
use support::{pairs, PangineTest};

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
    let mut test = PangineTest::new();

    test.assert_distinct(pairs! {
        "[A]" => "[B]",
        "[A]" => "[LONGER_NAME]",
        "[LONGER_NAME]" => "[LONGER-NAME]",
        "[LONGER-NAME]" => "[EVEN LONGER NAME]",
        "['A']" => "['B']",
        "[?]" => "[?LONGER_NAME]",
        "[?LONGER_NAME]" => "[?LONGER-NAME]",
        "[A]" => "['A']",
        "[A]" => "[?A]",
    });
    test.assert_equivalent(pairs! {
        "     [A]" => "[A]",
        " \t\r\n[B]" => "[B]",
        "[?]" => "[?]",
        "([A])" => "[A]",
        "(([A]))" => "[A]",
        "(['A'])" => "['A']",
        "((['A']))" => "['A']",
    });
    test.assert_invalid(["[??BAD_NAME]"]);

    let concept_a = test.concept("[A]");
    assert_eq!(test.engine().get_name(&concept_a), Some("A"));

    let test1 = test.concept("[test1]");
    let test2 = test.concept("[test2]");
    let with_params = test.engine_mut().reference_concept_with_params("{[test3]->[%]}*[%]", &[test1, test2]).unwrap().unwrap();
    let without_params = test.concept("{[test3]->[test1]}*[test2]");
    assert_eq!(with_params, without_params);
}

// Parity anchors:
// 1.x/pangine/include/pangine/pae_pangine.h:55,67,73,80,83
// 1.x/pangine/include/pangine/pae_concept.h:69,74,79
// 1.x/pangine/include/pangine/pae_percept.h:30,35,38
#[test]
fn public_api_surface_matches_1x_boundaries() {
    let mut test = PangineTest::new();

    let percept = test.engine_mut().reference_percept("direct");
    assert_eq!(test.reference("['direct']"), Some(percept.clone()));
    assert_eq!(test.engine().get_percept(&percept), Some(percept.clone()));
    assert_eq!(test.engine().get_value(&percept), None);

    let a = test.concept("[A]");
    let b = test.concept("[B]");
    assert!(test.engine_mut().set_percept_value(&percept, Some(a.clone())));
    assert_eq!(test.engine().get_value(&percept), Some(a.clone()));
    assert_eq!(test.engine().recurse(&percept, false), "['direct']");
    assert_eq!(test.engine().recurse(&percept, true), "[A]");

    let merged = test.engine_mut().perform_merge(&percept, Some(&b));
    assert_eq!(merged, test.reference("[A]*[B]"));
    assert_eq!(test.engine().get_value(&percept), Some(a));
    assert!(test.engine_mut().set_percept_value(&percept, merged.clone()));
    assert_eq!(test.engine().get_value(&percept), merged);

    let memory = test.engine_mut().reference_percept("memory");
    let experience = test.concept("{[A]->[B]}");
    let experienced = test.engine_mut().perform_experience(&memory, Some(&experience));
    assert_eq!(experienced, test.reference("<100%{[A]->[B]}, 100%[A], 100%[B]>"));
    assert_eq!(test.engine().get_value(&memory), None);

    let left = test.engine_mut().reference_percept("left");
    let right = test.engine_mut().reference_percept("right");
    let correlation = test.concept("{['left']->['right']}");
    assert_eq!(test.engine().get_percept_a(&correlation), Some(left));
    assert_eq!(test.engine().get_percept_b(&correlation), Some(right));
}

// Parity anchors:
// 1.x/pangine/src/test/common/test_reference_concept.cpp:218,244,272,313
#[test]
fn correlations_observations_and_relevance_are_canonical() {
    let mut test = PangineTest::new();

    let a = test.concept("[A]");
    let b_percept = test.concept("['B']");
    let correlation = test.concept("{[A]->['B']}");
    assert_eq!(test.engine().get_correlation_a(&correlation), Some(a.clone()));
    assert_eq!(test.engine().get_correlation_b(&correlation), Some(b_percept));

    let nested = test.concept("{{[A]->['B']}->{[?]->[D]}}");
    let question_to_d = test.concept("{[?]->[D]}");
    assert_eq!(test.engine().get_correlation_a(&nested), Some(correlation.clone()));
    assert_eq!(test.engine().get_correlation_b(&nested), Some(question_to_d));
    test.exec(["{{[A]->[B]}->[C]}", "{[C]->{[A]->[B]}}"]);

    let observation = test.concept("?[A]:[B]");
    let b = test.concept("[B]");
    assert_eq!(test.engine().get_observer(&observation), Some(a));
    assert_eq!(test.engine().get_observation(&observation), Some(b));
    let nested_observation = test.concept("?(?[A]:[B]):(?[C]:[D])");
    assert_eq!(test.engine().get_observer(&nested_observation), Some(observation));
    test.assert_formats(pairs! {
        "?[observer]:[observation]" => "?[observer]:[observation]",
        "?[weather_station]:{[rain]->[wet_ground]}" => "?[weather_station]:{[rain]->[wet_ground]}",
        "?(?[observer]:[report]):(?[camera]:([red][square]))" => "?(?[observer]:[report]):(?[camera]:([red][square]))",
    });
    test.assert_distinct(pairs! {
        "?[observer]:[observation]" => "[observation]",
        "?[observer]:[observation]" => "{[observer]->[observation]}",
    });

    test.assert_distinct(pairs! {
        "<50%[A], 50%[B]>" => "<25%[A], 25%[B]>",
        "<25%[A], 25%[B]>" => "<25%[A], 50%[B], 50%[B]>",
    });
    test.assert_equivalent(pairs! {
        "<25%[A], 50%[B], 50%[B]>" => "<25%[A], 25%[B], 75%[B]>",
        "<25%[A]*[B], 25%[B]*[A], 10%{[C]->[D]}, 10%{[C]->[D]}>" => "<15%[B]*[A], 35%[B]*[A], 5%{[C]->[D]}, 15%{[C]->[D]}>",
    });
    test.exec(["{<25%[A], 25%[B]>->[C]}"]);
}

// Parity anchors:
// 1.x/pangine/src/test/common/test_reference_concept.cpp:338,390,421,591,613,630
#[test]
fn union_inversion_normalization_and_null_removal_match_1x() {
    let mut test = PangineTest::new();

    test.assert_distinct(pairs! {
        "[A]" => "[A]*[B]",
        "[A]" => "[B]",
        "[A]*[B]" => "[B]",
        "[A]" => "![A]",
        "!([A]*[B])" => "[A]*[B]",
    });
    test.assert_equivalent(pairs! {
        "[A]*[B]" => "[B]*[A]",
        "([B]*[A])*([C]*[D])" => "([D]*[C])*([A]*[B])",
        "([D]*[C])*(([A]*[B])*[E]*[F]*([A]*[B]))" => "<100%x2[A], 100%x2[B], 100%[C], 100%[D], 100%[E], 100%[F]>",
        "[A]" => "!![A]",
        "[A]" => "!(!([A]))",
        "!([A]*[B])" => "!(!!([B]*[A]))",
        "<x-1[A]>" => "![A]",
        "<x-1[A], x-1[B]>" => "!([A]*[B])",
        "<x-2[A], x-2[B]>*([A]*[B])" => "!([A]*[B])",
        "!([A])*!([B])" => "![A]*(![B])",
        "![A]*(![B])" => "![A]*![B]",
        "![A]*![B]" => "(![A])*(![B])",
        "?![A]*[B]:[C]" => "?(![A]*[B]):[C]",
        "!([A]*[B])*!([C]*[D])" => "(![A]*![B]*![C]*![D])",
        "([A]*[B]*[C])*!([A]*[B])" => "[C]",
    });
    test.assert_null(["<x-2([A]*[B])>*<x2([A]*[B])>", "!([A]*[B])*([A]*[B])", "[A]*![A]", "([A]*[B])*!([A]*[B])"]);
    test.assert_invalid(["([A]*![B})*(![A]*[B])"]);
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:755,810,852,1226
// 1.x/pangine/src/test/common/test_reference_concept.cpp:421,448,554,630,674
#[test]
fn percept_merge_experience_recursion_questions_and_decisions_match_1x() {
    let mut test = PangineTest::new();

    test.assert_equivalent(pairs! {
        "['test'] *= [A]" => "[A]",
        "['test'] *= [B]" => "[A]*[B]",
        "['test'] *= ![A]" => "[B]",
        "$['test']" => "[B]",
    });

    let experience1 = test.concept("['mind01'] ~= {[tigger]->[meows]}");
    let experience2 = test.concept("['mind01'] ~= {[tigger]->[purrs]}");
    let expected2 = test.concept("<100%{[tigger]->[meows]}, 100%{[tigger]->[purrs]}, 100%x2.0[tigger], 100%[meows], 100%[purrs]>");
    let experience3 = test.concept("['mind01'] ~= {[tigger]->[purrs]}");
    let expected3a = test.concept("<100%{[tigger]->[meows]}, 100%x2{[tigger]->[purrs]}, 100%[tigger], 100%x2[tigger], 100%[meows], 100%x2[purrs]>");
    let expected3b = test.concept("<100%{[tigger]->[meows]}, 100%x2{[tigger]->[purrs]}, 100%x3.0[tigger], 100%[meows], 100%x2[purrs]>");

    assert_ne!(experience1, experience2);
    assert_eq!(experience2, expected2);
    assert_ne!(experience3, experience2);
    assert_eq!(experience3, expected3a);
    assert_eq!(experience3, expected3b);
    assert_ne!(expected3b, test.concept("<100%{[tigger]->[meows]}, 100%x2.001{[tigger]->[purrs]}>"));
    let single_b = test.concept("['A'] ~= [B]");
    let double_b = test.concept("['A'] ~= [B]");
    assert_ne!(single_b, double_b);
    assert_eq!(double_b, test.concept("[B]*[B]"));

    let recursive = test.concept("['dog_eats_dog'] = {{[dog]->[eats]}->[dog]}");
    test.exec(["['wisdom'] ~= ['dog_eats_dog']", "['obvious'] ~= {{['dog_eats_dog']->[is]}->['obvious']}"]);
    let recursive_a = test.engine().get_correlation_a(&recursive).unwrap();
    assert_eq!(test.engine().get_correlation_a(&recursive_a), test.engine().get_correlation_b(&recursive));

    let okay = test.concept("['okay'] ~= {[A]->[B]}");
    let okay_twice = test.concept("['okay'] ~= {[A]->[B]}");
    let still_okay = test.concept("['okay'] ~= !{[A]->[B]}");
    assert_ne!(okay, okay_twice);
    assert_eq!(still_okay, okay);

    test.exec([
        "['mind'] ~= {{[A]->[species_is]}->[cat]}",
        "['mind'] ~= {{[B]->[species_is]}->[dog]}",
        "['mind'] ~= {{[A]->[sound_is]}->[meow]}",
        "['mind'] ~= {{[B]->[sound_is]}->[bark]}",
        "['mind'] ~= {{[C]->[species_is]}->[cat]}",
        "['mind'] ~= {{[C]->[sound_is]}->['output']}",
        "[meow]",
    ]);
}

#[test]
fn correlation_questions_keep_each_output_binding_separate() {
    let mut test = PangineTest::new();
    test.exec(["['test'] ~= {[A]->[B]}"]);

    test.assert_equivalent(pairs! {
        "['test'] @ {['1']->[C]}*{['2']->[B]}" => "{['1']->[C]}*{['2']->[B]}",
        "$['1']" => "[A]",
        "$['2']" => "<x2[A]>",
    });
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:109
// 1.x/pangine/src/test/common/test_parse_text.cpp:20
#[test]
fn parse_text_matches_1x_word_union_behavior() {
    let mut test = PangineTest::new();
    let text1 = test.engine_mut().parse_text("simple parse text test").unwrap();
    let text2 = test.engine_mut().parse_text("simple text test %%%%% $$parse").unwrap();
    let expected = test.concept("[simple]*[parse]*[text]*[test]");

    assert_eq!(text1, text2);
    assert_eq!(text2, expected);
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_concept.cpp:21,113
// 1.x/pangine/src/test/common/test_pangine.cpp:41,110
#[test]
fn formatting_round_trips_and_relevance_map_orders_like_1x() {
    let mut test = PangineTest::new();
    let concept = test.concept("['test'] ~= {[test]->[okay]}");
    let printed = test.engine().format_concept(&concept, false);
    let reparsed = test.concept(&printed);
    assert_eq!(concept, reparsed);

    let relevance = test.concept("[A]*[A]*[C]*[C]*[B]*[C]");
    let strengths: Vec<_> = test.engine().get_relevance_map(&relevance).into_iter().map(|(relevance, _)| relevance.strength).collect();
    assert_eq!(strengths, vec![3.0, 2.0, 1.0]);

    let equal_relevance = test.concept("[A]*[C]*[B]");
    let strengths: Vec<_> = test.engine().get_relevance_map(&equal_relevance).into_iter().map(|(relevance, _)| relevance.strength).collect();
    assert_eq!(strengths, vec![1.0, 1.0, 1.0]);

    let single = test.concept("[A]");
    let single_relevance = test.engine().get_relevance_map(&single);
    assert_eq!(single_relevance.len(), 1);
    assert_eq!(single_relevance[0].0.strength, 1.0);
}

// Parity anchor:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:1311
#[test]
fn debug_console_rows_match_1x_display_rules() {
    let mut test = PangineTest::new();

    let a = test.concept("[A]");
    assert_eq!(test.engine().debug_console_lines(Some(&a), false), vec!["  [A]"]);
    assert_eq!(test.engine().debug_console_lines(None, false), vec!["  []"]);

    let relevance = test.concept("<50%[A], x2[B], x-1[C]>");
    assert_eq!(test.engine().debug_console_lines(Some(&relevance), false), vec!["  50% [A]", "  x2 [B]", "  ![C]"]);

    let combined_relevance = test.concept("<50%x2[a]>");
    assert_eq!(test.engine().debug_console_lines(Some(&combined_relevance), false), vec!["  50%x2 [a]"]);

    test.exec(["['test'] = [A]"]);
    let percept = test.concept("['test']");
    assert_eq!(test.engine().debug_console_lines(Some(&percept), true), vec!["  [A]"]);
}

// Parity anchors:
// 1.x/pangine/src/pangine/common/pae_pangine.cpp:39
// 1.x/pangine/src/test/common/test_parse_script.cpp:20,34,48,62,76,90,104,118
#[test]
fn historical_1x_scripts_return_success() {
    let scripts = [
        ("test_pangine.pae", include_str!("fixtures/1x/test_pangine.pae")),
        ("test_merge.pae", include_str!("fixtures/1x/test_merge.pae")),
        ("test_syllogism.pae", include_str!("fixtures/1x/test_syllogism.pae")),
        ("test_counting.pae", include_str!("fixtures/1x/test_counting.pae")),
        ("test_experience.pae", include_str!("fixtures/1x/test_experience.pae")),
        ("test_decision.pae", include_str!("fixtures/1x/test_decision.pae")),
        ("test_rule110.pae", include_str!("fixtures/1x/test_rule110.pae")),
    ];

    for (name, script) in scripts {
        let mut pangine = Pangine::new();
        let result = pangine.parse_script_text(script).unwrap();
        let success = pangine.reference_concept("[success]").unwrap();
        assert_eq!(result, success, "historical fixture failed: {name}");
    }
}

#[test]
fn historical_1x_question_fixture_is_not_a_recursive_projection_fixture() {
    let mut pangine = Pangine::new();
    let mut details = Vec::new();
    let result = pangine.parse_script_text_with_details(include_str!("fixtures/1x/test_question.pae"), &mut details);
    let details = String::from_utf8(details).expect("details should be UTF-8");

    assert!(matches!(result, Err(ParseError::InvalidSyntax)));
    assert!(details.contains("['current'] ~= $['1']*$['2']*$['3']"));
}
