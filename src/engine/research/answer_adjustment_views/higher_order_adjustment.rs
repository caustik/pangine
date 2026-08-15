//! Research probes for composing adjusted answers through several layers.

use super::*;

#[test]
#[ignore = "warning: answer adjustment composition does not establish general higher-order evidence semantics"]
fn reliability_can_adjust_an_outcome_answer_before_that_answer_adjusts_a_decision() {
    let mut pangine = layered_decision_answers();
    let decisions = linked_view(&mut pangine, "['decision']");
    let outcomes = linked_view(&mut pangine, "['episode']");
    let trusted = linked_view(&mut pangine, "['trusted-episode']");

    let trusted_outcomes = outcomes.adjust(&mut pangine, &trusted, Relevance::DEFAULT).expect("episode views match");
    assert_eq!(adjustment_counts(&trusted_outcomes), (2, 1, 1, 1, 1, 1, 1));
    let trusted_outcomes = trusted_outcomes.answer;
    assert_projection(&mut pangine, &trusted_outcomes, "['episode']", "x2[episode-b][episode-a]");
    assert_projection(&mut pangine, &trusted_outcomes, "['episode-decision']", "x2[B][A]");

    let episode_decision = must_ref(&mut pangine, "['episode-decision']");
    let trusted_decisions = trusted_outcomes.projecting(&pangine, episode_decision).expect("decision projection");
    let adjusted = decisions.adjust(&mut pangine, &trusted_decisions, Relevance::DEFAULT).expect("decision values match");
    assert_eq!(adjustment_counts(&adjusted), (2, 2, 2, 2, 2, 2, 3));
    let adjusted = adjusted.answer;

    assert_projection(&mut pangine, &adjusted, "['candidate']", "x3[choice-b]x2[choice-a]");
    assert_eq!(choose_value(&mut pangine, &adjusted), Some(must_ref(&mut pangine, "[B]")));
    assert_eq!(
        identified_source_contributions(&mut pangine, &adjusted, "['decision']", "[A]", &["choice-a", "episode-a", "review-b"]),
        BTreeMap::from([("choice-a".to_owned(), BTreeSet::from([(1, 1)])), ("episode-a".to_owned(), BTreeSet::from([(1, 1)]))])
    );
    assert_eq!(
        identified_source_contributions(&mut pangine, &adjusted, "['decision']", "[B]", &["choice-b", "episode-b", "review-b"]),
        BTreeMap::from([
            ("choice-b".to_owned(), BTreeSet::from([(1, 1)])),
            ("episode-b".to_owned(), BTreeSet::from([(1, 1)])),
            ("review-b".to_owned(), BTreeSet::from([(1, 1)])),
        ])
    );
    assert_eq!(adjusted.answer.shape(), decisions.answer.shape());
    let episode = pangine.reference_percept("episode");
    let trusted_episode = pangine.reference_percept("trusted-episode");
    assert!(adjusted.projecting(&pangine, episode).is_none());
    assert!(adjusted.projecting(&pangine, trusted_episode).is_none());

    assert_projection(&mut pangine, &decisions, "['candidate']", "[choice-a][choice-b]");
    assert_projection(&mut pangine, &outcomes, "['episode-decision']", "[A][B]");
}

