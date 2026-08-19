//! Research probes for adjusting explicit views of complete answers.
//!
//! The production Answer API accepts explicit projections of separate answers.
//! Any two projection templates can be instantiated over their respective rows
//! and compared as ordinary Concepts. The parser exposes the same operation as
//! `@+=` and `@-=`. The remaining probes cover deeper composition semantics.

use super::super::{
    Answer as ResearchAnswer, AnswerAdjustment as ResearchAdjustment, AnswerPublicationError as ResearchPublicationError, AnswerView as ResearchAnswerView,
    CompletionResult, ConceptId, Pangine, ParseError, QuestionSource,
};
use crate::Relevance;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

const BASE_QUESTION: &str = "
    (['candidate']->[problem]->[unresolved-symbol])
    (['candidate']->[action]->['action'])
    (['candidate']->[tool]->['tool'])";

const HELPFUL_QUESTION: &str = "
    (['helpful-episode']->[candidate]->['helpful-candidate'])
    (['helpful-episode']->[problem]->[unresolved-symbol])
    (['helpful-episode']->[action]->['helpful-action'])
    (['helpful-episode']->[tool]->['helpful-tool'])
    (['helpful-episode']->[outcome]->[helpful])";

const FAILED_QUESTION: &str = "
    (['failed-episode']->[candidate]->['failed-candidate'])
    (['failed-episode']->[problem]->[unresolved-symbol])
    (['failed-episode']->[action]->['failed-action'])
    (['failed-episode']->[tool]->['failed-tool'])
    (['failed-episode']->[outcome]->[failed])";

#[test]
#[ignore = "warning: helpful-minus-failed remains one explicit decision policy"]
fn explicit_answer_adjustment_runs_a_complete_console_decision_chain() {
    let mut pangine = troubleshooting_answers();
    let target = pangine.reference_percept("candidate");
    let helpful = pangine.reference_percept("helpful-action");
    let failed = pangine.reference_percept("failed-action");
    let target_before = pangine.shared_answer_revision(&target).expect("target answer");
    let helpful_before = pangine.shared_answer_revision(&helpful).expect("helpful answer");
    let failed_before = pangine.shared_answer_revision(&failed).expect("failed answer");

    let helpful_result = pangine
        .reference_concept("['action']->['tool'] @+= ['helpful-action']->['helpful-tool']")
        .expect("valid helpful adjustment")
        .expect("nonempty target projection");
    assert_eq!(
        helpful_result,
        must_ref(&mut pangine, "x2([inspect-symbols]->[dumpbin])([clean-build]->[cargo])([inspect-symbols]->[link-map])([reconfigure]->[cmake])",)
    );
    let target_after_helpful = pangine.shared_answer_revision(&target).expect("adjusted target answer");
    assert_ne!(target_after_helpful, target_before);
    assert_eq!(pangine.shared_answer_revision(&helpful), Some(helpful_before));
    assert_eq!(pangine.shared_answer_revision(&failed), Some(failed_before));

    let failed_result = pangine
        .reference_concept("['action']->['tool'] @-= ['failed-action']->['failed-tool']")
        .expect("valid failed adjustment")
        .expect("nonempty target projection");
    assert_eq!(
        failed_result,
        must_ref(&mut pangine, "x2([inspect-symbols]->[dumpbin])([inspect-symbols]->[link-map])([reconfigure]->[cmake])!([clean-build]->[cargo])",)
    );
    let target_after_failed = pangine.shared_answer_revision(&target).expect("adjusted target answer");
    assert_ne!(target_after_failed, target_after_helpful);
    assert_eq!(pangine.shared_answer_revision(&helpful), Some(helpful_before));
    assert_eq!(pangine.shared_answer_revision(&failed), Some(failed_before));

    assert_eq!(
        must_ref(&mut pangine, "$['candidate']"),
        must_ref(&mut pangine, "x2[candidate-dumpbin][candidate-map][candidate-reconfigure]![candidate-clean]")
    );
    assert_eq!(must_ref(&mut pangine, "&['action']"), must_ref(&mut pangine, BASE_QUESTION));
    assert_eq!(must_ref(&mut pangine, "^(['action']->['tool'])"), must_ref(&mut pangine, "[inspect-symbols]->[dumpbin]"));
    assert_eq!(must_ref(&mut pangine, "$['candidate']"), must_ref(&mut pangine, "x2[candidate-dumpbin]"));
}

#[test]
#[ignore = "warning: exact projection effects remain research evidence"]
fn explicit_answer_adjustment_uses_the_written_shape_instead_of_a_single_percept_rule() {
    let mut by_action = troubleshooting_answers();
    by_action.reference_concept("['action'] @+= ['helpful-action']").expect("valid action adjustment");
    assert_eq!(
        must_ref(&mut by_action, "$['candidate']"),
        must_ref(&mut by_action, "x2[candidate-dumpbin]x2[candidate-map][candidate-clean][candidate-reconfigure]")
    );

    let mut by_complete_value = troubleshooting_answers();
    by_complete_value.reference_concept("['action']->['tool'] @+= ['helpful-action']->['helpful-tool']").expect("valid complete adjustment");
    assert_eq!(
        must_ref(&mut by_complete_value, "$['candidate']"),
        must_ref(&mut by_complete_value, "x2[candidate-dumpbin][candidate-clean][candidate-map][candidate-reconfigure]")
    );

    let mut by_packaging_value = packaging_answers();
    by_packaging_value
        .reference_concept(
            "([action]->['action'])([tool]->['tool'])([scope]->['scope'])
         @+=
         ([action]->['useful-action'])([tool]->['useful-tool'])([scope]->['useful-scope'])",
        )
        .expect("valid unordered three-field adjustment");
    assert_eq!(
        must_ref(&mut by_packaging_value, "$['candidate']"),
        must_ref(&mut by_packaging_value, "x2[candidate-modes][candidate-architecture][candidate-reinstall][candidate-signature]")
    );
}

#[test]
#[ignore = "research detail: answer adjustment remains lexically separate from ordinary operations"]
fn explicit_answer_adjustment_coexists_with_addition_and_questions() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['ordinary'] = [A]");
    assert_eq!(must_ref(&mut pangine, "['ordinary'] += [B]"), must_ref(&mut pangine, "[A][B]"));

    must_ref(&mut pangine, "['memory'] ~= [cat]->[eats]->[fish]");
    assert_eq!(must_ref(&mut pangine, "['memory'] @ ['animal']->[eats]->['food']"), must_ref(&mut pangine, "[cat]->[eats]->[fish]"));
}

