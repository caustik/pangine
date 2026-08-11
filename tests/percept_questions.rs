use pangine::{ConceptId, Pangine, Relevance};
use std::collections::BTreeSet;

#[test]
fn question_rejects_fixed_mismatches_instead_of_treating_them_as_weak_matches() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    must_ref(&mut pangine, "['memory'] ~= {[B]->[D]}");
    ask(&mut pangine, "['memory'] @ {['X']->[A]}");

    let candidates = named_value(&mut pangine, "$['X']");
    assert_eq!(candidates.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["C"]);
}

#[test]
fn question_keeps_ordered_output_bindings_distinct() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}*{[B]->[D]}");
    let answer = must_ref(&mut pangine, "['memory'] @ {['X']->[A]}*{[B]->['Y']}");
    assert_eq!(answer, must_ref(&mut pangine, "{[C]->[A]}*{[B]->[D]}"));

    let x = named_value(&mut pangine, "$['X']");
    assert_eq!(x.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["C"]);

    let y = named_value(&mut pangine, "$['Y']");
    assert_eq!(y.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["D"]);
}

#[test]
fn question_matches_shared_unordered_context_inside_ordered_components() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {([C]*[A])->([B]*[D])}");
    ask(&mut pangine, "['memory'] @ {(['X']*[A])->([B]*['Y'])}");

    let x = named_value(&mut pangine, "$['X']");
    assert_eq!(x.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["C"]);

    let y = named_value(&mut pangine, "$['Y']");
    assert_eq!(y.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["D"]);
}

#[test]
fn evaluation_recursively_resolves_percepts_inside_a_shape() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['A'] = [resolved-a]");
    must_ref(&mut pangine, "['B'] = ['A']->[resolved-b]");

    assert_eq!(must_ref(&mut pangine, "$({['B']->['A']})"), must_ref(&mut pangine, "{{[resolved-a]->[resolved-b]}->[resolved-a]}"));
}

#[test]
fn question_recursively_finds_an_exact_nested_match() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->{[A]->[Z]}}");
    must_ref(&mut pangine, "['memory'] ~= {[B]->{[D]->[Y]}}");
    ask(&mut pangine, "['memory'] @ {['X']->{[A]->[Z]}}");

    let candidates = named_value(&mut pangine, "$['X']");
    assert_eq!(candidates.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["C"]);
}

#[test]
fn question_writes_each_output_percept_separately() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    ask(&mut pangine, "['memory'] @ {['X']->['Y']}");

    let x = named_value(&mut pangine, "$['X']");
    let y = named_value(&mut pangine, "$['Y']");
    assert_eq!(x[0].1, "C");
    assert_eq!(y[0].1, "A");
}

#[test]
fn repeated_output_percept_uses_one_binding_across_selected_sources() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['Alice'] ~= {[A]->[B]}");
    must_ref(&mut pangine, "['Bob'] ~= {[A]->[B]}");
    must_ref(&mut pangine, "['Carol'] ~= {[C]->[C]}");
    ask(&mut pangine, "['Alice']['Bob']['Carol'] @ {['X']->['X']}");

    let candidates = named_value(&mut pangine, "$['X']");
    assert_eq!(candidates.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["C"]);
}

#[test]
fn repeated_output_binding_stays_consistent_across_one_ordered_question() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['Alice'] ~= {([A]*[P])->([B]*[Q])}");
    must_ref(&mut pangine, "['Bob'] ~= {([A]*[P])->([B]*[Q])}");
    must_ref(&mut pangine, "['Carol'] ~= {([C]*[P])->([C]*[Q])}");
    ask(&mut pangine, "['Alice']['Bob']['Carol'] @ {(['X']*[P])->(['X']*[Q])}");

    let candidates = named_value(&mut pangine, "$['X']");
    assert_eq!(candidates.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["C"]);
}

#[test]
fn repeated_experience_increments_the_exact_root_count() {
    let mut pangine = Pangine::new();
    let memory = pangine.reference_percept("memory");
    let ordered = must_ref(&mut pangine, "{[cat]->[purrs]}");

    must_ref(&mut pangine, "['memory'] ~= {[cat]->[purrs]}");
    must_ref(&mut pangine, "['memory'] ~= {[cat]->[purrs]}");
    assert_eq!(pangine.get_percept_roots(&memory), Some(vec![ordered.clone()]));
    assert_eq!(pangine.get_percept_root_count(&memory, &ordered), Some(2));
    assert_eq!(pangine.format_concept(&pangine.get_value(&memory).unwrap(), false), "x2{[cat]->[purrs]}");

    ask(&mut pangine, "['memory'] @ {[cat]->['sound']}");
    let candidates = named_value(&mut pangine, "$['sound']");
    assert_eq!(candidate_weight(&candidates, "purrs"), 2.0);
}

