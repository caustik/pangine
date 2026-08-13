mod support;

use pangine::{ConceptConstructionError, Pangine, Relevance};
use support::{pairs, PangineTest};

#[test]
fn relevance_addition_and_subtraction_are_stable() {
    let mut relevance = Relevance::new(1);

    relevance = relevance.checked_add(Relevance::new(-1)).unwrap();
    relevance = relevance.checked_add(Relevance::new(1)).unwrap();
    assert_eq!(relevance, Relevance::new(1));

    relevance = relevance.checked_sub(Relevance::new(-1)).unwrap();
    relevance = relevance.checked_sub(Relevance::new(1)).unwrap();
    assert_eq!(relevance, Relevance::new(1));

    assert_eq!(Relevance::new(i64::MAX).checked_add(Relevance::DEFAULT), None);
    assert_eq!(Relevance::new(i64::MIN).checked_neg(), None);
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
    assert_eq!(test.engine().get_relevance_map(&percept), Vec::new());

    let a = test.concept("[A]");
    let b = test.concept("[B]");
    assert!(test.engine_mut().set_percept_value(&percept, Some(a.clone())));
    assert_eq!(test.engine().get_value(&percept), Some(a.clone()));
    assert_eq!(test.engine().get_relevance_map(&percept), vec![(Relevance::DEFAULT, a.clone())]);
    assert_eq!(test.engine().recurse(&percept, false), "['direct']");
    assert_eq!(test.engine().recurse(&percept, true), "[A]");

    let merged = test.engine_mut().perform_merge(&percept, Some(&b));
    assert_eq!(merged, test.reference("[A]*[B]"));
    assert_eq!(test.engine().get_value(&percept), merged);
    assert_eq!(test.engine().get_relevance_map(&percept), merged.into_iter().map(|concept| (Relevance::DEFAULT, concept)).collect::<Vec<_>>());

    let ab = test.concept("[A][B]");
    let cd = test.concept("[C][D]");
    assert!(test.engine_mut().set_percept_value(&percept, Some(ab.clone())));
    let added = test.engine_mut().perform_addition(&percept, Some(&cd));
    assert_eq!(added, test.reference("[A][B]([C][D])"));
    assert!(test.engine_mut().set_percept_value(&percept, Some(ab)));
    let merged = test.engine_mut().perform_merge(&percept, Some(&cd));
    assert_eq!(merged, test.reference("[A][B][C][D]"));

    let memory = test.engine_mut().reference_percept("memory");
    let experience = test.concept("{[A]->[B]}");
    let experienced = test.engine_mut().perform_experience(&memory, Some(&experience));
    assert_eq!(experienced, Some(experience.clone()));
    assert_eq!(test.engine().get_value(&memory), Some(experience.clone()));
    assert_eq!(test.engine().get_relevance_map(&memory), vec![(Relevance::DEFAULT, experience.clone())]);

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
        "x2[A]x2[B]" => "x3[A]x3[B]",
        "x2[A]x2[B]" => "x2[A]x5[B]",
    });
    test.assert_equivalent(pairs! {
        "x2[A]x2[B]x3[B]" => "x2[A]x5[B]",
        "x2([A]*[B])x3([B]*[A])x2{[C]->[D]}" => "x5([A][B])x2{[C]->[D]}",
    });
    test.exec(["{x2[A]x2[B]->[C]}"]);
}

