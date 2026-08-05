mod support;

use pangine::{Pangine, Relevance};
use support::{pairs, PangineTest};

#[test]
fn relevance_addition_and_subtraction_are_stable() {
    let mut relevance = Relevance::new(1.0, 1.0);

    relevance.add(Relevance::new(1.0, -1.0));
    relevance.add(Relevance::new(1.0, 1.0));
    assert_eq!(relevance, Relevance::new(1.0, 1.0));

    relevance.sub(Relevance::new(1.0, -1.0));
    relevance.sub(Relevance::new(1.0, 1.0));
    assert_eq!(relevance, Relevance::new(1.0, 1.0));
}

#[test]
fn references_names_and_parentheses() {
    let mut test = PangineTest::new();

    test.assert_distinct(pairs! {
        "[A]" => "[B]",
        "[A]" => "[LONGER_NAME]",
        "[LONGER_NAME]" => "[LONGER-NAME]",
        "[LONGER-NAME]" => "[EVEN LONGER NAME]",
        "['A']" => "['B']",
        "[A]" => "['A']",
    });
    test.assert_equivalent(pairs! {
        "     [A]" => "[A]",
        " \t\r\n[B]" => "[B]",
        "([A])" => "[A]",
        "(([A]))" => "[A]",
        "(['A'])" => "['A']",
        "((['A']))" => "['A']",
    });
    test.assert_invalid(["[?]", "[?A]", "[&A]", "[%]"]);

    let concept_a = test.concept("[A]");
    assert_eq!(test.engine().get_name(&concept_a), Some("A"));
}

#[test]
fn public_api_mutations_update_the_unified_percept_state() {
    let mut test = PangineTest::new();

    let percept = test.engine_mut().reference_percept("direct");
    assert_eq!(test.reference("['direct']"), Some(percept.clone()));
    assert_eq!(test.engine().get_percept(&percept), Some(percept.clone()));
    assert_eq!(test.engine().get_value(&percept), None);
    assert_eq!(test.engine().get_percept_roots(&percept), Some(Vec::new()));

    let a = test.concept("[A]");
    let b = test.concept("[B]");
    assert!(test.engine_mut().set_percept_value(&percept, Some(a.clone())));
    assert_eq!(test.engine().get_value(&percept), Some(a.clone()));
    assert_eq!(test.engine().get_percept_roots(&percept), Some(vec![a.clone()]));
    assert_eq!(test.engine().recurse(&percept, false), "['direct']");
    assert_eq!(test.engine().recurse(&percept, true), "[A]");

    let merged = test.engine_mut().perform_merge(&percept, Some(&b));
    assert_eq!(merged, test.reference("[A]*[B]"));
    assert_eq!(test.engine().get_value(&percept), merged);
    assert_eq!(test.engine().get_percept_roots(&percept), merged.into_iter().collect::<Vec<_>>().into());

    let memory = test.engine_mut().reference_percept("memory");
    let experience = test.concept("{[A]->[B]}");
    let experienced = test.engine_mut().perform_experience(&memory, Some(&experience));
    assert_eq!(experienced, Some(experience.clone()));
    assert_eq!(test.engine().get_value(&memory), Some(experience.clone()));
    assert_eq!(test.engine().get_percept_roots(&memory), Some(vec![experience.clone()]));
    assert_eq!(test.engine().get_percept_root_count(&memory, &experience), Some(1));

    let left = test.engine_mut().reference_percept("left");
    let right = test.engine_mut().reference_percept("right");
    let ordered = test.concept("{['left']->['right']}");
    assert_eq!(test.engine().get_ordered_components(&ordered), Some(vec![left, right]));
}

#[test]
fn ordered_and_unordered_compositions_are_canonical() {
    let mut test = PangineTest::new();

    let a = test.concept("[A]");
    let b_percept = test.concept("['B']");
    let ordered = test.concept("{[A]->['B']}");
    assert_eq!(test.engine().get_ordered_components(&ordered), Some(vec![a, b_percept]));

    let nested = test.concept("{{[A]->['B']}->{[Q]->[D]}}");
    let question_to_d = test.concept("{[Q]->[D]}");
    assert_eq!(test.engine().get_ordered_components(&nested), Some(vec![ordered, question_to_d]));
    test.exec(["{{[A]->[B]}->[C]}", "{[C]->{[A]->[B]}}"]);

    test.assert_distinct(pairs! {
        "50%[A]50%[B]" => "25%[A]25%[B]",
        "25%[A]25%[B]" => "25%[A]50%[B]50%[B]",
    });
    test.assert_equivalent(pairs! {
        "25%[A]50%[B]50%[B]" => "25%[A]25%[B]75%[B]",
        "25%([A]*[B])25%([B]*[A])10%{[C]->[D]}10%{[C]->[D]}" => "15%([B]*[A])35%([B]*[A])5%{[C]->[D]}15%{[C]->[D]}",
    });
    test.exec(["{25%[A]25%[B]->[C]}"]);
}