#[test]
#[ignore = "research detail: no-match publication and total cancellation advance the live answer"]
fn explicit_answer_adjustment_has_predictable_null_and_invalid_boundaries() {
    let mut pangine = troubleshooting_answers();
    let target = pangine.reference_percept("candidate");
    let initial_revision = pangine.shared_answer_revision(&target).expect("target answer");

    assert!(matches!(pangine.reference_concept("(['action'])(['helpful-tool']) @+= ['helpful-action']"), Err(ParseError::InvalidSyntax)));
    assert_eq!(pangine.shared_answer_revision(&target), Some(initial_revision));

    assert!(matches!(pangine.reference_concept("['action'] @+= [unlinked]"), Err(ParseError::InvalidSyntax)));
    assert_eq!(pangine.shared_answer_revision(&target), Some(initial_revision));

    assert!(matches!(pangine.reference_concept("['action'] @+="), Err(ParseError::InvalidSyntax)));
    assert_eq!(pangine.shared_answer_revision(&target), Some(initial_revision));

    let unchanged = pangine
        .reference_concept("['action']->['tool'] @+= ['helpful-candidate']")
        .expect("valid adjustment with no matching value")
        .expect("unchanged target projection");
    assert_eq!(
        unchanged,
        must_ref(&mut pangine, "([clean-build]->[cargo])([inspect-symbols]->[dumpbin])([inspect-symbols]->[link-map])([reconfigure]->[cmake])",)
    );
    let unchanged_revision = pangine.shared_answer_revision(&target).expect("published no-match answer");
    assert_ne!(unchanged_revision, initial_revision);

    assert!(pangine.reference_concept("['action']->['tool'] @-= ['action']->['tool']").expect("valid self-cancellation").is_none());
    assert!(pangine.reference_concept("$['action']").expect("valid read").is_none());
    assert_eq!(must_ref(&mut pangine, "&['action']"), must_ref(&mut pangine, BASE_QUESTION));
    assert_eq!(pangine.answer_snapshot(&target).expect("proof-bearing answer").result().completions().len(), 4);
}

mod higher_order_adjustment;

#[test]
#[ignore = "research detail: repeated source adjustment remains idempotent"]
fn complete_action_and_tool_views_adjust_separate_outputs_without_a_single_decision_percept() {
    let mut pangine = troubleshooting_answers();
    let base = linked_view(&mut pangine, "['action']->['tool']");
    let helpful = linked_view(&mut pangine, "['helpful-action']->['helpful-tool']");
    let failed = linked_view(&mut pangine, "['failed-action']->['failed-tool']");

    let helpful_adjustment = base.adjust(&mut pangine, &helpful, Relevance::DEFAULT).expect("compatible views");
    assert_eq!(adjustment_counts(&helpful_adjustment), (4, 2, 1, 1, 1, 1, 1));
    let repeated_helpful = helpful_adjustment.answer.adjust(&mut pangine, &helpful, Relevance::DEFAULT).expect("repeated view");
    assert_eq!(adjustment_counts(&repeated_helpful), (4, 2, 1, 1, 1, 0, 0));
    let failed_adjustment = helpful_adjustment.answer.adjust(&mut pangine, &failed, Relevance::new(-1)).expect("compatible views");
    assert_eq!(adjustment_counts(&failed_adjustment), (4, 2, 1, 2, 2, 1, 2));
    let adjusted = failed_adjustment.answer;

    assert_projection(
        &mut pangine,
        &adjusted,
        "['action']->['tool']",
        "x2([inspect-symbols]->[dumpbin])([inspect-symbols]->[link-map])([reconfigure]->[cmake])!([clean-build]->[cargo])",
    );
    assert_projection(&mut pangine, &adjusted, "['candidate']", "x2[candidate-dumpbin][candidate-map][candidate-reconfigure]![candidate-clean]");
    assert_eq!(choose_value(&mut pangine, &adjusted), Some(must_ref(&mut pangine, "[inspect-symbols]->[dumpbin]")));

    let inventory = source_inventory(&mut pangine, &adjusted);
    assert_eq!(inventory["{[clean-build]->[cargo]}"]["episode-clean-failed-1"], (1, -1));
    assert_eq!(inventory["{[clean-build]->[cargo]}"]["episode-clean-failed-2"], (1, -1));
    assert_eq!(inventory["{[inspect-symbols]->[dumpbin]}"]["episode-dumpbin-helpful"], (1, 1));
    assert_eq!(inventory["{[inspect-symbols]->[link-map]}"].len(), 1);
}

#[test]
#[ignore = "warning: helpful-minus-failed repetition remains one provisional decision policy"]
fn repeated_outcomes_rebuild_views_from_one_episode_log_and_change_the_next_choice() {
    let mut pangine = troubleshooting_answers();

    let first = adjusted_troubleshooting_view(&mut pangine);
    assert_eq!(choose_value(&mut pangine, &first), Some(must_ref(&mut pangine, "[inspect-symbols]->[dumpbin]")));

    remember_episode(&mut pangine, "episode-dumpbin-failed-1", "candidate-dumpbin", "inspect-symbols", "dumpbin", "failed");
    refresh_troubleshooting_outcomes(&mut pangine);
    let second = adjusted_troubleshooting_view(&mut pangine);
    assert_eq!(choose_value(&mut pangine, &second), Some(must_ref(&mut pangine, "[inspect-symbols]->[dumpbin]")));

    remember_episode(&mut pangine, "episode-dumpbin-failed-2", "candidate-dumpbin", "inspect-symbols", "dumpbin", "failed");
    refresh_troubleshooting_outcomes(&mut pangine);
    let third = adjusted_troubleshooting_view(&mut pangine);
    assert_eq!(choose_value(&mut pangine, &third), Some(must_ref(&mut pangine, "[inspect-symbols]->[link-map]")));

    let base = linked_view(&mut pangine, "['action']->['tool']");
    assert_projection(&mut pangine, &base, "['candidate']", "[candidate-clean][candidate-dumpbin][candidate-map][candidate-reconfigure]");
}

#[test]
#[ignore = "research detail: projection size changes which rows receive evidence"]
fn the_written_view_controls_whether_evidence_applies_to_one_complete_decision_or_a_shared_action() {
    let mut pangine = troubleshooting_answers();
    let base_complete = linked_view(&mut pangine, "['action']->['tool']");
    let helpful_complete = linked_view(&mut pangine, "['helpful-action']->['helpful-tool']");
    let base_action = linked_view(&mut pangine, "['action']");
    let helpful_action = linked_view(&mut pangine, "['helpful-action']");

    let by_action = base_action.adjust(&mut pangine, &helpful_action, Relevance::DEFAULT).expect("compatible views");
    assert_eq!(adjustment_counts(&by_action), (4, 2, 2, 1, 2, 2, 2));
    let by_action = by_action.answer;
    assert_projection(&mut pangine, &by_action, "['candidate']", "x2[candidate-dumpbin]x2[candidate-map][candidate-clean][candidate-reconfigure]");

    let by_complete_decision = base_complete.adjust(&mut pangine, &helpful_complete, Relevance::DEFAULT).expect("compatible views");
    assert_eq!(adjustment_counts(&by_complete_decision), (4, 2, 1, 1, 1, 1, 1));
    let by_complete_decision = by_complete_decision.answer;
    assert_projection(&mut pangine, &by_complete_decision, "['candidate']", "x2[candidate-dumpbin][candidate-clean][candidate-map][candidate-reconfigure]");

    let helpful_candidate = linked_view(&mut pangine, "['helpful-candidate']");
    let no_match = base_complete.adjust(&mut pangine, &helpful_candidate, Relevance::DEFAULT).expect("valid empty match");
    assert_eq!(adjustment_counts(&no_match), (4, 2, 0, 0, 0, 0, 0));
    assert_projection(&mut pangine, &no_match.answer, "['candidate']", "[candidate-clean][candidate-dumpbin][candidate-map][candidate-reconfigure]");

    assert_projection(&mut pangine, &base_complete, "['candidate']", "[candidate-clean][candidate-dumpbin][candidate-map][candidate-reconfigure]");
}