#[test]
#[ignore = "warning: higher-order adjustment order and replay are still only a research contract"]
fn layered_adjustment_is_an_explicit_snapshot_sequence_instead_of_a_reactive_dependency() {
    let mut pangine = layered_decision_answers();
    let decisions = linked_view(&mut pangine, "['decision']");
    let outcomes = linked_view(&mut pangine, "['episode-decision']");
    let episodes = linked_view(&mut pangine, "['episode']");
    let trusted = linked_view(&mut pangine, "['trusted-episode']");

    let decisions_before_trust = decisions.adjusted_by(&mut pangine, &outcomes, Relevance::DEFAULT).expect("raw outcome adjustment");
    assert_projection(&mut pangine, &decisions_before_trust, "['candidate']", "x2[choice-a]x2[choice-b]");

    let trusted_outcomes = episodes.adjusted_by(&mut pangine, &trusted, Relevance::DEFAULT).expect("trust adjustment");
    let episode_decision = must_ref(&mut pangine, "['episode-decision']");
    let trusted_outcomes = trusted_outcomes.projecting(&pangine, episode_decision).expect("decision projection");
    assert_projection(&mut pangine, &trusted_outcomes, "['episode-decision']", "x2[B][A]");
    assert_projection(&mut pangine, &decisions_before_trust, "['candidate']", "x2[choice-a]x2[choice-b]");

    let replayed = decisions_before_trust.adjust(&mut pangine, &trusted_outcomes, Relevance::DEFAULT).expect("explicit replay");
    assert_eq!(adjustment_counts(&replayed), (2, 2, 2, 2, 2, 1, 1));
    assert_projection(&mut pangine, &replayed.answer, "['candidate']", "x3[choice-b]x2[choice-a]");

    let nested = decisions.adjusted_by(&mut pangine, &trusted_outcomes, Relevance::DEFAULT).expect("nested adjustment");
    assert!(same_completion_result(&replayed.answer.answer.result, &nested.answer.result));
}

#[test]
#[ignore = "warning: the proposed console operator demonstrates only sequential live higher-order composition"]
fn explicit_answer_adjustment_can_publish_a_three_layer_console_chain() {
    let mut pangine = layered_decision_answers();
    let candidate = pangine.reference_percept("candidate");
    let episode = pangine.reference_percept("episode");
    let trusted_episode = pangine.reference_percept("trusted-episode");
    let candidate_before = pangine.shared_answer_state(&candidate).expect("candidate answer");
    let episode_before = pangine.shared_answer_state(&episode).expect("outcome answer");
    let trusted_before = pangine.shared_answer_state(&trusted_episode).expect("reliability answer");

    assert_eq!(
        reference_research_adjustment(&mut pangine, "['episode'] @+= ['trusted-episode']").expect("valid trust adjustment"),
        Some(must_ref(&mut pangine, "x2[episode-b][episode-a]"))
    );
    let episode_after = pangine.shared_answer_state(&episode).expect("adjusted outcome answer");
    assert_ne!(episode_after, episode_before);
    assert_eq!(pangine.shared_answer_state(&candidate), Some(candidate_before));
    assert_eq!(pangine.shared_answer_state(&trusted_episode), Some(trusted_before));

    assert_eq!(
        reference_research_adjustment(&mut pangine, "['decision'] @+= ['episode-decision']").expect("valid decision adjustment"),
        Some(must_ref(&mut pangine, "x3[B]x2[A]"))
    );
    assert_ne!(pangine.shared_answer_state(&candidate), Some(candidate_before));
    assert_eq!(pangine.shared_answer_state(&episode), Some(episode_after));
    assert_eq!(pangine.shared_answer_state(&trusted_episode), Some(trusted_before));
    assert_eq!(must_ref(&mut pangine, "$['candidate']"), must_ref(&mut pangine, "x3[choice-b]x2[choice-a]"));
    assert_eq!(must_ref(&mut pangine, "^['decision']"), must_ref(&mut pangine, "[B]"));
    assert_eq!(must_ref(&mut pangine, "$['candidate']"), must_ref(&mut pangine, "x3[choice-b]"));
}