#[test]
fn direct_construction_reuses_parser_normalization_and_identity() {
    let mut test = PangineTest::new();
    let a = test.concept("[A]");
    let b = test.concept("[B]");
    let c = test.concept("[C]");
    let d = test.concept("[D]");
    let ab = test.concept("[A][B]");
    let cd = test.concept("[C][D]");

    let ordered = test.engine_mut().compose_ordered(&[a.clone(), b.clone()]).unwrap().unwrap();
    assert_eq!(ordered, test.concept("[A]->[B]"));
    let nested_ordered = test.engine_mut().compose_ordered(&[ordered.clone(), c.clone()]).unwrap().unwrap();
    assert_eq!(nested_ordered, test.concept("([A]->[B])->[C]"));
    assert_ne!(nested_ordered, test.concept("[A]->[B]->[C]"));
    assert_eq!(test.engine_mut().compose_ordered(&[]).unwrap(), None);
    assert_eq!(test.engine_mut().compose_ordered(std::slice::from_ref(&a)).unwrap(), Some(a.clone()));

    let grouped = test.engine_mut().compose_union(&[(Relevance::DEFAULT, ab.clone()), (Relevance::DEFAULT, cd.clone())]).unwrap().unwrap();
    assert_eq!(grouped, test.concept("([A][B])([C][D])"));
    assert_ne!(grouped, test.concept("[A][B][C][D]"));
    assert_eq!(test.engine_mut().compose_union(&[(Relevance::DEFAULT, cd), (Relevance::DEFAULT, ab.clone())]).unwrap(), Some(grouped.clone()));

    let weighted = test.engine_mut().compose_union(&[(Relevance::new(2), ab)]).unwrap().unwrap();
    assert_eq!(weighted, test.concept("x2([A][B])"));
    assert_eq!(test.engine_mut().compose_union(&[]).unwrap(), None);

    let flat = test
        .engine_mut()
        .compose_union(&[(Relevance::DEFAULT, a), (Relevance::DEFAULT, b), (Relevance::DEFAULT, c), (Relevance::DEFAULT, d)])
        .unwrap()
        .unwrap();
    assert_eq!(flat, test.concept("[A][B][C][D]"));
    assert_eq!(test.concept(&test.engine().format_concept(&grouped, false)), grouped);
}

#[test]
fn direct_union_accumulates_coefficients_and_reports_overflow() {
    let mut test = PangineTest::new();
    let a = test.concept("[A]");
    let x3_a = test.concept("x3[A]");

    let accumulated = test.engine_mut().compose_union(&[(Relevance::new(2), a.clone()), (Relevance::new(4), a.clone())]).unwrap().unwrap();
    assert_eq!(accumulated, test.concept("x6[A]"));
    assert_eq!(test.engine_mut().compose_union(&[(Relevance::DEFAULT, a.clone()), (Relevance::new(-1), a.clone())]).unwrap(), None);
    let count = test.engine().concept_count();
    assert_eq!(
        test.engine_mut().compose_union(&[(Relevance::new(i64::MAX), a.clone()), (Relevance::DEFAULT, a.clone())]),
        Err(ConceptConstructionError::RelevanceOverflow)
    );
    assert_eq!(test.engine().concept_count(), count);
    assert_eq!(test.engine_mut().compose_union(&[(Relevance::new(i64::MAX), x3_a.clone())]), Err(ConceptConstructionError::RelevanceOverflow));
    assert_eq!(test.engine().concept_count(), count);
}