#[test]
fn coefficient_root_and_repeated_atomic_root_remain_distinct() {
    let mut pangine = Pangine::new();
    let memory = pangine.reference_percept("memory");

    must_ref(&mut pangine, "['memory'] ~= [A]");
    must_ref(&mut pangine, "['memory'] ~= [A]");
    must_ref(&mut pangine, "['memory'] ~= x2[A]");

    let roots = pangine.get_percept_roots(&memory).unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(roots.into_iter().collect::<BTreeSet<_>>(), BTreeSet::from([must_ref(&mut pangine, "[A]"), must_ref(&mut pangine, "x2[A]")]));
    let atomic = must_ref(&mut pangine, "[A]");
    let coefficient_root = must_ref(&mut pangine, "x2[A]");
    assert_eq!(pangine.get_percept_root_count(&memory, &atomic), Some(2));
    assert_eq!(pangine.get_percept_root_count(&memory, &coefficient_root), Some(1));
    assert_eq!(pangine.format_concept(&pangine.get_value(&memory).unwrap(), false), "x4[A]");
}

#[test]
fn roots_with_different_coefficients_remain_distinct_in_the_materialized_value() {
    let mut pangine = Pangine::new();
    let memory = pangine.reference_percept("memory");

    must_ref(&mut pangine, "['memory'] ~= [A][B]");
    must_ref(&mut pangine, "['memory'] ~= x2[A]x2[B]");

    let first = must_ref(&mut pangine, "[A][B]");
    let second = must_ref(&mut pangine, "x2[A]x2[B]");
    assert_eq!(pangine.get_percept_roots(&memory).unwrap().into_iter().collect::<BTreeSet<_>>(), BTreeSet::from([first, second]));

    let materialized = must_ref(&mut pangine, "([A][B])(x2[A]x2[B])");
    assert_eq!(pangine.get_value(&memory), Some(materialized.clone()));
    assert_eq!(must_ref(&mut pangine, "$['memory']"), materialized);
}

#[test]
fn flat_and_grouped_unions_remain_distinct_exact_experience_roots() {
    let mut pangine = Pangine::new();
    let memory = pangine.reference_percept("memory");

    must_ref(&mut pangine, "['memory'] ~= [A][B][A][B]");
    must_ref(&mut pangine, "['memory'] ~= ([A][B])([A][B])");

    let flat = must_ref(&mut pangine, "x2[A]x2[B]");
    let grouped = must_ref(&mut pangine, "x2([A][B])");
    assert_eq!(pangine.get_percept_roots(&memory).unwrap().into_iter().collect::<BTreeSet<_>>(), BTreeSet::from([flat.clone(), grouped.clone()]));
    assert_eq!(pangine.get_percept_root_count(&memory, &flat), Some(1));
    assert_eq!(pangine.get_percept_root_count(&memory, &grouped), Some(1));
    let materialized = must_ref(&mut pangine, "x2([A][B])(x2[A]x2[B])");
    assert_eq!(pangine.get_value(&memory), Some(materialized));
}

#[test]
fn cancelling_materialized_value_does_not_delete_exact_roots() {
    let mut pangine = Pangine::new();
    let memory = pangine.reference_percept("memory");

    must_ref(&mut pangine, "['memory'] ~= [A]");
    assert!(pangine.reference_concept("['memory'] ~= ![A]").unwrap().is_none());

    assert!(pangine.get_value(&memory).is_none());
    assert_eq!(pangine.get_percept_roots(&memory).unwrap().len(), 2);
}

#[test]
fn materialized_value_does_not_erase_exact_root_boundaries() {
    let mut pangine = Pangine::new();
    let whole = pangine.reference_percept("whole");
    let split = pangine.reference_percept("split");

    must_ref(&mut pangine, "['whole'] = [A][B]");
    must_ref(&mut pangine, "['split'] ~= [A]");
    must_ref(&mut pangine, "['split'] ~= [B]");

    assert_eq!(pangine.get_value(&whole), pangine.get_value(&split));
    assert_eq!(pangine.get_percept_roots(&whole).unwrap().len(), 1);
    assert_eq!(pangine.get_percept_roots(&split).unwrap().len(), 2);
}