#[test]
#[ignore = "warning: nested adjustment currently flattens every retained source into the outer answer"]
fn an_outer_adjustment_transforms_each_retained_leaf_source_instead_of_one_intermediate_total() {
    let mut pangine = layered_decision_answers();
    let decisions = linked_view(&mut pangine, "['decision']");
    let episodes = linked_view(&mut pangine, "['episode']");
    let trusted = linked_view(&mut pangine, "['trusted-episode']");

    let trusted_outcomes = episodes.adjusted_by(&mut pangine, &trusted, Relevance::DEFAULT).expect("trust adjustment");
    let episode_decision = must_ref(&mut pangine, "['episode-decision']");
    let trusted_outcomes = trusted_outcomes.projecting(&pangine, episode_decision).expect("decision projection");
    let discouraged = decisions.adjusted_by(&mut pangine, &trusted_outcomes, Relevance::new(-1)).expect("negative outer adjustment");

    assert_projection(&mut pangine, &discouraged, "['candidate']", "![choice-b]");
    assert_eq!(
        identified_source_contributions(&mut pangine, &discouraged, "['decision']", "[B]", &["choice-b", "episode-b", "review-b"]),
        BTreeMap::from([
            ("choice-b".to_owned(), BTreeSet::from([(1, 1)])),
            ("episode-b".to_owned(), BTreeSet::from([(1, -1)])),
            ("review-b".to_owned(), BTreeSet::from([(1, -1)])),
        ])
    );
}

#[test]
#[ignore = "warning: source identity rather than path count is still a provisional higher-order rule"]
fn one_reliability_source_reaching_a_decision_through_two_episodes_counts_once() {
    let mut pangine = converging_layered_answers();
    let decisions = linked_view(&mut pangine, "['decision']");
    let episodes = linked_view(&mut pangine, "['episode']");
    let trusted = linked_view(&mut pangine, "['trusted-episode']");

    assert_projection(&mut pangine, &episodes, "['episode-decision']", "x2[B]");
    assert_projection(&mut pangine, &trusted, "['trusted-episode']", "[episode-b-1][episode-b-2]");

    let trusted_outcomes = episodes.adjust(&mut pangine, &trusted, Relevance::DEFAULT).expect("two trusted episode paths");
    assert_eq!(adjustment_counts(&trusted_outcomes), (2, 2, 2, 2, 2, 2, 2));
    let trusted_outcomes = trusted_outcomes.answer;
    assert_projection(&mut pangine, &trusted_outcomes, "['episode-decision']", "x3[B]");

    let episode_decision = must_ref(&mut pangine, "['episode-decision']");
    let trusted_decisions = trusted_outcomes.projecting(&pangine, episode_decision).expect("decision projection");
    let adjusted = decisions.adjust(&mut pangine, &trusted_decisions, Relevance::DEFAULT).expect("converging decision paths");
    assert_eq!(adjustment_counts(&adjusted), (1, 2, 1, 2, 2, 1, 3));
    let adjusted = adjusted.answer;

    assert_projection(&mut pangine, &adjusted, "['candidate']", "x4[choice-b]");
    assert_eq!(
        identified_source_contributions(&mut pangine, &adjusted, "['decision']", "[B]", &["choice-b", "episode-b-1", "episode-b-2", "review-b"],),
        BTreeMap::from([
            ("choice-b".to_owned(), BTreeSet::from([(1, 1)])),
            ("episode-b-1".to_owned(), BTreeSet::from([(1, 1)])),
            ("episode-b-2".to_owned(), BTreeSet::from([(1, 1)])),
            ("review-b".to_owned(), BTreeSet::from([(1, 1)])),
        ])
    );
}

