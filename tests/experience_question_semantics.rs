use pangine::{ConceptId, ConceptKind, Pangine, Relevance};
use std::collections::BTreeSet;

#[test]
fn question_ranks_exact_correlation_above_generic_role_evidence() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    must_ref(&mut pangine, "['memory'] ~= {[B]->[D]}");
    ask_question(&mut pangine, "['memory'] @ {['X']->[A]}");

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    let c = candidates.iter().find(|(_, name)| name == "C").unwrap().0;
    let b = candidates.iter().find(|(_, name)| name == "B").unwrap().0;

    assert!(c.weight() > b.weight());
    assert!(b.weight() > 0.0);
}

#[test]
fn question_keeps_correlation_output_bindings_distinct() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}*{[B]->[D]}");
    let question = must_ref(&mut pangine, "['memory'] @ {['X']->[A]}*{[B]->['Y']}");
    assert_eq!(question, must_ref(&mut pangine, "{['X']->[A]}*{[B]->['Y']}"));

    let answer = must_ref(&mut pangine, "$['memory'] @ {['X']->[A]}*{[B]->['Y']}");
    assert_eq!(answer, must_ref(&mut pangine, "{x12[C]x4[B]->[A]}*{[B]->x12[D]x4[A]}",));

    let x = must_ref(&mut pangine, "$['X']");
    let x_candidates = named_relevance(&pangine, &x);
    assert_eq!(x_candidates[0].1, "C");
    assert_eq!(candidate_weight(&x_candidates, "C"), 12.0);
    assert_eq!(candidate_weight(&x_candidates, "B"), 4.0);

    let y = must_ref(&mut pangine, "$['Y']");
    let y_candidates = named_relevance(&pangine, &y);
    assert_eq!(y_candidates[0].1, "D");
    assert_eq!(candidate_weight(&y_candidates, "D"), 12.0);
    assert_eq!(candidate_weight(&y_candidates, "A"), 4.0);
}

#[test]
fn question_preserves_outer_correlation_context_across_unordered_children() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {([C]*[A])->([B]*[D])}");
    let answer = must_ref(&mut pangine, "$['memory'] @ {(['X']*[A])->([B]*['Y'])}");
    assert_eq!(answer, must_ref(&mut pangine, "{[A](x16[C]x8[A][B][D])->[B](x16[D]x8[B][A][C])}",));

    let x = must_ref(&mut pangine, "$['X']");
    let x_candidates = named_relevance(&pangine, &x);
    assert_eq!(candidate_weight(&x_candidates, "C"), 16.0);
    assert_eq!(candidate_weight(&x_candidates, "A"), 8.0);
    assert_eq!(candidate_weight(&x_candidates, "B"), 1.0);
    assert_eq!(candidate_weight(&x_candidates, "D"), 1.0);

    let y = must_ref(&mut pangine, "$['Y']");
    let y_candidates = named_relevance(&pangine, &y);
    assert_eq!(candidate_weight(&y_candidates, "D"), 16.0);
    assert_eq!(candidate_weight(&y_candidates, "B"), 8.0);
    assert_eq!(candidate_weight(&y_candidates, "A"), 1.0);
    assert_eq!(candidate_weight(&y_candidates, "C"), 1.0);
}

#[test]
fn evaluation_recursively_resolves_percepts_inside_a_shape() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['A'] = [resolved_a]");
    must_ref(&mut pangine, "['B'] = ['A']->[resolved_b]");

    assert_eq!(must_ref(&mut pangine, "$(?['B']:{[fixed]->['A']})"), must_ref(&mut pangine, "?({[resolved_a]->[resolved_b]}):({[fixed]->[resolved_a]})",));
}

#[test]
fn question_folds_recursive_wildcard_projections_into_context_score() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->{[A]->[Z]}}");
    must_ref(&mut pangine, "['memory'] ~= {[B]->{[D]->[Y]}}");
    ask_question(&mut pangine, "['memory'] @ {['X']->{[A]->[Z]}}");

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    let c = candidates.iter().find(|(_, name)| name == "C").unwrap().0;
    let b = candidates.iter().find(|(_, name)| name == "B").unwrap().0;

    assert_eq!(c.weight(), 5.0);
    assert_eq!(b.weight(), 2.0);
}

#[test]
fn question_folds_multiple_output_wildcards_into_separate_marginals() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    ask_question(&mut pangine, "['memory'] @ {['X']->['Y']}");

    let x = must_ref(&mut pangine, "$['X']");
    assert_eq!(candidate_weight(&named_relevance(&pangine, &x), "C"), 2.0);

    let y = must_ref(&mut pangine, "$['Y']");
    assert_eq!(candidate_weight(&named_relevance(&pangine, &y), "A"), 2.0);
}

