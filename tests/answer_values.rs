use pangine::{AnswerPublicationError, ConceptId, Pangine, Relevance};

#[test]
fn immutable_choice_changes_live_outputs_only_after_publication() {
    let mut pangine = weighted_animals();
    let pair = must_ref(&mut pangine, "['animal']->['food']");
    let animal = pangine.reference_percept("animal");
    let food = pangine.reference_percept("food");
    let base = pangine.answer_view(&pair).expect("animal-food answer");
    let animal_choice = base.projecting(&pangine, animal.clone()).expect("animal view").choose(&mut pangine).expect("positive animal choice");
    let food_choice = base.projecting(&pangine, food).expect("food view").choose(&mut pangine).expect("positive food choice");

    assert_eq!(animal_choice.selected(), &must_ref(&mut pangine, "[cat]"));
    assert_eq!(food_choice.selected(), &must_ref(&mut pangine, "[fish]"));
    assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "x8[cat]x7[dog]"));

    let publication = animal_choice.view().answer().publish(&mut pangine).expect("current answer revision");
    assert_ne!(publication.revision(), publication.prior_revision());
    assert_eq!(must_ref(&mut pangine, "$['animal']"), must_ref(&mut pangine, "x8[cat]"));
    assert_eq!(must_ref(&mut pangine, "$['food']"), must_ref(&mut pangine, "x5[milk]x3[fish]"));
    assert_eq!(food_choice.view().answer().publish(&mut pangine).err(), Some(AnswerPublicationError::Stale));
}

#[test]
fn answer_adjustment_composes_through_reliability_outcomes_and_candidates() {
    let mut pangine = layered_decisions();
    let decision = pangine.reference_percept("decision");
    let episode = pangine.reference_percept("episode");
    let episode_decision = pangine.reference_percept("episode-decision");
    let trusted_episode = pangine.reference_percept("trusted-episode");
    let candidates = pangine.answer_view(&decision).expect("candidate answer");
    let outcomes = pangine.answer_view(&episode).expect("outcome answer");
    let trusted = pangine.answer_view(&trusted_episode).expect("reliability answer");

    let trusted_outcomes = outcomes.adjust(&mut pangine, &trusted, Relevance::DEFAULT).expect("matching episode values");
    assert_eq!(
        (
            trusted_outcomes.target_rows(),
            trusted_outcomes.adjustment_rows(),
            trusted_outcomes.matched_target_rows(),
            trusted_outcomes.matched_adjustment_rows(),
            trusted_outcomes.matched_pairs(),
            trusted_outcomes.changed_target_rows(),
            trusted_outcomes.added_source_occurrences(),
        ),
        (2, 1, 1, 1, 1, 1, 1)
    );
    let trusted_outcomes = trusted_outcomes.into_view();
    assert_eq!(trusted_outcomes.materialize(&mut pangine), Some(must_ref(&mut pangine, "x2[episode-b][episode-a]")));

    let trusted_decisions = trusted_outcomes.projecting(&pangine, episode_decision).expect("outcome decision view");
    let adjusted = candidates.adjust(&mut pangine, &trusted_decisions, Relevance::DEFAULT).expect("matching decision values").into_view();
    assert_eq!(adjusted.materialize(&mut pangine), Some(must_ref(&mut pangine, "x3[B]x2[A]")));
    assert_eq!(adjusted.answer().shape(), candidates.answer().shape());
    assert!(adjusted.projecting(&pangine, episode).is_none());

    let choice = adjusted.choose(&mut pangine).expect("positive final choice");
    assert_eq!(choice.selected(), &must_ref(&mut pangine, "[B]"));
    choice.view().answer().publish(&mut pangine).expect("current candidate revision");
    assert_eq!(must_ref(&mut pangine, "$['candidate']"), must_ref(&mut pangine, "x3[choice-b]"));
    assert_eq!(must_ref(&mut pangine, "$['decision']"), must_ref(&mut pangine, "x3[B]"));
}

#[test]
fn intermediate_choice_controls_which_evidence_reaches_a_later_answer() {
    let mut pangine = layered_decisions();
    let decision = pangine.reference_percept("decision");
    let episode = pangine.reference_percept("episode");
    let episode_decision = pangine.reference_percept("episode-decision");
    let trusted_episode = pangine.reference_percept("trusted-episode");
    let candidates = pangine.answer_view(&decision).expect("candidate answer");
    let outcomes = pangine.answer_view(&episode).expect("outcome answer");
    let trusted = pangine.answer_view(&trusted_episode).expect("reliability answer");

    let trusted_outcomes = outcomes.adjusted_by(&mut pangine, &trusted, Relevance::DEFAULT).expect("matching episode values");
    let selected_episode = trusted_outcomes.choose(&mut pangine).expect("positive episode choice");
    assert_eq!(selected_episode.selected(), &must_ref(&mut pangine, "[episode-b]"));

    let selected_decision = selected_episode.view().projecting(&pangine, episode_decision).expect("selected outcome decision");
    let adjusted = candidates.adjusted_by(&mut pangine, &selected_decision, Relevance::DEFAULT).expect("matching selected decision");
    assert_eq!(adjusted.materialize(&mut pangine), Some(must_ref(&mut pangine, "x3[B][A]")));
}