#[test]
#[ignore = "warning: Answer choice is production behavior but its language form remains open"]
fn choosing_a_view_returns_the_conditioned_answer_for_later_projection_and_choice() {
    let mut pangine = troubleshooting_answers();
    let base = linked_view(&mut pangine, "['action']");
    let helpful = linked_view(&mut pangine, "['helpful-action']");
    let adjusted = base.adjusted_by(&mut pangine, &helpful, Relevance::DEFAULT).expect("compatible views");

    let action_choice = adjusted.choose(&mut pangine).expect("positive action choice");
    assert_eq!(action_choice.selected, must_ref(&mut pangine, "[inspect-symbols]"));
    assert_projection(&mut pangine, &action_choice.answer, "['candidate']", "x2[candidate-dumpbin]x2[candidate-map]");
    assert_projection(&mut pangine, &action_choice.answer, "['tool']", "x2[dumpbin]x2[link-map]");

    let tool_projection = must_ref(&mut pangine, "['tool']");
    let tool = action_choice.answer.projecting(&pangine, tool_projection).expect("tool view");
    let tool_choice = tool.choose(&mut pangine).expect("positive tool choice");
    assert_eq!(tool_choice.selected, must_ref(&mut pangine, "[dumpbin]"));
    assert_projection(&mut pangine, &tool_choice.answer, "['candidate']", "x2[candidate-dumpbin]");

    assert_projection(&mut pangine, &adjusted, "['candidate']", "x2[candidate-dumpbin]x2[candidate-map][candidate-clean][candidate-reconfigure]");
    let live = linked_view(&mut pangine, "['candidate']");
    assert_projection(&mut pangine, &live, "['candidate']", "[candidate-clean][candidate-dumpbin][candidate-map][candidate-reconfigure]");
}

#[test]
#[ignore = "warning: immutable Answer branches are available in Rust but have no language form"]
fn one_answer_snapshot_can_compare_collapse_orders_and_a_simultaneous_choice() {
    let mut pangine = weighted_animal_answer();
    let base = linked_view(&mut pangine, "['animal']->['food']");
    let animal = must_ref(&mut pangine, "['animal']");
    let food = must_ref(&mut pangine, "['food']");

    let animal_view = base.projecting(&pangine, animal.clone()).expect("animal view");
    assert!(Rc::ptr_eq(&animal_view.answer.result, &base.answer.result));
    let animal_first = animal_view.choose(&mut pangine).expect("animal choice");
    assert!(!Rc::ptr_eq(&animal_first.answer.answer.result, &base.answer.result));
    assert_eq!(animal_first.selected, must_ref(&mut pangine, "[cat]"));
    let animal_first_food = animal_first.answer.projecting(&pangine, food.clone()).expect("conditioned food view");
    let animal_first_food = animal_first_food.choose(&mut pangine).expect("food choice after animal");
    assert_eq!(animal_first_food.selected, must_ref(&mut pangine, "[milk]"));

    let food_first = base.projecting(&pangine, food).expect("food view").choose(&mut pangine).expect("food choice");
    assert_eq!(food_first.selected, must_ref(&mut pangine, "[fish]"));
    let food_first_animal = food_first.answer.projecting(&pangine, animal).expect("conditioned animal view");
    let food_first_animal = food_first_animal.choose(&mut pangine).expect("animal choice after food");
    assert_eq!(food_first_animal.selected, must_ref(&mut pangine, "[dog]"));

    let together = base.choose(&mut pangine).expect("complete choice");
    assert_eq!(together.selected, must_ref(&mut pangine, "[dog]->[fish]"));
    assert_projection(&mut pangine, &together.answer, "['animal']->['food']", "x7([dog]->[fish])");

    assert_projection(&mut pangine, &base, "['animal']->['food']", "x3([cat]->[fish])x5([cat]->[milk])x7([dog]->[fish])");
    let live = linked_view(&mut pangine, "['animal']->['food']");
    assert_projection(&mut pangine, &live, "['animal']->['food']", "x3([cat]->[fish])x5([cat]->[milk])x7([dog]->[fish])");
}

#[test]
#[ignore = "warning: advancing every successful publication revision remains provisional"]
fn publishing_one_branch_advances_the_live_revision_and_rejects_a_stale_sibling() {
    let mut pangine = weighted_animal_answer();
    let base = linked_view(&mut pangine, "['animal']->['food']");
    let animal = must_ref(&mut pangine, "['animal']");
    let food = must_ref(&mut pangine, "['food']");
    let initial_revision = pangine.shared_answer_revision(&animal).expect("linked answer");

    let animal_first = base.projecting(&pangine, animal.clone()).expect("animal view").choose(&mut pangine).expect("animal choice");
    let food_first = base.projecting(&pangine, food.clone()).expect("food view").choose(&mut pangine).expect("food choice");
    assert_eq!(animal_first.selected, must_ref(&mut pangine, "[cat]"));
    assert_eq!(food_first.selected, must_ref(&mut pangine, "[fish]"));

    let published = animal_first.answer.answer.publish(&mut pangine).expect("current branch publication");
    assert_eq!(published.prior_revision, initial_revision);
    assert_ne!(published.revision, initial_revision);
    assert_eq!(pangine.shared_answer_revision(&animal), Some(published.revision));
    let live_animal = linked_view(&mut pangine, "['animal']->['food']");
    assert_projection(&mut pangine, &live_animal, "['animal']->['food']", "x3([cat]->[fish])x5([cat]->[milk])");

    assert_eq!(food_first.answer.answer.publish(&mut pangine).err(), Some(ResearchPublicationError::Stale));
    assert_eq!(animal_first.answer.answer.publish(&mut pangine).err(), Some(ResearchPublicationError::Stale));

    let food_after_animal = published.answer.view(&pangine, food).expect("current food view").choose(&mut pangine).expect("food choice after animal");
    assert_eq!(food_after_animal.selected, must_ref(&mut pangine, "[milk]"));
    let published_food = food_after_animal.answer.answer.publish(&mut pangine).expect("current descendant publication");
    assert_eq!(published_food.prior_revision, published.revision);
    assert_ne!(published_food.revision, published.revision);
    let live_food = linked_view(&mut pangine, "['animal']->['food']");
    assert_projection(&mut pangine, &live_food, "['animal']->['food']", "x5([cat]->[milk])");
}