#[test]
fn generic_projection_rule_applies_to_observations_too() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= ?[C]:[A]");
    must_ref(&mut pangine, "['memory'] ~= ?[B]:[D]");
    ask_question(&mut pangine, "['memory'] @ ?['X']:[A]");

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    assert_eq!(candidate_weight(&candidates, "C"), 2.0);
    assert_eq!(candidate_weight(&candidates, "B"), 1.0);
}

#[test]
fn repeated_experience_is_idempotent_in_folded_projection_evidence() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}");
    must_ref(&mut pangine, "['memory'] ~= {[B]->[D]}");
    ask_question(&mut pangine, "['memory'] @ {['X']->[A]}");

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    assert_eq!(candidate_weight(&candidates, "C"), 2.0);
    assert_eq!(candidate_weight(&candidates, "B"), 1.0);
}

#[test]
fn experience_set_replay_is_content_blind_and_idempotent() {
    let mut pangine = Pangine::new();

    let global_once = must_ref(&mut pangine, "['global'] ~= [rain]");
    let global_replay = must_ref(&mut pangine, "['global'] ~= [rain]");
    assert_eq!(global_once, global_replay);
    assert_eq!(global_replay, must_ref(&mut pangine, "?[]:[rain]"));

    let event_once = must_ref(&mut pangine, "['event'] ~= ?[event-1]:[rain]");
    let event_replay = must_ref(&mut pangine, "['event'] ~= ?[event-1]:[rain]");
    assert_eq!(event_once, event_replay);

    must_ref(&mut pangine, "['event'] ~= ?[event-1]:[snow]");
    let unequal = must_ref(&mut pangine, "['event'] ~= ?[event-1]:![rain]");
    let records = observation_records(&pangine, &unequal);
    assert_eq!(
        records,
        BTreeSet::from([
            must_ref(&mut pangine, "?[event-1]:[rain]"),
            must_ref(&mut pangine, "?[event-1]:[snow]"),
            must_ref(&mut pangine, "?[event-1]:![rain]"),
        ])
    );
}

#[test]
fn observation_state_has_explicit_identity_and_a_recursive_canonical_surface() {
    let mut pangine = Pangine::new();
    let state = must_ref(&mut pangine, "<?[event-2]:[B], ?[event-1]:[A]>");
    let records = observation_records(&pangine, &state);
    let event_1 = must_ref(&mut pangine, "?[event-1]:[A]");
    let event_2 = must_ref(&mut pangine, "?[event-2]:[B]");

    assert!(matches!(pangine.concept_kind(&state), Some(ConceptKind::ObservationSet)));
    assert_eq!(pangine.get_observations(&state), Some(vec![event_1.clone(), event_2.clone()]));
    assert_eq!(records, BTreeSet::from([event_1, event_2]));
    assert_eq!(pangine.format_concept(&state, false), "<?[event-1]:[A], ?[event-2]:[B]>");
    assert_eq!(pangine.get_relevance_map(&state), vec![(Relevance::DEFAULT, state.clone())]);

    let composition = must_ref(&mut pangine, "(?[event-1]:[A])(?[event-2]:[B])");
    assert!(matches!(pangine.concept_kind(&composition), Some(ConceptKind::Relevance)));
    assert!(pangine.get_observations(&composition).is_none());
    assert_ne!(state, composition);

    let weighted = must_ref(&mut pangine, "x2<?[event-1]:[A], ?[event-2]:[B]>");
    assert_eq!(pangine.format_concept(&weighted, false), "x2<?[event-1]:[A], ?[event-2]:[B]>");

    let as_observer = must_ref(&mut pangine, "?<?[event-1]:[A], ?[event-2]:[B]>:[C]");
    let as_payload = must_ref(&mut pangine, "?[outer]:<?[event-1]:[A], ?[event-2]:[B]>");
    for concept in [state, weighted, as_observer, as_payload] {
        let formatted = pangine.format_concept(&concept, false);
        assert_eq!(must_ref(&mut pangine, &formatted), concept);
    }
}

#[test]
fn collection_delimiters_do_not_dispatch_by_member_shape() {
    let mut pangine = Pangine::new();
    let singleton = must_ref(&mut pangine, "?[event-1]:[A]");

    assert_eq!(must_ref(&mut pangine, "<?[event-1]:[A]>"), singleton);
    assert!(pangine.reference_concept("<>").unwrap().is_none());
    for invalid in ["<[A]>", "<x2[A]>", "<?[event-1]:[A], [B]>", "{?[event-1]:[A], ?[]:[B]}", "(x2[A], [B])"] {
        assert!(pangine.reference_concept(invalid).is_err(), "expected invalid syntax: {invalid}");
    }

    let mixed_relevance = must_ref(&mut pangine, "(?[event-1]:[A])[B]");
    assert!(matches!(pangine.concept_kind(&mixed_relevance), Some(ConceptKind::Relevance)));
}