#[test]
#[ignore = "warning: cyclic answer adjustment is a sequence of snapshots, not an accepted reactive system"]
fn repeated_positive_adjustment_reaches_a_source_fixed_point_even_across_a_cycle() {
    let mut pangine = mutually_adjustable_answers();
    let left = linked_view(&mut pangine, "['left-decision']");
    let right = linked_view(&mut pangine, "['right-decision']");

    let self_adjusted = left.adjust(&mut pangine, &left, Relevance::DEFAULT).expect("positive self adjustment");
    assert_eq!(adjustment_counts(&self_adjusted), (1, 1, 1, 1, 1, 0, 0));
    assert_projection(&mut pangine, &self_adjusted.answer, "['left-decision']", "[A]");

    let left_with_right = left.adjust(&mut pangine, &right, Relevance::DEFAULT).expect("right into left");
    assert_eq!(adjustment_counts(&left_with_right), (1, 1, 1, 1, 1, 1, 1));
    let left_with_right = left_with_right.answer;
    assert_projection(&mut pangine, &left_with_right, "['left-decision']", "x2[A]");

    let right_with_left = right.adjust(&mut pangine, &left_with_right, Relevance::DEFAULT).expect("left into right");
    assert_eq!(adjustment_counts(&right_with_left), (1, 1, 1, 1, 1, 1, 1));
    let right_with_left = right_with_left.answer;
    assert_projection(&mut pangine, &right_with_left, "['right-decision']", "x2[A]");

    let closed = left_with_right.adjust(&mut pangine, &right_with_left, Relevance::DEFAULT).expect("closed source set");
    assert_eq!(adjustment_counts(&closed), (1, 1, 1, 1, 1, 0, 0));
    assert_projection(&mut pangine, &closed.answer, "['left-decision']", "x2[A]");
    assert!(same_completion_result(&left_with_right.answer.result, &closed.answer.answer.result));
    assert_eq!(
        identified_source_contributions(&mut pangine, &closed.answer, "['left-decision']", "[A]", &["left-record", "right-record"]),
        BTreeMap::from([("left-record".to_owned(), BTreeSet::from([(1, 1)])), ("right-record".to_owned(), BTreeSet::from([(1, 1)])),])
    );
    assert_eq!(
        (
            sole_completion_evidence_count(&left),
            sole_completion_evidence_count(&left_with_right),
            sole_completion_evidence_count(&right_with_left),
            sole_completion_evidence_count(&closed.answer),
        ),
        (1, 2, 2, 2)
    );

    let right_closed = right_with_left.adjust(&mut pangine, &closed.answer, Relevance::DEFAULT).expect("right provenance closure");
    assert_eq!(adjustment_counts(&right_closed), (1, 1, 1, 1, 1, 0, 0));
    assert!(same_completion_result(&right_with_left.answer.result, &right_closed.answer.answer.result));
    let left_closed_again = closed.answer.adjust(&mut pangine, &right_closed.answer, Relevance::DEFAULT).expect("left provenance closure");
    assert_eq!(adjustment_counts(&left_closed_again), (1, 1, 1, 1, 1, 0, 0));
    assert!(same_completion_result(&closed.answer.answer.result, &left_closed_again.answer.answer.result));
}

#[test]
#[ignore = "warning: alternate higher-order branches still have no console value form"]
fn one_outcome_snapshot_can_branch_through_opposite_reliability_adjustments() {
    let mut pangine = layered_decision_answers();
    let decisions = linked_view(&mut pangine, "['decision']");
    let episodes = linked_view(&mut pangine, "['episode']");
    let trusted = linked_view(&mut pangine, "['trusted-episode']");
    let episode_decision = must_ref(&mut pangine, "['episode-decision']");

    let strengthened = episodes.adjusted_by(&mut pangine, &trusted, Relevance::DEFAULT).expect("positive reliability branch");
    let strengthened = strengthened.projecting(&pangine, episode_decision.clone()).expect("decision projection");
    let strengthened = decisions.adjusted_by(&mut pangine, &strengthened, Relevance::DEFAULT).expect("strengthened decisions");
    assert_projection(&mut pangine, &strengthened, "['candidate']", "x3[choice-b]x2[choice-a]");
    assert_eq!(choose_value(&mut pangine, &strengthened), Some(must_ref(&mut pangine, "[B]")));

    let weakened = episodes.adjusted_by(&mut pangine, &trusted, Relevance::new(-1)).expect("negative reliability branch");
    assert_projection(&mut pangine, &weakened, "['episode-decision']", "[A]");
    let weakened = weakened.projecting(&pangine, episode_decision).expect("decision projection");
    let weakened = decisions.adjusted_by(&mut pangine, &weakened, Relevance::DEFAULT).expect("weakened decisions");
    assert_projection(&mut pangine, &weakened, "['candidate']", "x2[choice-a][choice-b]");
    assert_eq!(choose_value(&mut pangine, &weakened), Some(must_ref(&mut pangine, "[A]")));

    assert_projection(&mut pangine, &decisions, "['candidate']", "[choice-a][choice-b]");
    assert_projection(&mut pangine, &episodes, "['episode-decision']", "[A][B]");
}