#[test]
fn ordinary_operations_replace_experience_with_one_result_root() {
    let mut pangine = Pangine::new();
    let memory = pangine.reference_percept("memory");

    must_ref(&mut pangine, "['memory'] ~= [A]");
    must_ref(&mut pangine, "['memory'] ~= [B]");
    assert_eq!(pangine.get_percept_roots(&memory).unwrap().len(), 2);

    must_ref(&mut pangine, "['memory'] += [C]");
    assert_eq!(pangine.get_percept_roots(&memory).unwrap(), vec![must_ref(&mut pangine, "[A][B][C]")]);

    must_ref(&mut pangine, "['memory'] ~= [D]");
    must_ref(&mut pangine, "['memory'] *= [E]");
    assert_eq!(pangine.get_percept_roots(&memory).unwrap(), vec![must_ref(&mut pangine, "([A][B][C])[D][E]")]);

    must_ref(&mut pangine, "['memory'] -= [A]");
    assert_eq!(pangine.get_percept_roots(&memory).unwrap(), vec![must_ref(&mut pangine, "![A]([A][B][C])[D][E]")]);
}

#[test]
fn at_binds_after_the_complete_unparenthesized_left_union() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['Alice'] ~= {[cat]->[purr]}");
    must_ref(&mut pangine, "['Bob'] ~= {[cat]->[meow]}");

    ask(&mut pangine, "['Alice'] @ {[cat]->['alice-sound']}");
    let alice = named_value(&mut pangine, "$['alice-sound']");
    assert!(alice.iter().any(|(_, name)| name == "purr"));
    assert!(!alice.iter().any(|(_, name)| name == "meow"));

    ask(&mut pangine, "['Alice']['Bob'] @ {[cat]->['sound']}");
    let unparenthesized = must_ref(&mut pangine, "$['sound']");
    let candidates = named_relevance(&pangine, &unparenthesized);
    assert!(candidates.iter().any(|(_, name)| name == "purr"));
    assert!(candidates.iter().any(|(_, name)| name == "meow"));

    ask(&mut pangine, "(['Alice']['Bob']) @ {[cat]->['grouped-sound']}");
    assert_eq!(must_ref(&mut pangine, "$['grouped-sound']"), unparenthesized);
}

#[test]
fn flat_ordered_questions_match_only_exact_contiguous_windows() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['alice'] ~= [cat]->[eats]->[cat_food]");

    ask(&mut pangine, "['alice'] @ ['what']->[eats]");
    assert_eq!(must_ref(&mut pangine, "$['what']"), must_ref(&mut pangine, "[cat]"));

    ask(&mut pangine, "['alice'] @ [eats]->['what-next']");
    assert_eq!(must_ref(&mut pangine, "$['what-next']"), must_ref(&mut pangine, "[cat_food]"));

    ask(&mut pangine, "['alice'] @ [cat_food]->['wrong-order']");
    assert!(pangine.reference_concept("$['wrong-order']").unwrap().is_none());
}

#[test]
fn question_outputs_replace_roots_and_clear_when_unbound() {
    let mut pangine = Pangine::new();
    let output = pangine.reference_percept("X");

    must_ref(&mut pangine, "['X'] ~= [old-a]");
    must_ref(&mut pangine, "['X'] ~= [old-b]");
    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    ask(&mut pangine, "['memory'] @ {['X']->[A]}");
    assert_eq!(pangine.get_percept_roots(&output).unwrap().len(), 1);

    must_ref(&mut pangine, "['memory'] = [atomic]");
    ask(&mut pangine, "['memory'] @ {['X']->[missing]}");
    assert!(pangine.get_percept_roots(&output).unwrap().is_empty());
    assert!(pangine.reference_concept("$['X']").unwrap().is_none());
}

#[test]
fn a_selected_source_can_be_replaced_by_its_own_question_output() {
    let mut pangine = Pangine::new();
    let alice = pangine.reference_percept("Alice");

    must_ref(&mut pangine, "['Alice'] ~= [A]");
    must_ref(&mut pangine, "['Alice'] ~= [B]");
    assert_eq!(pangine.get_percept_roots(&alice).unwrap().len(), 2);

    ask(&mut pangine, "['Alice'] @ ['Alice']");
    assert_eq!(pangine.get_percept_roots(&alice).unwrap().len(), 1);
}