#[test]
fn experience_set_deduplicates_recursive_observations_without_inventing_shapes() {
    let mut pangine = Pangine::new();

    let a_root = must_ref(&mut pangine, "?[event-1]:[rain]*[A]");
    let b_root = must_ref(&mut pangine, "?[event-1]:[rain]*[B]");
    must_ref(&mut pangine, "['memory'] ~= ?[event-1]:[rain]*[A]");
    let state = must_ref(&mut pangine, "['memory'] ~= ?[event-1]:[rain]*[B]");
    let records = observation_records(&pangine, &state);

    assert_eq!(
        records,
        BTreeSet::from([
            a_root,
            b_root,
            must_ref(&mut pangine, "?[event-1]:[rain]"),
            must_ref(&mut pangine, "?[event-1]:[A]"),
            must_ref(&mut pangine, "?[event-1]:[B]"),
        ])
    );
    assert!(!records.contains(&must_ref(&mut pangine, "?[event-1]:[rain]*[A]*[B]")));

    must_ref(&mut pangine, "['nested'] ~= {(?[event-1]:[rain])->[A]}");
    let nested = must_ref(&mut pangine, "['nested'] ~= {(?[event-1]:[rain])->[B]}");
    let nested_records = observation_records(&pangine, &nested);
    assert_eq!(nested_records.len(), 5);
    assert!(nested_records.contains(&must_ref(&mut pangine, "?[event-1]:[rain]")));
}

#[test]
fn experience_set_preserves_structural_multiplicity() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]");
    let state = must_ref(&mut pangine, "['memory'] ~= x2[A]");
    let records = observation_records(&pangine, &state);

    assert_eq!(records, BTreeSet::from([must_ref(&mut pangine, "?[]:[A]"), must_ref(&mut pangine, "?[]:x2[A]")]));
}

#[test]
fn experience_set_is_order_and_partition_independent() {
    let mut pangine = Pangine::new();
    let forward = ["?[event-1]:[rain]*[A]", "?[event-1]:[rain]*[B]", "?[event-2]:[rain]", "[global]"];

    for experience in forward {
        must_ref(&mut pangine, &format!("['forward'] ~= {experience}"));
    }
    for experience in forward.into_iter().rev() {
        must_ref(&mut pangine, &format!("['reverse'] ~= {experience}"));
    }
    let forward_state = must_ref(&mut pangine, "$['forward']");
    assert_eq!(must_ref(&mut pangine, "$['reverse']"), forward_state);

    for experience in [forward[0], forward[2]] {
        must_ref(&mut pangine, &format!("['partition-a'] ~= {experience}"));
    }
    for experience in [forward[1], forward[3]] {
        must_ref(&mut pangine, &format!("['partition-b'] ~= {experience}"));
    }
    must_ref(&mut pangine, "['combined'] ~= $['partition-a']");
    let combined = must_ref(&mut pangine, "['combined'] ~= $['partition-b']");
    assert_eq!(combined, forward_state);
}

#[test]
fn global_observations_remain_queryable_without_leaking_the_wrapper_into_plain_questions() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]");
    ask_question(&mut pangine, "['memory'] @ ?[]:['scoped']");
    assert_eq!(must_ref(&mut pangine, "$['scoped']"), must_ref(&mut pangine, "[A]"));

    ask_question(&mut pangine, "['memory'] @ ['plain']");
    assert_eq!(must_ref(&mut pangine, "$['plain']"), must_ref(&mut pangine, "[A]"));
}

#[test]
fn distinct_partial_experience_can_induce_an_unseen_complete_answer() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= {[C]->[A]}*{[B]->[D]}");
    for partial in ["{[E]->[A]}*{[P1]->[Q1]}", "{[E]->[A]}*{[P2]->[Q2]}", "{[E]->[A]}*{[P3]->[Q3]}"] {
        must_ref(&mut pangine, &format!("['memory'] ~= {partial}"));
    }

    let unseen_complete = must_ref(&mut pangine, "{[E]->[A]}*{[B]->[D]}");
    let memory = must_ref(&mut pangine, "$['memory']");
    assert!(!pangine.get_observations(&memory).unwrap().iter().any(|record| pangine.get_observation(record).as_ref() == Some(&unseen_complete)));

    ask_question(&mut pangine, "['memory'] @ {['X']->[A]}*{[B]->[D]}");
    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    assert_eq!(pangine.format_concept(&result, false), "x14[E]x12[C]x3[B]x3[P1]x3[P2]x3[P3]");
    assert_eq!(candidate_weight(&candidates, "E"), 14.0);
    assert_eq!(candidate_weight(&candidates, "C"), 12.0);
    assert_eq!(must_ref(&mut pangine, "^['X']"), must_ref(&mut pangine, "[E]"));
}