#[test]
fn empty_adjustment_factor_matches_without_changing_the_answer() {
    let mut pangine = layered_decisions();
    let decision = pangine.reference_percept("decision");
    let episode_decision = pangine.reference_percept("episode-decision");
    let candidates = pangine.answer_view(&decision).expect("candidate answer");
    let outcomes = pangine.answer_view(&episode_decision).expect("outcome answer");

    let adjusted = candidates.adjust(&mut pangine, &outcomes, Relevance::EMPTY).expect("valid empty adjustment");
    assert_eq!(adjusted.matched_target_rows(), 2);
    assert_eq!(adjusted.matched_adjustment_rows(), 2);
    assert_eq!(adjusted.matched_pairs(), 2);
    assert_eq!(adjusted.changed_target_rows(), 0);
    assert_eq!(adjusted.added_source_occurrences(), 0);
    assert!(adjusted.view().answer().result().completions() == candidates.answer().result().completions());
}

#[test]
fn deep_adjustment_keeps_binding_context_linear() {
    const DEPTH: usize = 8;
    const OUTPUTS_PER_LAYER: usize = 2;

    let mut pangine = Pangine::new();
    let mut views = Vec::new();
    let mut outputs = Vec::new();
    for index in 0..DEPTH {
        must_ref(&mut pangine, &format!("['memory-{index}'] ~= [record-{index}]->[value]->[A]"));
        must_ref(&mut pangine, &format!("['memory-{index}'] @ ['row-{index}']->[value]->['layer-{index}']"));
        let layer = pangine.reference_percept(&format!("layer-{index}"));
        views.push(pangine.answer_view(&layer).expect("layer answer"));
        outputs.push(pangine.reference_percept(&format!("row-{index}")));
        outputs.push(layer);
    }

    let mut chain = views.pop().expect("deepest answer");
    while let Some(target) = views.pop() {
        chain = target.adjusted_by(&mut pangine, &chain, Relevance::DEFAULT).expect("matching layer values");
    }

    assert_eq!(chain.materialize(&mut pangine), Some(must_ref(&mut pangine, "x8[A]")));
    let completion = &chain.answer().result().completions()[0];
    let source_bindings =
        completion.evidence().iter().map(|evidence| outputs.iter().filter(|output| evidence.binding(output).is_some()).count()).sum::<usize>();
    let adjusted_outputs = completion.evidence().iter().map(|evidence| evidence.adjusted_outputs().count()).sum::<usize>();
    assert_eq!(completion.evidence().len(), DEPTH);
    assert_eq!(source_bindings, DEPTH * OUTPUTS_PER_LAYER);
    assert_eq!(adjusted_outputs, (DEPTH - 1) * OUTPUTS_PER_LAYER);
}

#[test]
fn detaching_one_output_stales_the_complete_snapshot() {
    let mut pangine = weighted_animals();
    let pair = must_ref(&mut pangine, "['animal']->['food']");
    let animal = pangine.reference_percept("animal");
    let food = pangine.reference_percept("food");
    let snapshot = pangine.answer_view(&pair).expect("complete answer");
    let branch = snapshot.choose(&mut pangine).expect("complete branch");

    let manual = must_ref(&mut pangine, "[manual-animal]");
    assert!(pangine.set_percept_value(&animal, Some(manual)));
    assert_eq!(branch.view().answer().publish(&mut pangine).err(), Some(AnswerPublicationError::Stale));
    assert!(pangine.answer_snapshot(&animal).is_none());
    assert_eq!(pangine.answer_snapshot(&food).expect("remaining answer").shape(), &food);
}

fn weighted_animals() -> Pangine {
    let mut pangine = Pangine::new();
    for (row, amount) in [("[cat]->[fish]", 3), ("[cat]->[milk]", 5), ("[dog]->[fish]", 7)] {
        for _ in 0..amount {
            must_ref(&mut pangine, &format!("['memory'] ~= {row}"));
        }
    }
    must_ref(&mut pangine, "['memory'] @ ['animal']->['food']");
    pangine
}

fn layered_decisions() -> Pangine {
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

fn must_ref(pangine: &mut Pangine, input: &str) -> ConceptId {
    pangine
        .reference_concept(input)
        .unwrap_or_else(|error| panic!("failed to parse {input:?}: {error}"))
        .unwrap_or_else(|| panic!("expected non-null Concept for {input:?}"))
}