#[test]
#[ignore = "research detail: direct live collapse now uses functional choice and a new answer revision"]
fn functional_choice_and_publication_match_direct_collapse_for_single_and_complete_views() {
    for projection in ["['animal']", "['food']", "['animal']->['food']"] {
        let mut functional = weighted_animal_answer();
        let base = linked_view(&mut functional, "['animal']->['food']");
        let view = must_ref(&mut functional, projection);
        let animal = functional.reference_percept("animal");
        let revision_before = functional.shared_answer_revision(&animal).expect("functional answer");
        let choice = base.projecting(&functional, view).expect("functional view").choose(&mut functional).expect("functional choice");
        let published = choice.answer.answer.publish(&mut functional).expect("functional publication");
        assert_eq!(published.prior_revision, revision_before);
        assert_ne!(published.revision, revision_before);
        let functional_live = linked_view(&mut functional, "['animal']->['food']");
        let functional_rows = project(&mut functional, &functional_live, "['animal']->['food']");
        let functional_rows = functional.format_concept(&functional_rows, false);

        let mut direct = weighted_animal_answer();
        let animal = direct.reference_percept("animal");
        let direct_revision_before = direct.shared_answer_revision(&animal).expect("direct answer");
        must_ref(&mut direct, &format!("^({projection})"));
        assert_ne!(direct.shared_answer_revision(&animal), Some(direct_revision_before));
        let direct_live = linked_view(&mut direct, "['animal']->['food']");
        let direct_rows = project(&mut direct, &direct_live, "['animal']->['food']");
        let direct_rows = direct.format_concept(&direct_rows, false);

        assert_eq!(functional_rows, direct_rows);
    }
}

#[test]
#[ignore = "warning: captured-source publication semantics remain provisional"]
fn publishing_an_adjusted_answer_revises_only_the_target_and_keeps_its_captured_sources() {
    let mut pangine = troubleshooting_answers();
    let base = linked_view(&mut pangine, "['action']->['tool']");
    let helpful = linked_view(&mut pangine, "['helpful-action']->['helpful-tool']");
    let failed = linked_view(&mut pangine, "['failed-action']->['failed-tool']");
    let target_revision = pangine.shared_answer_revision(&base.projection).expect("target answer");
    let helpful_revision = pangine.shared_answer_revision(&helpful.projection).expect("helpful answer");
    let failed_revision = pangine.shared_answer_revision(&failed.projection).expect("failed answer");

    let adjusted = base.adjusted_by(&mut pangine, &helpful, Relevance::DEFAULT).expect("helpful adjustment");
    let adjusted = adjusted.adjusted_by(&mut pangine, &failed, Relevance::new(-1)).expect("failed adjustment");

    remember_episode(&mut pangine, "episode-map-helpful-later", "candidate-map", "inspect-symbols", "link-map", "helpful");
    must_ref(&mut pangine, &format!("['episodes'] @ {HELPFUL_QUESTION}"));
    let refreshed_helpful_revision = pangine.shared_answer_revision(&helpful.projection).expect("refreshed helpful answer");
    assert_ne!(refreshed_helpful_revision, helpful_revision);

    let published = adjusted.answer.publish(&mut pangine).expect("current adjusted answer");
    assert_eq!(published.prior_revision, target_revision);
    assert_ne!(published.revision, target_revision);
    assert_eq!(pangine.shared_answer_revision(&helpful.projection), Some(refreshed_helpful_revision));
    assert_eq!(pangine.shared_answer_revision(&failed.projection), Some(failed_revision));

    let live = linked_view(&mut pangine, "['action']->['tool']");
    assert_projection(&mut pangine, &live, "['candidate']", "x2[candidate-dumpbin][candidate-map][candidate-reconfigure]![candidate-clean]");
    assert_eq!(base.answer.publish(&mut pangine).err(), Some(ResearchPublicationError::Stale));
}

#[test]
#[ignore = "warning: publishing proof-bearing rows with no positive projection remains provisional"]
fn publication_keeps_the_answer_link_when_every_visible_projection_is_null() {
    let mut pangine = troubleshooting_answers();
    let base = linked_view(&mut pangine, "['action']->['tool']");
    let cancelled = base.adjusted_by(&mut pangine, &base, Relevance::new(-1)).expect("self-cancelled answer");

    let published = cancelled.answer.publish(&mut pangine).expect("current cancelled answer");
    assert!(pangine.reference_concept("$['action']").expect("valid read").is_none());
    assert!(pangine.reference_concept("$['tool']").expect("valid read").is_none());
    assert_eq!(must_ref(&mut pangine, "&['action']"), must_ref(&mut pangine, BASE_QUESTION));

    let candidate = pangine.reference_percept("candidate");
    let result = pangine.answer_snapshot(&candidate).expect("linked proof-bearing answer");
    assert_eq!(result.result().completions().len(), 4);
    assert_eq!(cancelled.answer.publish(&mut pangine).err(), Some(ResearchPublicationError::Stale));
    assert_eq!(published.answer.shape(), &must_ref(&mut pangine, BASE_QUESTION));
}

#[test]
#[ignore = "research detail: live collapse creates a new revision and stales prior branches"]
fn publication_detects_a_new_live_collapse_revision() {
    let mut pangine = weighted_animal_answer();
    let snapshot = linked_view(&mut pangine, "['animal']->['food']");
    let stale_branch = snapshot.choose(&mut pangine).expect("complete branch");
    let animal = must_ref(&mut pangine, "['animal']");
    let revision_before = pangine.shared_answer_revision(&animal).expect("linked answer");

    assert_eq!(must_ref(&mut pangine, "^['animal']"), must_ref(&mut pangine, "[cat]"));
    let revision_after = pangine.shared_answer_revision(&animal).expect("conditioned answer");
    assert_ne!(revision_after, revision_before);
    assert_eq!(stale_branch.answer.answer.publish(&mut pangine).err(), Some(ResearchPublicationError::Stale));

    assert_projection(&mut pangine, &snapshot, "['animal']->['food']", "x3([cat]->[fish])x5([cat]->[milk])x7([dog]->[fish])");
    let live = linked_view(&mut pangine, "['animal']->['food']");
    assert_projection(&mut pangine, &live, "['animal']->['food']", "x3([cat]->[fish])x5([cat]->[milk])");
}