#[test]
fn question_clears_an_output_when_no_projection_can_bind_it() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['X'] = [old]");
    must_ref(&mut pangine, "['memory'] ~= [A]");
    assert_eq!(must_ref(&mut pangine, "['memory'] @ {['X']->[B]}"), must_ref(&mut pangine, "{['X']->[B]}"));
    assert!(pangine.reference_concept("$['X']").unwrap().is_none());
}

#[test]
fn standalone_wildcard_question_can_bind_a_single_atomic_experience() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]");
    ask_question(&mut pangine, "['memory'] @ ['X']");

    assert_eq!(must_ref(&mut pangine, "$['X']"), must_ref(&mut pangine, "[A]"));
}

#[test]
fn unequal_union_question_binds_the_exact_remainder_above_its_parts() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]*[B]*[C]");
    ask_question(&mut pangine, "['memory'] @ ['X']*[B]");

    assert_eq!(must_ref(&mut pangine, "$['X']"), must_ref(&mut pangine, "x2([A]*[C])[A][C]",));
    assert_eq!(must_ref(&mut pangine, "^['X']"), must_ref(&mut pangine, "[A]*[C]"));
}

#[test]
fn unequal_union_question_does_not_bind_a_generic_mismatch_remainder() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= [A]*[B]*[C]");
    must_ref(&mut pangine, "['X'] = [old]");
    ask_question(&mut pangine, "['memory'] @ ['X']*[D]");

    assert!(pangine.reference_concept("$['X']").unwrap().is_none());
}

#[test]
fn unequal_union_remainder_defers_non_default_relevance() {
    let mut pangine = Pangine::new();

    must_ref(&mut pangine, "['memory'] ~= x2[A][B][C]");
    ask_question(&mut pangine, "['memory'] @ ['X']*[B]");

    assert!(pangine.reference_concept("$['X']").unwrap().is_none());
}

#[test]
fn experience_stores_linear_structure_instead_of_wildcard_closure() {
    let mut pangine = Pangine::new();
    let depth = 20;
    let mut experience = format!("[N{depth}]");
    for index in (0..depth).rev() {
        experience = format!("[N{index}]->({experience})");
    }

    let value = must_ref(&mut pangine, &format!("['memory'] ~= {experience}"));

    assert_eq!(pangine.get_observations(&value).unwrap().len(), depth * 2 + 1);
}

#[test]
fn experience_recurses_within_the_explicit_observer_scope() {
    let mut pangine = Pangine::new();
    let value = must_ref(&mut pangine, "['memory'] ~= ?({[A]->[B]}):([C][D])");
    let entries = pangine.get_observations(&value).unwrap();

    assert_eq!(entries.len(), 3);
    for expected in ["?({[A]->[B]}):([C][D])", "?({[A]->[B]}):[C]", "?({[A]->[B]}):[D]"] {
        let expected = must_ref(&mut pangine, expected);
        assert!(entries.contains(&expected));
    }
}

#[test]
fn wide_unordered_projection_uses_folded_matching() {
    let mut pangine = Pangine::new();
    let width = 20;
    let experience = (0..width).map(|index| format!("{{[V{index}]->[K{index}]}}")).collect::<Vec<_>>().join("*");
    let question =
        (0..width).map(|index| if index == 0 { "{['X']->[K0]}".to_owned() } else { format!("{{[V{index}]->[K{index}]}}") }).collect::<Vec<_>>().join("*");

    must_ref(&mut pangine, &format!("['memory'] ~= {experience}"));
    ask_question(&mut pangine, &format!("['memory'] @ {question}"));

    let result = must_ref(&mut pangine, "$['X']");
    let candidates = named_relevance(&pangine, &result);
    assert_eq!(candidates[0].1, "V0");
    assert!(candidates.iter().all(|(relevance, _)| relevance.weight().is_finite()));
}

fn candidate_weight(candidates: &[(Relevance, String)], name: &str) -> f32 {
    candidates.iter().find(|(_, candidate)| candidate == name).unwrap_or_else(|| panic!("missing candidate {name:?}")).0.weight()
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

fn observation_records(pangine: &Pangine, state: &ConceptId) -> BTreeSet<ConceptId> {
    pangine
        .get_observations(state)
        .unwrap_or_else(|| panic!("expected Observation state, got {state:?}"))
        .into_iter()
        .inspect(|record| {
            assert!(pangine.get_observation(record).is_some());
        })
        .collect()
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null concept for {input:?}"))
}

fn ask_question(pangine: &mut Pangine, input: &str) {
    must_ref(pangine, input);
}