#[test]
#[ignore = "warning: opposite paths from one source currently remain as cancelling signed witnesses"]
fn one_source_can_reach_a_later_answer_with_both_signs_without_losing_either_path() {
    let mut pangine = layered_decision_answers();
    let decisions = linked_view(&mut pangine, "['decision']");
    let episodes = linked_view(&mut pangine, "['episode']");
    let trusted = linked_view(&mut pangine, "['trusted-episode']");

    let strengthened = episodes.adjusted_by(&mut pangine, &trusted, Relevance::DEFAULT).expect("positive trust path");
    let neutralized = strengthened.adjust(&mut pangine, &trusted, Relevance::new(-1)).expect("negative trust path");
    assert_eq!(adjustment_counts(&neutralized), (2, 1, 1, 1, 1, 1, 1));
    let neutralized = neutralized.answer;
    assert_projection(&mut pangine, &neutralized, "['episode-decision']", "[A][B]");
    assert_eq!(
        identified_source_contributions(&mut pangine, &neutralized, "['episode']", "[episode-b]", &["episode-b", "review-b"]),
        BTreeMap::from([("episode-b".to_owned(), BTreeSet::from([(1, 1)])), ("review-b".to_owned(), BTreeSet::from([(1, -1), (1, 1)])),])
    );

    let episode_decision = must_ref(&mut pangine, "['episode-decision']");
    let neutralized = neutralized.projecting(&pangine, episode_decision).expect("decision projection");
    let adjusted = decisions.adjusted_by(&mut pangine, &neutralized, Relevance::DEFAULT).expect("neutralized outcome adjustment");
    assert_projection(&mut pangine, &adjusted, "['candidate']", "x2[choice-a]x2[choice-b]");
    assert_eq!(
        identified_source_contributions(&mut pangine, &adjusted, "['decision']", "[B]", &["choice-b", "episode-b", "review-b"]),
        BTreeMap::from([
            ("choice-b".to_owned(), BTreeSet::from([(1, 1)])),
            ("episode-b".to_owned(), BTreeSet::from([(1, 1)])),
            ("review-b".to_owned(), BTreeSet::from([(1, -1), (1, 1)])),
        ])
    );
}

#[test]
#[ignore = "warning: choosing which intermediate evidence reaches a later answer is still provisional"]
fn an_intermediate_choice_can_condition_the_evidence_used_by_a_later_decision() {
    let mut pangine = layered_decision_answers();
    let decisions = linked_view(&mut pangine, "['decision']");
    let episodes = linked_view(&mut pangine, "['episode']");
    let trusted = linked_view(&mut pangine, "['trusted-episode']");

    let trusted_outcomes = episodes.adjusted_by(&mut pangine, &trusted, Relevance::DEFAULT).expect("trust adjustment");
    let selected_episode = trusted_outcomes.choose(&mut pangine).expect("strongest episode");
    assert_eq!(selected_episode.selected, must_ref(&mut pangine, "[episode-b]"));
    assert_projection(&mut pangine, &selected_episode.answer, "['episode-decision']", "x2[B]");

    let episode_decision = must_ref(&mut pangine, "['episode-decision']");
    let selected_decision = selected_episode.answer.projecting(&pangine, episode_decision).expect("selected decision projection");
    let adjusted = decisions.adjusted_by(&mut pangine, &selected_decision, Relevance::DEFAULT).expect("selected outcome adjustment");
    assert_projection(&mut pangine, &adjusted, "['candidate']", "x3[choice-b][choice-a]");
    assert_eq!(choose_value(&mut pangine, &adjusted), Some(must_ref(&mut pangine, "[B]")));

    assert_projection(&mut pangine, &decisions, "['candidate']", "[choice-a][choice-b]");
    assert_projection(&mut pangine, &episodes, "['episode']", "[episode-a][episode-b]");
}