#[test]
fn direct_construction_rejects_foreign_handles_and_does_not_evaluate_percepts() {
    let mut first = Pangine::new();
    let foreign = first.reference_concept("[foreign]").unwrap().unwrap();
    let mut second = Pangine::new();
    let local = second.reference_concept("[local]").unwrap().unwrap();
    let count = second.concept_count();

    assert_eq!(
        second.compose_union(&[(Relevance::DEFAULT, local.clone()), (Relevance::DEFAULT, foreign.clone())]),
        Err(ConceptConstructionError::ForeignConcept)
    );
    assert_eq!(second.compose_ordered(&[local.clone(), foreign]), Err(ConceptConstructionError::ForeignConcept));
    assert_eq!(second.concept_count(), count);

    let percept = second.reference_percept("live-child");
    assert!(second.set_percept_value(&percept, Some(local.clone())));
    let parent = second.compose_ordered(&[percept.clone(), local.clone()]).unwrap().unwrap();
    assert_eq!(second.get_ordered_components(&parent), Some(vec![percept, local]));
    assert_eq!(second.format_concept(&parent, false), "{['live-child']->[local]}");
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
fn unordered_identity_inversion_and_explicit_merge_are_canonical() {
    let mut test = PangineTest::new();

    test.assert_distinct(pairs! {
        "[A]" => "[A]*[B]",
        "[A]" => "[B]",
        "[A]*[B]" => "[B]",
        "[A]" => "![A]",
        "!([A]*[B])" => "[A]*[B]",
        "!([A]*[B])" => "![A]*![B]",
    });
    test.assert_equivalent(pairs! {
        "[A]*[B]" => "[B]*[A]",
        "([B]*[A])*([C]*[D])" => "([D]*[C])*([A]*[B])",
        "([D]*[C])*(([A]*[B])*[E]*[F]*([A]*[B]))" => "x2[A]x2[B][C][D][E][F]",
        "[A]" => "!![A]",
        "[A]" => "!(!([A]))",
        "!([A]*[B])" => "!(!!([B]*[A]))",
        "x-1[A]" => "![A]",
        "x-1([A]*[B])" => "!([A]*[B])",
        "x-2([A]*[B])([A]*[B])" => "!([A]*[B])",
        "!([A])*!([B])" => "![A]*(![B])",
        "![A]*(![B])" => "![A]*![B]",
        "![A]*![B]" => "(![A])*(![B])",
        "!([A]*[B])*!([C]*[D])" => "x-1([A][B])x-1([C][D])",
        "([A]*[B]*[C])/([A]*[B])" => "[C]",
    });
    test.assert_null(["x-2([A]*[B])*x2([A]*[B])", "!([A]*[B])([A]*[B])", "[A]*![A]", "([A]*[B])/([A]*[B])"]);
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
    let x_coefficients: Vec<_> = test.engine().get_relevance_map(&relevance).into_iter().map(|(relevance, _)| relevance.x_coefficient).collect();
    assert_eq!(x_coefficients, vec![3, 2, 1]);

    let equal_relevance = test.concept("[A]*[C]*[B]");
    let x_coefficients: Vec<_> = test.engine().get_relevance_map(&equal_relevance).into_iter().map(|(relevance, _)| relevance.x_coefficient).collect();
    assert_eq!(x_coefficients, vec![1, 1, 1]);
}

#[test]
fn debug_console_rows_use_canonical_order_instead_of_allocation_order() {
    let mut forward = Pangine::new();
    let forward_a = forward.reference_concept("[A]").unwrap().unwrap();
    let forward_b = forward.reference_concept("[B]").unwrap().unwrap();
    let forward_union = forward.reference_concept("[A][B]").unwrap().unwrap();

    let mut reverse = Pangine::new();
    let reverse_b = reverse.reference_concept("[B]").unwrap().unwrap();
    let reverse_a = reverse.reference_concept("[A]").unwrap().unwrap();
    let reverse_union = reverse.reference_concept("[A][B]").unwrap().unwrap();

    assert_eq!(forward.debug_console_lines(Some(&forward_union)), vec!["  [A]", "  [B]"]);
    assert_eq!(reverse.debug_console_lines(Some(&reverse_union)), vec!["  [A]", "  [B]"]);
    drop((forward_a, forward_b, reverse_a, reverse_b));
}

#[test]
fn debug_console_rows_expose_signed_coefficients() {
    let mut test = PangineTest::new();

    let a = test.concept("[A]");
    assert_eq!(test.engine().debug_console_lines(Some(&a)), vec!["  [A]"]);
    assert_eq!(test.engine().debug_console_lines(None), vec!["  []"]);

    let relevance = test.concept("[A]x2[B]x-1[C]");
    assert_eq!(test.engine().debug_console_lines(Some(&relevance)), vec!["  x2 [B]", "  [A]", "  ![C]"]);
}