#[test]
#[ignore = "warning: overlapping live questions and snapshot publication remain provisional"]
fn extending_a_live_answer_stales_older_branches_instead_of_erasing_the_new_shape() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['meals'] ~= [cat]->[eats]->[fish]");
    must_ref(&mut pangine, "['meals'] ~= [dog]->[eats]->[bone]");
    must_ref(&mut pangine, "['homes'] ~= [cat]->[lives-in]->[house]");
    must_ref(&mut pangine, "['homes'] ~= [dog]->[lives-in]->[yard]");
    must_ref(&mut pangine, "['meals'] @ ['animal']->[eats]->['food']");

    let original = linked_view(&mut pangine, "['animal']");
    let old_branch = original.choose(&mut pangine).expect("old animal branch");
    let old_revision = pangine.shared_answer_revision(&original.projection).expect("original answer");

    must_ref(&mut pangine, "['homes'] @ ['animal']->[lives-in]->['home']");
    let new_revision = pangine.shared_answer_revision(&original.projection).expect("extended answer");
    assert_ne!(new_revision, old_revision);
    let complete_shape = must_ref(&mut pangine, "(['animal']->[eats]->['food'])(['animal']->[lives-in]->['home'])");
    assert_eq!(must_ref(&mut pangine, "&['animal']"), complete_shape);

    assert_eq!(old_branch.answer.answer.publish(&mut pangine).err(), Some(ResearchPublicationError::Stale));
    assert_eq!(must_ref(&mut pangine, "&['food']"), complete_shape);
    assert_eq!(must_ref(&mut pangine, "&['home']"), complete_shape);

    let current = linked_view(&mut pangine, "['animal']");
    let current_branch = current.choose(&mut pangine).expect("current extended branch");
    current_branch.answer.answer.publish(&mut pangine).expect("extended answer publication");

    must_ref(&mut pangine, "['food'] = [detached]");
    let remaining_shape = must_ref(&mut pangine, "['animal']->[lives-in]->['home']");
    assert_eq!(must_ref(&mut pangine, "&['animal']"), remaining_shape);
    assert_eq!(must_ref(&mut pangine, "&['home']"), remaining_shape);
}

#[test]
#[ignore = "warning: the existing answer-shape Concept is a live selector, not a snapshot"]
fn the_visible_answer_shape_follows_live_state_while_an_answer_snapshot_does_not() {
    let mut pangine = weighted_animal_answer();
    let snapshot = linked_view(&mut pangine, "['animal']->['food']");
    let shape = must_ref(&mut pangine, "&['animal']");
    let before = pangine.evaluate_concept(&shape).expect("complete live answer");
    assert_eq!(before, must_ref(&mut pangine, "x3([cat]->[fish])x5([cat]->[milk])x7([dog]->[fish])"));

    must_ref(&mut pangine, "^['animal']");
    let after = pangine.evaluate_concept(&shape).expect("conditioned live answer");
    assert_eq!(after, must_ref(&mut pangine, "x3([cat]->[fish])x5([cat]->[milk])"));
    assert_projection(&mut pangine, &snapshot, "['animal']->['food']", "x3([cat]->[fish])x5([cat]->[milk])x7([dog]->[fish])");
}

#[test]
#[ignore = "warning: equal grounded Concepts can carry distinct answer evidence"]
fn an_answer_cannot_be_identified_only_by_its_shape_and_materialized_concept() {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['left-memory'] ~= [cat]->[fish]");
    must_ref(&mut pangine, "['right-memory'] ~= [cat]->[fish]");

    let left_rows = must_ref(&mut pangine, "['left-memory'] @ ['animal']->['food']");
    let left = linked_view(&mut pangine, "['animal']->['food']");
    let right_rows = must_ref(&mut pangine, "['right-memory'] @ ['animal']->['food']");
    let right = linked_view(&mut pangine, "['animal']->['food']");

    assert_eq!(left_rows, right_rows);
    assert_eq!(left.answer.shape(), right.answer.shape());
    assert_eq!(left.materialize(&mut pangine), right.materialize(&mut pangine));
    assert_eq!(sole_source_percept(&left), pangine.reference_percept("left-memory"));
    assert_eq!(sole_source_percept(&right), pangine.reference_percept("right-memory"));
    assert_eq!(left.answer.publish(&mut pangine).err(), Some(ResearchPublicationError::Stale));
    let published = right.answer.publish(&mut pangine).expect("current answer");
    assert_ne!(published.revision, published.prior_revision);
}

#[test]
#[ignore = "warning: a null materialized projection can still have proof-bearing answer rows"]
fn an_answer_can_keep_rows_and_shape_when_no_projected_candidate_has_positive_support() {
    let mut pangine = troubleshooting_answers();
    let base = linked_view(&mut pangine, "['action']->['tool']");
    let cancelled = base.adjust(&mut pangine, &base, Relevance::new(-1)).expect("self-cancellation view");
    assert_eq!(adjustment_counts(&cancelled), (4, 4, 4, 4, 4, 4, 4));

    assert_eq!(cancelled.answer.materialize(&mut pangine), None);
    assert!(cancelled.answer.choose(&mut pangine).is_none());
    assert_eq!(cancelled.answer.answer.result.completions().len(), 4);
    assert_eq!(cancelled.answer.answer.shape(), &must_ref(&mut pangine, BASE_QUESTION));

    assert_projection(&mut pangine, &base, "['candidate']", "[candidate-clean][candidate-dumpbin][candidate-map][candidate-reconfigure]");
}

#[test]
#[ignore = "warning: Answer snapshots are production Rust values but have no language form"]
fn an_answer_view_is_explicit_and_does_not_depend_on_later_percept_linkage() {
    let mut pangine = troubleshooting_answers();
    let base = linked_view(&mut pangine, "['action']->['tool']");
    let helpful = linked_view(&mut pangine, "['helpful-action']->['helpful-tool']");
    assert_eq!(base.answer.shape(), &must_ref(&mut pangine, BASE_QUESTION));

    let mixed_answers = must_ref(&mut pangine, "(['action'])(['helpful-tool'])");
    assert!(pangine.answer_view(&mixed_answers).is_none());

    let no_output = must_ref(&mut pangine, "[action]->[tool]");
    assert!(base.projecting(&pangine, no_output).is_none());

    let other_answer_output = must_ref(&mut pangine, "['helpful-action']");
    assert!(base.projecting(&pangine, other_answer_output).is_none());

    let mut other_pangine = Pangine::new();
    let foreign_projection = other_pangine.reference_percept("action");
    assert!(base.projecting(&other_pangine, foreign_projection).is_none());
    assert_eq!(base.materialize(&mut other_pangine), None);
    assert!(base.choose(&mut other_pangine).is_none());
    assert!(base.adjust(&mut other_pangine, &helpful, Relevance::DEFAULT).is_none());
    assert_eq!(base.answer.publish(&mut other_pangine).err(), Some(ResearchPublicationError::ForeignAnswer));
    let detached = ResearchAnswer::detached(base.answer.result.as_ref().clone());
    assert_eq!(detached.publish(&mut pangine).err(), Some(ResearchPublicationError::Detached));

    must_ref(&mut pangine, "['action'] = [manual-action]");
    assert!(pangine.answer_view(&base.projection).is_none());
    assert_eq!(base.answer.publish(&mut pangine).err(), Some(ResearchPublicationError::Stale));

    let current_tool = linked_view(&mut pangine, "['tool']");
    let action = must_ref(&mut pangine, "['action']");
    assert!(current_tool.projecting(&pangine, action.clone()).is_none());
    assert!(base.projecting(&pangine, action).is_some());
    assert_eq!(current_tool.answer.shape(), &must_ref(&mut pangine, "['candidate']['tool']"));

    let adjusted = base.adjusted_by(&mut pangine, &helpful, Relevance::DEFAULT).expect("captured views remain usable");
    assert_projection(&mut pangine, &adjusted, "['candidate']", "x2[candidate-dumpbin][candidate-clean][candidate-map][candidate-reconfigure]");
}