#[test]
fn standalone_wildcard_question_can_bind_an_atomic_root() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]");
    ask(&mut pangine, "['memory'] @ ['X']");
    assert_eq!(must_ref(&mut pangine, "$['X']"), must_ref(&mut pangine, "[A]"));
}

#[test]
fn unequal_union_question_binds_the_exact_remainder_above_its_parts() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]*[B]*[C]");
    ask(&mut pangine, "['memory'] @ ['X']*[B]");

    assert_eq!(must_ref(&mut pangine, "$['X']"), must_ref(&mut pangine, "[A]*[C]"));
}

#[test]
fn unequal_union_question_does_not_bind_a_generic_mismatch() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]*[B]*[C]");
    must_ref(&mut pangine, "['X'] = [old]");
    ask(&mut pangine, "['memory'] @ ['X']*[D]");
    assert!(pangine.reference_concept("$['X']").unwrap().is_none());
}

#[test]
fn deep_experience_keeps_one_root_while_question_derives_nested_matches() {
    let mut pangine = Pangine::new();
    let memory = pangine.reference_percept("memory");
    let depth = 20;
    let mut experience = format!("[N{depth}]");
    for index in (0..depth).rev() {
        experience = format!("[N{index}]->({experience})");
    }

    let root = must_ref(&mut pangine, &format!("['memory'] ~= {experience}"));
    assert_eq!(pangine.get_percept_roots(&memory), Some(vec![root]));

    ask(&mut pangine, &format!("['memory'] @ [N{depth_minus_one}]->['tail']", depth_minus_one = depth - 1));
    assert_eq!(must_ref(&mut pangine, "^['tail']"), must_ref(&mut pangine, &format!("[N{depth}]")));
}

#[test]
fn explicit_cyclic_query_graph_terminates() {
    let mut pangine = Pangine::new();

    for root in ["[A]->[link]->[B]", "[B]->[link]->[C]", "[C]->[link]->[A]", "[C]->[sound]->[answer]"] {
        must_ref(&mut pangine, &format!("['memory'] ~= {root}"));
    }

    assert!(pangine.reference_concept("['memory'] @ ([A]->[link]->['one'])(['one']->[link]->['two'])(['two']->[link]->[A])").unwrap().is_some());
}

#[test]
fn wide_question_remains_finite_and_finds_the_expected_answer() {
    let mut pangine = Pangine::new();
    let width = 20;
    let experience = (0..width).map(|index| format!("{{[V{index}]->[K{index}]}}")).collect::<Vec<_>>().join("*");
    let question =
        (0..width).map(|index| if index == 0 { "{['X']->[K0]}".to_owned() } else { format!("{{[V{index}]->[K{index}]}}") }).collect::<Vec<_>>().join("*");

    must_ref(&mut pangine, &format!("['memory'] ~= {experience}"));
    ask(&mut pangine, &format!("['memory'] @ {question}"));

    let candidates = named_value(&mut pangine, "$['X']");
    assert_eq!(candidates[0].1, "V0");
    assert!(candidates.iter().all(|(relevance, _)| relevance.weight().is_finite()));
}

fn named_relevance(pangine: &Pangine, concept: &ConceptId) -> Vec<(Relevance, String)> {
    pangine
        .get_relevance_map(concept)
        .into_iter()
        .map(|(relevance, concept)| {
            let name = pangine.get_name(&concept).unwrap_or_else(|| panic!("expected named candidate, got {concept:?}"));
            (relevance, name.to_owned())
        })
        .collect()
}

fn named_value(pangine: &mut Pangine, input: &str) -> Vec<(Relevance, String)> {
    let concept = must_ref(pangine, input);
    named_relevance(pangine, &concept)
}

fn candidate_weight(candidates: &[(Relevance, String)], name: &str) -> f32 {
    candidates.iter().find_map(|(relevance, candidate)| (candidate == name).then(|| relevance.weight())).unwrap_or_else(|| panic!("missing candidate {name:?}"))
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}

fn ask(pangine: &mut Pangine, input: &str) {
    pangine.reference_concept(input).unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"));
}