#[test]
fn composite_identity_survives_registry_growth_and_cleanup() {
    let mut pangine = Pangine::new();
    let ordered = pangine.reference_concept("[target-a]->[target-b]").unwrap().unwrap();
    let unordered = pangine.reference_concept("[target-a]*[target-b]").unwrap().unwrap();

    let retained = (0..256).map(|index| pangine.reference_concept(&format!("[left{index}]->[right{index}]")).unwrap().unwrap()).collect::<Vec<_>>();
    assert_eq!(pangine.reference_concept("[target-a]->[target-b]").unwrap(), Some(ordered.clone()));
    assert_eq!(pangine.reference_concept("[target-b]*[target-a]").unwrap(), Some(unordered.clone()));

    drop(retained);
    for index in 256..768 {
        drop(pangine.reference_concept(&format!("[left{index}]->[right{index}]")).unwrap());
    }
    assert_eq!(pangine.reference_concept("[target-a]->[target-b]").unwrap(), Some(ordered));
    assert_eq!(pangine.reference_concept("[target-a]*[target-b]").unwrap(), Some(unordered));
}

#[test]
fn union_inversion_normalization_and_null_removal_are_canonical() {
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
        "([D]*[C])*(([A]*[B])*[E]*[F]*([A]*[B]))" => "100%x2[A]100%x2[B]100%[C]100%[D]100%[E]100%[F]",
        "[A]" => "!![A]",
        "[A]" => "!(!([A]))",
        "!([A]*[B])" => "!(!!([B]*[A]))",
        "x-1[A]" => "![A]",
        "x-1[A]x-1[B]" => "!([A]*[B])",
        "x-2[A]x-2[B]*([A]*[B])" => "!([A]*[B])",
        "!([A])*!([B])" => "![A]*(![B])",
        "![A]*(![B])" => "![A]*![B]",
        "![A]*![B]" => "(![A])*(![B])",
        "!([A]*[B])*!([C]*[D])" => "(![A]*![B]*![C]*![D])",
        "([A]*[B]*[C])*!([A]*[B])" => "[C]",
    });
    test.assert_null(["x-2([A]*[B])*x2([A]*[B])", "!([A]*[B])*([A]*[B])", "[A]*![A]", "([A]*[B])*!([A]*[B])"]);
    test.assert_invalid(["([A]*![B})*(![A]*[B])"]);
}

#[test]
fn formatting_round_trips_and_relevance_entries_are_ordered() {
    let mut test = PangineTest::new();
    let concept = test.concept("{[test]->[okay]}");
    let printed = test.engine().format_concept(&concept, false);
    let reparsed = test.concept(&printed);
    assert_eq!(concept, reparsed);

    let relevance = test.concept("[A]*[A]*[C]*[C]*[B]*[C]");
    let strengths: Vec<_> = test.engine().get_relevance_map(&relevance).into_iter().map(|(relevance, _)| relevance.strength).collect();
    assert_eq!(strengths, vec![3.0, 2.0, 1.0]);

    let equal_relevance = test.concept("[A]*[C]*[B]");
    let strengths: Vec<_> = test.engine().get_relevance_map(&equal_relevance).into_iter().map(|(relevance, _)| relevance.strength).collect();
    assert_eq!(strengths, vec![1.0, 1.0, 1.0]);
}

#[test]
fn debug_console_rows_expose_relevance_components() {
    let mut test = PangineTest::new();

    let a = test.concept("[A]");
    assert_eq!(test.engine().debug_console_lines(Some(&a)), vec!["  [A]"]);
    assert_eq!(test.engine().debug_console_lines(None), vec!["  []"]);

    let relevance = test.concept("50%[A]x2[B]x-1[C]");
    assert_eq!(test.engine().debug_console_lines(Some(&relevance)), vec!["  50% [A]", "  x2 [B]", "  ![C]"]);
}