#[test]
#[ignore = "research detail: ordinary addition no longer guesses answer adjustment from linked Percepts"]
fn ordinary_addition_does_not_guess_answer_adjustment_from_linked_percepts() {
    let mut pangine = troubleshooting_answers();
    let target_complete = must_ref(&mut pangine, "['action']->['tool']");
    let helpful_complete = must_ref(&mut pangine, "['helpful-action']->['helpful-tool']");
    assert_eq!(pangine.perform_addition(&target_complete, Some(&helpful_complete)), None);
    assert_eq!(must_ref(&mut pangine, "$['candidate']"), must_ref(&mut pangine, "[candidate-clean][candidate-dumpbin][candidate-map][candidate-reconfigure]"));

    let target_action = pangine.reference_percept("action");
    let helpful_action = pangine.reference_percept("helpful-action");
    assert!(pangine.perform_addition(&target_action, Some(&helpful_action)).is_some());
    assert!(pangine.reference_concept("&['action']").expect("valid answer inspection").is_none());
    assert_eq!(must_ref(&mut pangine, "$['candidate']"), must_ref(&mut pangine, "[candidate-clean][candidate-dumpbin][candidate-map][candidate-reconfigure]"));
}

#[test]
#[ignore = "warning: weighted source selection is simpler only when outcome roles are already partitioned"]
fn weighted_sources_match_the_result_for_partitioned_logs_but_not_for_one_raw_episode_log() {
    let mut pangine = troubleshooting_answers();
    let candidates = pangine.reference_percept("candidates");
    let episodes = pangine.reference_percept("episodes");
    let helpful = pangine.reference_percept("helpful-episodes");
    let failed = pangine.reference_percept("failed-episodes");
    let question = must_ref(&mut pangine, "(['candidate']->[available]->[now])(['candidate']->[action]->['action'])(['candidate']->[tool]->['tool'])");
    let candidate_projection = must_ref(&mut pangine, "['candidate']");

    let partitioned = complete_weighted_sources(
        &mut pangine,
        &[(candidates.clone(), Relevance::DEFAULT), (helpful, Relevance::DEFAULT), (failed, Relevance::new(-1))],
        &question,
    );
    let partitioned = ResearchAnswerView::from_result(&pangine, partitioned, candidate_projection.clone()).expect("candidate view");
    assert_projection(&mut pangine, &partitioned, "['candidate']", "x2[candidate-dumpbin][candidate-map][candidate-reconfigure]![candidate-clean]");

    let all_positive = complete_weighted_sources(&mut pangine, &[(candidates.clone(), Relevance::DEFAULT), (episodes.clone(), Relevance::DEFAULT)], &question);
    let all_positive = ResearchAnswerView::from_result(&pangine, all_positive, candidate_projection.clone()).expect("candidate view");
    let all_negative = complete_weighted_sources(&mut pangine, &[(candidates, Relevance::DEFAULT), (episodes, Relevance::new(-1))], &question);
    let all_negative = ResearchAnswerView::from_result(&pangine, all_negative, candidate_projection).expect("candidate view");
    let expected = partitioned.materialize(&mut pangine).expect("candidate projection");
    assert_ne!(all_positive.materialize(&mut pangine), Some(expected.clone()));
    assert_ne!(all_negative.materialize(&mut pangine), Some(expected));
}

#[test]
#[ignore = "warning: a three-field unordered answer view is still only a research contract"]
fn unordered_packaging_decisions_compose_in_either_adjustment_order_without_changing_the_base_answer() {
    let mut pangine = packaging_answers();
    let base = linked_view(&mut pangine, "([action]->['action'])([tool]->['tool'])([scope]->['scope'])");
    let useful = linked_view(&mut pangine, "([action]->['useful-action'])([tool]->['useful-tool'])([scope]->['useful-scope'])");
    let failed = linked_view(&mut pangine, "([action]->['failed-action'])([tool]->['failed-tool'])([scope]->['failed-scope'])");

    let useful_then_failed = base.adjusted_by(&mut pangine, &useful, Relevance::DEFAULT).expect("compatible views");
    let useful_then_failed = useful_then_failed.adjusted_by(&mut pangine, &failed, Relevance::new(-1)).expect("compatible views");
    let failed_then_useful = base.adjusted_by(&mut pangine, &failed, Relevance::new(-1)).expect("compatible views");
    let failed_then_useful = failed_then_useful.adjusted_by(&mut pangine, &useful, Relevance::DEFAULT).expect("compatible views");

    let expected = "x2(([action]->[inspect-installed-modes])([scope]->[installed-payload])([tool]->[pkgutil]))
                    (([action]->[inspect-architecture])([scope]->[app-bundle])([tool]->[file]))
                    (([action]->[reinstall-package])([scope]->[installed-payload])([tool]->[installer]))
                    !(([action]->[inspect-signature])([scope]->[app-bundle])([tool]->[codesign]))";
    assert_projection(&mut pangine, &useful_then_failed, "([action]->['action'])([tool]->['tool'])([scope]->['scope'])", expected);
    assert_eq!(
        project(&mut pangine, &useful_then_failed, "([action]->['action'])([tool]->['tool'])([scope]->['scope'])"),
        project(&mut pangine, &failed_then_useful, "([action]->['action'])([tool]->['tool'])([scope]->['scope'])")
    );
    assert_eq!(
        choose_value(&mut pangine, &useful_then_failed),
        Some(must_ref(&mut pangine, "([action]->[inspect-installed-modes])([tool]->[pkgutil])([scope]->[installed-payload])"))
    );
    assert_projection(&mut pangine, &base, "['candidate']", "[candidate-architecture][candidate-modes][candidate-reinstall][candidate-signature]");
    let live_base = linked_view(&mut pangine, "([action]->['action'])([tool]->['tool'])([scope]->['scope'])");
    assert_projection(&mut pangine, &live_base, "['candidate']", "[candidate-architecture][candidate-modes][candidate-reinstall][candidate-signature]");
}