#[test]
#[ignore = "warning: flattened source contributions do not retain the sequence of adjustment factors"]
fn nested_adjustment_factors_collapse_to_the_same_final_leaf_contributions() {
    let mut pangine = layered_decision_answers();
    let decisions = linked_view(&mut pangine, "['decision']");
    let outcomes = linked_view(&mut pangine, "['episode-decision']");

    let direct_positive = decisions.adjusted_by(&mut pangine, &outcomes, Relevance::DEFAULT).expect("positive outcome adjustment");
    assert_projection(&mut pangine, &direct_positive, "['candidate']", "x2[choice-a]x2[choice-b]");

    let outcome_source = pangine.reference_percept("outcomes");
    let outcome_question = outcomes.answer.result.question().clone();
    let negative_result = complete_weighted_sources(&mut pangine, &[(outcome_source, Relevance::new(-1))], &outcome_question);
    let episode_decision = must_ref(&mut pangine, "['episode-decision']");
    let negative_outcomes = ResearchAnswerView::from_result(&pangine, negative_result, episode_decision).expect("negative outcome view");
    assert_projection(&mut pangine, &negative_outcomes, "['episode-decision']", "![A]![B]");

    let double_negative = decisions.adjusted_by(&mut pangine, &negative_outcomes, Relevance::new(-1)).expect("second negative factor");
    assert_projection(&mut pangine, &double_negative, "['candidate']", "x2[choice-a]x2[choice-b]");
    assert!(same_completion_result(&direct_positive.answer.result, &double_negative.answer.result));
}

#[test]
#[ignore = "research detail: deep answer composition keeps source and adjustment context linear"]
fn an_eight_layer_chain_has_linear_source_and_adjustment_context() {
    const DEPTH: usize = 8;

    let mut pangine = Pangine::new();
    let mut views = Vec::new();
    let mut outputs = Vec::new();
    for index in 0..DEPTH {
        must_ref(&mut pangine, &format!("['memory-{index}'] ~= [record-{index}]->[value]->[A]"));
        must_ref(&mut pangine, &format!("['memory-{index}'] @ ['row-{index}']->[value]->['layer-{index}']"));
        views.push(linked_view(&mut pangine, &format!("['layer-{index}']")));
        outputs.push(pangine.reference_percept(&format!("row-{index}")));
        outputs.push(pangine.reference_percept(&format!("layer-{index}")));
    }

    let mut chain = views.pop().expect("deepest answer");
    while let Some(target) = views.pop() {
        chain = target.adjusted_by(&mut pangine, &chain, Relevance::DEFAULT).expect("matching layer values");
    }

    assert_projection(&mut pangine, &chain, "['layer-0']", "x8[A]");
    assert_eq!(sole_completion_evidence_count(&chain), DEPTH);
    let completion = &chain.answer.result.completions()[0];
    let source_binding_cells =
        completion.evidence().iter().map(|evidence| outputs.iter().filter(|output| evidence.binding(output).is_some()).count()).sum::<usize>();
    let adjusted_output_cells = completion.evidence().iter().map(|evidence| evidence.adjusted_outputs().count()).sum::<usize>();
    assert_eq!(source_binding_cells, DEPTH * 2);
    assert_eq!(adjusted_output_cells, (DEPTH - 1) * 2);
}