fn troubleshooting_answers() -> Pangine {
    let mut pangine = Pangine::new();
    remember_candidate(&mut pangine, "candidate-clean", "clean-build", "cargo");
    remember_candidate(&mut pangine, "candidate-dumpbin", "inspect-symbols", "dumpbin");
    remember_candidate(&mut pangine, "candidate-map", "inspect-symbols", "link-map");
    remember_candidate(&mut pangine, "candidate-reconfigure", "reconfigure", "cmake");

    remember_episode(&mut pangine, "episode-clean-failed-1", "candidate-clean", "clean-build", "cargo", "failed");
    remember_episode(&mut pangine, "episode-clean-failed-2", "candidate-clean", "clean-build", "cargo", "failed");
    remember_episode(&mut pangine, "episode-dumpbin-helpful", "candidate-dumpbin", "inspect-symbols", "dumpbin", "helpful");
    remember_episode(&mut pangine, "episode-obsolete-helpful", "candidate-obsolete", "delete-cache", "shell", "helpful");

    must_ref(&mut pangine, &format!("['candidates'] @ {BASE_QUESTION}"));
    must_ref(&mut pangine, &format!("['episodes'] @ {HELPFUL_QUESTION}"));
    must_ref(&mut pangine, &format!("['episodes'] @ {FAILED_QUESTION}"));
    pangine
}

fn layered_decision_answers() -> Pangine {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['choices'] ~= [choice-a]->[decision]->[A]");
    must_ref(&mut pangine, "['choices'] ~= [choice-b]->[decision]->[B]");
    must_ref(&mut pangine, "['outcomes'] ~= ([episode-a]->[decision]->[A])([episode-a]->[outcome]->[helpful])");
    must_ref(&mut pangine, "['outcomes'] ~= ([episode-b]->[decision]->[B])([episode-b]->[outcome]->[helpful])");
    must_ref(&mut pangine, "['reliability'] ~= ([review-b]->[episode]->[episode-b])([review-b]->[assessment]->[trusted])");

    must_ref(&mut pangine, "['choices'] @ ['candidate']->[decision]->['decision']");
    must_ref(&mut pangine, "['outcomes'] @ (['episode']->[decision]->['episode-decision'])(['episode']->[outcome]->[helpful])");
    must_ref(&mut pangine, "['reliability'] @ (['review']->[episode]->['trusted-episode'])(['review']->[assessment]->[trusted])");
    pangine
}

fn converging_layered_answers() -> Pangine {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['choices'] ~= [choice-b]->[decision]->[B]");
    must_ref(&mut pangine, "['outcomes'] ~= ([episode-b-1]->[decision]->[B])([episode-b-1]->[outcome]->[helpful])");
    must_ref(&mut pangine, "['outcomes'] ~= ([episode-b-2]->[decision]->[B])([episode-b-2]->[outcome]->[helpful])");
    must_ref(
        &mut pangine,
        "['reliability'] ~= ([review-b]->[episode]->[episode-b-1])([review-b]->[episode]->[episode-b-2])([review-b]->[assessment]->[trusted])",
    );

    must_ref(&mut pangine, "['choices'] @ ['candidate']->[decision]->['decision']");
    must_ref(&mut pangine, "['outcomes'] @ (['episode']->[decision]->['episode-decision'])(['episode']->[outcome]->[helpful])");
    must_ref(&mut pangine, "['reliability'] @ (['review']->[episode]->['trusted-episode'])(['review']->[assessment]->[trusted])");
    pangine
}

fn mutually_adjustable_answers() -> Pangine {
    let mut pangine = Pangine::new();
    must_ref(&mut pangine, "['left-memory'] ~= [left-record]->[decision]->[A]");
    must_ref(&mut pangine, "['right-memory'] ~= [right-record]->[decision]->[A]");
    must_ref(&mut pangine, "['left-memory'] @ ['left-row']->[decision]->['left-decision']");
    must_ref(&mut pangine, "['right-memory'] @ ['right-row']->[decision]->['right-decision']");
    pangine
}

fn weighted_animal_answer() -> Pangine {
    let mut pangine = Pangine::new();
    for (row, amount) in [("[cat]->[fish]", 3), ("[cat]->[milk]", 5), ("[dog]->[fish]", 7)] {
        for _ in 0..amount {
            must_ref(&mut pangine, &format!("['memory'] ~= {row}"));
        }
    }
    must_ref(&mut pangine, "['memory'] @ ['animal']->['food']");
    pangine
}

fn adjusted_troubleshooting_view(pangine: &mut Pangine) -> ResearchAnswerView {
    let base = linked_view(pangine, "['action']->['tool']");
    let helpful = linked_view(pangine, "['helpful-action']->['helpful-tool']");
    let failed = linked_view(pangine, "['failed-action']->['failed-tool']");
    let adjusted = base.adjusted_by(pangine, &helpful, Relevance::DEFAULT).expect("compatible helpful view");
    adjusted.adjusted_by(pangine, &failed, Relevance::new(-1)).expect("compatible failed view")
}

fn refresh_troubleshooting_outcomes(pangine: &mut Pangine) {
    must_ref(pangine, &format!("['episodes'] @ {HELPFUL_QUESTION}"));
    must_ref(pangine, &format!("['episodes'] @ {FAILED_QUESTION}"));
}

fn packaging_answers() -> Pangine {
    let mut pangine = Pangine::new();
    remember_packaging_candidate(&mut pangine, "candidate-signature", "inspect-signature", "codesign", "app-bundle");
    remember_packaging_candidate(&mut pangine, "candidate-modes", "inspect-installed-modes", "pkgutil", "installed-payload");
    remember_packaging_candidate(&mut pangine, "candidate-architecture", "inspect-architecture", "file", "app-bundle");
    remember_packaging_candidate(&mut pangine, "candidate-reinstall", "reinstall-package", "installer", "installed-payload");

    remember_packaging_episode(&mut pangine, "episode-signature-1", "candidate-signature", "inspect-signature", "codesign", "app-bundle", "failed");
    remember_packaging_episode(&mut pangine, "episode-signature-2", "candidate-signature", "inspect-signature", "codesign", "app-bundle", "failed");
    remember_packaging_episode(&mut pangine, "episode-modes-useful", "candidate-modes", "inspect-installed-modes", "pkgutil", "installed-payload", "useful");

    must_ref(
        &mut pangine,
        "['packaging-candidates'] @
            (['candidate']->[problem]->[installed-bundle-will-not-load])
            (['candidate']->[action]->['action'])
            (['candidate']->[tool]->['tool'])
            (['candidate']->[scope]->['scope'])",
    );
    must_ref(
        &mut pangine,
        "['packaging-episodes'] @
            (['useful-episode']->[candidate]->['useful-candidate'])
            (['useful-episode']->[problem]->[installed-bundle-will-not-load])
            (['useful-candidate']->[action]->['useful-action'])
            (['useful-candidate']->[tool]->['useful-tool'])
            (['useful-candidate']->[scope]->['useful-scope'])
            (['useful-episode']->[outcome]->[useful])",
    );
    must_ref(
        &mut pangine,
        "['packaging-episodes'] @
            (['failed-episode']->[candidate]->['failed-candidate'])
            (['failed-episode']->[problem]->[installed-bundle-will-not-load])
            (['failed-candidate']->[action]->['failed-action'])
            (['failed-candidate']->[tool]->['failed-tool'])
            (['failed-candidate']->[scope]->['failed-scope'])
            (['failed-episode']->[outcome]->[failed])",
    );
    pangine
}

fn remember_candidate(pangine: &mut Pangine, candidate: &str, action: &str, tool: &str) {
    must_ref(
        pangine,
        &format!(
            "['candidates'] ~= ([{candidate}]->[available]->[now])
                               ([{candidate}]->[problem]->[unresolved-symbol])
                               ([{candidate}]->[action]->[{action}])
                               ([{candidate}]->[tool]->[{tool}])"
        ),
    );
}

fn remember_episode(pangine: &mut Pangine, episode: &str, candidate: &str, action: &str, tool: &str, outcome: &str) {
    let record = format!(
        "([{episode}]->[candidate]->[{candidate}])
         ([{episode}]->[problem]->[unresolved-symbol])
         ([{episode}]->[action]->[{action}])
         ([{episode}]->[tool]->[{tool}])
         ([{candidate}]->[action]->[{action}])
         ([{candidate}]->[tool]->[{tool}])
         ([{episode}]->[outcome]->[{outcome}])"
    );
    must_ref(pangine, &format!("['episodes'] ~= {record}"));
    must_ref(pangine, &format!("['{outcome}-episodes'] ~= {record}"));
}

fn remember_packaging_candidate(pangine: &mut Pangine, candidate: &str, action: &str, tool: &str, scope: &str) {
    must_ref(
        pangine,
        &format!(
            "['packaging-candidates'] ~= ([{candidate}]->[problem]->[installed-bundle-will-not-load])
                                         ([{candidate}]->[action]->[{action}])
                                         ([{candidate}]->[tool]->[{tool}])
                                         ([{candidate}]->[scope]->[{scope}])"
        ),
    );
}

fn remember_packaging_episode(pangine: &mut Pangine, episode: &str, candidate: &str, action: &str, tool: &str, scope: &str, outcome: &str) {
    must_ref(
        pangine,
        &format!(
            "['packaging-episodes'] ~= ([{episode}]->[candidate]->[{candidate}])
                                       ([{episode}]->[problem]->[installed-bundle-will-not-load])
                                       ([{candidate}]->[action]->[{action}])
                                       ([{candidate}]->[tool]->[{tool}])
                                       ([{candidate}]->[scope]->[{scope}])
                                       ([{episode}]->[outcome]->[{outcome}])"
        ),
    );
}

fn linked_view(pangine: &mut Pangine, projection: &str) -> ResearchAnswerView {
    let projection = must_ref(pangine, projection);
    pangine.answer_view(&projection).expect("one answer behind projection")
}

fn complete_weighted_sources(pangine: &mut Pangine, sources: &[(ConceptId, Relevance)], question: &ConceptId) -> CompletionResult {
    let mut selected = Vec::new();
    let mut factors = BTreeMap::new();
    for (percept, factor) in sources {
        for (concept, &relevance) in &pangine.percept_subconcepts[&percept.index()] {
            let source = QuestionSource::from_percept(percept.clone(), concept.clone(), relevance);
            selected.push(source);
        }
        factors.insert(percept.clone(), *factor);
    }
    let snapshot = pangine.question_snapshot_from_sources(selected, question);
    let mut result = pangine.complete_question_snapshot(question, &snapshot);
    pangine.scale_completion_result_sources(&mut result, &factors).expect("weighted source contribution");
    result
}

fn same_completion_result(left: &CompletionResult, right: &CompletionResult) -> bool {
    left.question() == right.question() && left.completions() == right.completions()
}

fn source_inventory(pangine: &mut Pangine, view: &ResearchAnswerView) -> BTreeMap<String, BTreeMap<String, (i64, i64)>> {
    view.answer
        .result
        .completions()
        .iter()
        .map(|completion| {
            let candidate = pangine.instantiate_completion(&view.projection, completion).expect("complete view");
            let candidate = pangine.format_concept(&candidate, false);
            let sources = completion
                .evidence()
                .iter()
                .map(|evidence| {
                    let source = pangine.format_concept(evidence.source_concept(), false);
                    let id = [
                        "episode-clean-failed-1",
                        "episode-clean-failed-2",
                        "episode-dumpbin-helpful",
                        "candidate-clean",
                        "candidate-dumpbin",
                        "candidate-map",
                        "candidate-reconfigure",
                    ]
                    .into_iter()
                    .find(|id| source.contains(id))
                    .expect("stable source id")
                    .to_owned();
                    (id, (evidence.source_relevance().weight(), evidence.source_contribution().weight()))
                })
                .collect();
            (candidate, sources)
        })
        .collect()
}

fn identified_source_contributions(
    pangine: &mut Pangine,
    view: &ResearchAnswerView,
    projection: &str,
    candidate: &str,
    source_ids: &[&str],
) -> BTreeMap<String, BTreeSet<(i64, i64)>> {
    let projection = must_ref(pangine, projection);
    let candidate = must_ref(pangine, candidate);
    let completion = view
        .answer
        .result
        .completions()
        .iter()
        .find(|completion| pangine.instantiate_completion(&projection, completion).as_ref() == Some(&candidate))
        .expect("candidate completion");

    let mut sources = BTreeMap::<String, BTreeSet<(i64, i64)>>::new();
    for evidence in completion.evidence() {
        let source = pangine.format_concept(evidence.source_concept(), false);
        if let Some(id) = source_ids.iter().find(|id| source.contains(&format!("[{id}]->"))) {
            sources.entry((*id).to_string()).or_default().insert((evidence.source_relevance().weight(), evidence.source_contribution().weight()));
        }
    }
    sources
}

fn sole_completion_evidence_count(view: &ResearchAnswerView) -> usize {
    assert_eq!(view.answer.result.completions().len(), 1);
    view.answer.result.completions()[0].evidence().len()
}

fn sole_source_percept(view: &ResearchAnswerView) -> ConceptId {
    view.answer.result.completions()[0].evidence()[0].source_percept().expect("Percept source").clone()
}

fn assert_projection(pangine: &mut Pangine, answer: &ResearchAnswerView, projection: &str, expected: &str) {
    assert_eq!(project(pangine, answer, projection), must_ref(pangine, expected));
}

fn choose_value(pangine: &mut Pangine, answer: &ResearchAnswerView) -> Option<ConceptId> {
    answer.choose(pangine).map(|choice| choice.selected)
}

fn adjustment_counts(adjustment: &ResearchAdjustment) -> (usize, usize, usize, usize, usize, usize, usize) {
    (
        adjustment.target_rows,
        adjustment.adjustment_rows,
        adjustment.matched_target_rows,
        adjustment.matched_adjustment_rows,
        adjustment.matched_pairs,
        adjustment.changed_target_rows,
        adjustment.added_source_occurrences,
    )
}

fn project(pangine: &mut Pangine, answer: &ResearchAnswerView, projection: &str) -> ConceptId {
    let projection = must_ref(pangine, projection);
    let view = answer.projecting(pangine, projection).expect("answer projection");
    view.materialize(pangine).expect("nonempty projection")
}

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
