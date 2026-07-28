//! Test-only comparison of correction and retraction state models.
//!
//! These models use Pangine's current experience traversal to obtain exact
//! Observation closures, but they do not change production state or propose
//! final grammar. The tests make the identity, replay, partition, and degraded
//! mode tradeoffs explicit before any model can become accepted semantics.

use std::collections::{BTreeMap, BTreeSet};

use pangine::{ConceptId, ConceptKind, Pangine};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DurableExactState {
    learned: BTreeSet<ConceptId>,
    retracted: BTreeSet<ConceptId>,
}

impl DurableExactState {
    fn learn(&mut self, closure: &BTreeSet<ConceptId>) {
        self.learned.extend(closure.iter().cloned());
    }

    fn retract(&mut self, closure: &BTreeSet<ConceptId>) {
        self.retracted.extend(closure.iter().cloned());
    }

    fn correct(&mut self, old_closure: &BTreeSet<ConceptId>, new_closure: &BTreeSet<ConceptId>) {
        self.retract(old_closure);
        self.learn(new_closure);
    }

    fn merged(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        merged.learned.extend(other.learned.iter().cloned());
        merged.retracted.extend(other.retracted.iter().cloned());
        merged
    }

    fn active(&self) -> BTreeSet<ConceptId> {
        self.learned.difference(&self.retracted).cloned().collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TaggedDeliveryState {
    learned: BTreeSet<(ConceptId, ConceptId)>,
    retracted: BTreeSet<(ConceptId, ConceptId)>,
}

impl TaggedDeliveryState {
    fn learn(&mut self, closure: &BTreeSet<ConceptId>, delivery: &ConceptId) {
        self.learned.extend(closure.iter().cloned().map(|observation| (observation, delivery.clone())));
    }

    fn retract(&mut self, closure: &BTreeSet<ConceptId>, delivery: &ConceptId) {
        self.retracted.extend(closure.iter().cloned().map(|observation| (observation, delivery.clone())));
    }

    fn active(&self) -> BTreeSet<ConceptId> {
        self.learned.difference(&self.retracted).map(|(observation, _)| observation.clone()).collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RootRevisionState {
    learned: BTreeSet<ConceptId>,
    retracted: BTreeSet<ConceptId>,
    corrections: BTreeSet<(ConceptId, ConceptId)>,
}

impl RootRevisionState {
    fn learn(&mut self, root: &ConceptId) {
        self.learned.insert(root.clone());
    }

    fn retract(&mut self, root: &ConceptId) {
        self.retracted.insert(root.clone());
    }

    fn correct(&mut self, old_root: &ConceptId, new_root: &ConceptId) {
        self.learn(new_root);
        self.corrections.insert((old_root.clone(), new_root.clone()));
    }

    fn merged(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        merged.learned.extend(other.learned.iter().cloned());
        merged.retracted.extend(other.retracted.iter().cloned());
        merged.corrections.extend(other.corrections.iter().cloned());
        merged
    }

    fn active_roots(&self) -> BTreeSet<ConceptId> {
        let superseded = self.retracted.iter().cloned().chain(self.corrections.iter().map(|(old, _)| old.clone())).collect::<BTreeSet<_>>();
        self.learned.difference(&superseded).cloned().collect()
    }

    fn active_observations(&self, closures: &BTreeMap<ConceptId, BTreeSet<ConceptId>>) -> BTreeSet<ConceptId> {
        self.active_roots()
            .into_iter()
            .flat_map(|root| closures.get(&root).unwrap_or_else(|| panic!("missing closure for root {root:?}")).iter().cloned())
            .collect()
    }

    fn replacements(&self, root: &ConceptId) -> BTreeSet<ConceptId> {
        self.corrections.iter().filter_map(|(old, new)| (old == root).then_some(new.clone())).collect()
    }

    fn correction_cycle_nodes(&self) -> BTreeSet<ConceptId> {
        let nodes = self.corrections.iter().flat_map(|(old, new)| [old.clone(), new.clone()]).collect::<BTreeSet<_>>();
        let mut cycle_nodes = BTreeSet::new();

        for start in nodes {
            let mut visited = BTreeSet::new();
            let mut pending = vec![start.clone()];
            while let Some(current) = pending.pop() {
                for replacement in self.replacements(&current) {
                    if replacement == start {
                        cycle_nodes.insert(start.clone());
                    } else if visited.insert(replacement.clone()) {
                        pending.push(replacement);
                    }
                }
            }
        }

        cycle_nodes
    }
}

fn must_reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a concept from {source:?}"))
}

fn observation_closure_for(pangine: &mut Pangine, input: &ConceptId) -> BTreeSet<ConceptId> {
    let scratch = pangine.reference_percept("revision-oracle-scratch");
    let state = pangine.perform_experience(&scratch, Some(input)).unwrap();
    pangine.get_observations(&state).unwrap().into_iter().collect()
}

fn experience_roots_and_closures(pangine: &mut Pangine, source: &str) -> BTreeMap<ConceptId, BTreeSet<ConceptId>> {
    let input = must_reference(pangine, source);
    if matches!(pangine.concept_kind(&input), Some(ConceptKind::ObservationSet)) {
        return pangine
            .get_observations(&input)
            .unwrap()
            .into_iter()
            .map(|root| {
                let closure = observation_closure_for(pangine, &root);
                (root, closure)
            })
            .collect();
    }

    let closure = observation_closure_for(pangine, &input);
    let root = if matches!(pangine.concept_kind(&input), Some(ConceptKind::Observation { .. })) {
        input
    } else {
        closure
            .iter()
            .find(|record| pangine.get_observation(record).as_ref() == Some(&input))
            .cloned()
            .unwrap_or_else(|| panic!("missing root Observation for {source:?}"))
    };
    BTreeMap::from([(root, closure)])
}

fn experience_root_and_closure(pangine: &mut Pangine, source: &str) -> (ConceptId, BTreeSet<ConceptId>) {
    let mut roots = experience_roots_and_closures(pangine, source);
    assert_eq!(roots.len(), 1, "expected one direct root from {source:?}");
    roots.pop_first().unwrap()
}

fn observation_closure(pangine: &mut Pangine, source: &str) -> BTreeSet<ConceptId> {
    experience_root_and_closure(pangine, source).1
}

fn reference_observation_state(pangine: &mut Pangine, observations: &BTreeSet<ConceptId>) -> Option<ConceptId> {
    match observations.len() {
        0 => None,
        1 => observations.first().cloned(),
        _ => {
            let entries = observations.iter().map(|observation| pangine.format_concept(observation, false)).collect::<Vec<_>>().join(", ");
            pangine.reference_concept(&format!("<{entries}>")).unwrap()
        }
    }
}

fn ask(pangine: &mut Pangine, memory_name: &str, output_name: &str, question_template: &str) -> Option<ConceptId> {
    let output = pangine.reference_percept(output_name);
    assert!(pangine.set_percept_value(&output, None));
    let question = question_template.replace("$OUTPUT", &format!("['{output_name}']"));
    must_reference(pangine, &format!("['{memory_name}'] @ {question}"));
    pangine.get_value(&output)
}

#[test]
fn physical_deletion_allows_stale_replay_to_restore_an_observation() {
    let mut pangine = Pangine::new();
    let observation = must_reference(&mut pangine, "?[policy-v1]:[cargo]");
    let stale_partition = BTreeSet::from([observation.clone()]);
    let mut current = stale_partition.clone();

    current.remove(&observation);
    assert!(current.is_empty());

    current.extend(stale_partition);
    assert!(current.contains(&observation));
}

#[test]
fn durable_exact_retraction_is_replay_order_and_partition_independent() {
    let mut pangine = Pangine::new();
    let closure = observation_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");

    let mut learn_then_retract = DurableExactState::default();
    learn_then_retract.learn(&closure);
    learn_then_retract.retract(&closure);

    let mut retract_then_learn = DurableExactState::default();
    retract_then_learn.retract(&closure);
    retract_then_learn.learn(&closure);

    let mut learned_partition = DurableExactState::default();
    learned_partition.learn(&closure);
    let mut retracted_partition = DurableExactState::default();
    retracted_partition.retract(&closure);
    let merged = learned_partition.merged(&retracted_partition);

    assert_eq!(learn_then_retract, retract_then_learn);
    assert_eq!(learn_then_retract, merged);
    assert!(merged.active().is_empty());

    let mut stale_replay = merged;
    stale_replay.learn(&closure);
    assert!(stale_replay.active().is_empty());
}

#[test]
fn correction_retracts_the_old_closure_without_affecting_an_unrelated_observer() {
    let mut pangine = Pangine::new();
    let old_closure = observation_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let new_closure = observation_closure(&mut pangine, "?[policy-v2]:{[full-test]->[cli-runner]}");
    let unrelated_closure = observation_closure(&mut pangine, "?[personal-note]:{[full-test]->[cargo]}");

    let mut state = DurableExactState::default();
    state.learn(&old_closure);
    state.learn(&unrelated_closure);
    state.correct(&old_closure, &new_closure);

    let active = state.active();
    assert!(old_closure.is_disjoint(&active));
    assert!(new_closure.is_subset(&active));
    assert!(unrelated_closure.is_subset(&active));

    state.learn(&old_closure);
    assert!(old_closure.is_disjoint(&state.active()));
}

#[test]
fn exact_retraction_does_not_treat_inversion_as_withdrawal() {
    let mut pangine = Pangine::new();
    let positive = observation_closure(&mut pangine, "?[weather]:[rain]");
    let inverted = observation_closure(&mut pangine, "?[weather]:![rain]");

    let mut state = DurableExactState::default();
    state.learn(&positive);
    state.learn(&inverted);
    state.retract(&positive);

    let active = state.active();
    assert!(positive.is_disjoint(&active));
    assert!(inverted.is_subset(&active));
}

#[test]
fn exact_retraction_removes_shared_recursive_records_with_the_same_identity() {
    let mut pangine = Pangine::new();
    let first_closure = observation_closure(&mut pangine, "?[same-source]:[A]*[B]");
    let second_closure = observation_closure(&mut pangine, "?[same-source]:[A]*[C]");
    let shared_a = must_reference(&mut pangine, "?[same-source]:[A]");
    let second_root = must_reference(&mut pangine, "?[same-source]:[A]*[C]");
    assert!(first_closure.contains(&shared_a));
    assert!(second_closure.contains(&shared_a));

    let mut state = DurableExactState::default();
    state.learn(&first_closure);
    state.learn(&second_closure);
    state.retract(&first_closure);

    let active = state.active();
    assert!(!active.contains(&shared_a));
    assert!(active.contains(&second_root));
}

#[test]
fn exact_retraction_requires_a_new_revision_identity_to_restore_the_same_payload() {
    let mut pangine = Pangine::new();
    let first_revision = observation_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let later_revision = observation_closure(&mut pangine, "?[policy-v3]:{[full-test]->[cargo]}");

    let mut state = DurableExactState::default();
    state.learn(&first_revision);
    state.retract(&first_revision);
    state.learn(&first_revision);
    assert!(first_revision.is_disjoint(&state.active()));

    state.learn(&later_revision);
    assert!(later_revision.is_subset(&state.active()));
}

#[test]
fn tagged_deliveries_allow_reactivation_and_preserve_overlapping_records() {
    let mut pangine = Pangine::new();
    let first_closure = observation_closure(&mut pangine, "?[same-source]:[A]*[B]");
    let second_closure = observation_closure(&mut pangine, "?[same-source]:[A]*[C]");
    let shared_a = must_reference(&mut pangine, "?[same-source]:[A]");
    let first_delivery = must_reference(&mut pangine, "[delivery-1]");
    let second_delivery = must_reference(&mut pangine, "[delivery-2]");
    let third_delivery = must_reference(&mut pangine, "[delivery-3]");

    let mut state = TaggedDeliveryState::default();
    state.learn(&first_closure, &first_delivery);
    state.learn(&second_closure, &second_delivery);
    state.retract(&first_closure, &first_delivery);
    assert!(state.active().contains(&shared_a));

    state.learn(&first_closure, &third_delivery);
    assert!(first_closure.is_subset(&state.active()));
}

#[test]
fn direct_roots_reconstruct_the_current_add_only_observation_state() {
    let mut pangine = Pangine::new();
    let current_memory = pangine.reference_percept("current-memory");
    let sources = [
        "[global]",
        "![inverted]",
        "[A][B]",
        "[A]*[B]",
        "x2[weighted-a]x3[weighted-b]",
        "{[source]->{[relation]->[target]}}",
        "?[event-1]:[rain]*[A]",
        "?[event-1]:[rain]*[B]",
        "?[event-2]:{[C]->![D]}",
        "?[outer]:?[inner]:[nested]",
        "?[]:[explicit-global]",
        "['percept-input']",
        "<?[batch-a]:[left]*[A]?[batch-b]:{[right]->[B]}>",
    ];
    let mut roots = RootRevisionState::default();
    let mut closures = BTreeMap::new();

    for source in sources {
        let input = must_reference(&mut pangine, source);
        let expected_input_closure = observation_closure_for(&mut pangine, &input);
        let input_roots = experience_roots_and_closures(&mut pangine, source);
        let reconstructed_input_closure = input_roots.values().flat_map(|closure| closure.iter().cloned()).collect::<BTreeSet<_>>();
        assert_eq!(reconstructed_input_closure, expected_input_closure, "direct roots did not reconstruct {source:?}");

        for (root, closure) in input_roots {
            roots.learn(&root);
            closures.insert(root, closure);
        }
        let current = pangine.perform_experience(&current_memory, Some(&input));
        assert!(pangine.set_percept_value(&current_memory, current));
    }

    let current = pangine.get_value(&current_memory).unwrap();
    let current_observations = pangine.get_observations(&current).unwrap().into_iter().collect::<BTreeSet<_>>();
    assert_eq!(roots.active_observations(&closures), current_observations);
}

#[test]
fn root_derived_state_preserves_current_question_results() {
    let mut pangine = Pangine::new();
    let sources = [
        "?[event-1]:[A]*[B]",
        "?[event-2]:[C]*[B]",
        "?[edge-1]:{[D]->[E]}",
        "?[edge-2]:{[F]->[E]}",
        "?[nested]:{[G]->{[H]->[I]}}",
        "[global-a]*[B]",
        "<?[batch-a]:[batch-left]*[B]?[batch-b]:{[batch-right]->[E]}>",
    ];
    let current_memory = pangine.reference_percept("current-question-memory");
    let root_memory = pangine.reference_percept("root-question-memory");
    let mut roots = RootRevisionState::default();
    let mut closures = BTreeMap::new();

    for source in sources {
        let input = must_reference(&mut pangine, source);
        let current = pangine.perform_experience(&current_memory, Some(&input));
        assert!(pangine.set_percept_value(&current_memory, current));
        for (root, closure) in experience_roots_and_closures(&mut pangine, source) {
            roots.learn(&root);
            closures.insert(root, closure);
        }
    }

    let active = roots.active_observations(&closures);
    let root_state = reference_observation_state(&mut pangine, &active);
    assert!(pangine.set_percept_value(&root_memory, root_state));
    assert_eq!(pangine.get_value(&root_memory), pangine.get_value(&current_memory));

    let questions = ["$OUTPUT*[B]", "{$OUTPUT->[E]}", "{[G]->{$OUTPUT->[I]}}", "?[event-1]:$OUTPUT*[B]"];
    for (index, template) in questions.into_iter().enumerate() {
        let current_answer = ask(&mut pangine, "current-question-memory", &format!("current-answer-{index}"), template);
        let root_answer = ask(&mut pangine, "root-question-memory", &format!("root-answer-{index}"), template);
        assert_eq!(root_answer, current_answer, "question parity failed for {template:?}");
    }
}

#[test]
fn root_retraction_preserves_shared_recursive_records_from_active_roots() {
    let mut pangine = Pangine::new();
    let (first_root, first_closure) = experience_root_and_closure(&mut pangine, "?[same-source]:[A]*[B]");
    let (second_root, second_closure) = experience_root_and_closure(&mut pangine, "?[same-source]:[A]*[C]");
    let shared_a = must_reference(&mut pangine, "?[same-source]:[A]");
    let mut closures = BTreeMap::from([(first_root.clone(), first_closure), (second_root.clone(), second_closure)]);
    let mut state = RootRevisionState::default();
    state.learn(&first_root);
    state.learn(&second_root);
    state.retract(&first_root);

    let active = state.active_observations(&closures);
    assert!(!active.contains(&first_root));
    assert!(active.contains(&second_root));
    assert!(active.contains(&shared_a));

    closures.remove(&first_root);
    assert_eq!(state.active_observations(&closures), active);
}

#[test]
fn root_retraction_is_partition_and_stale_replay_safe() {
    let mut pangine = Pangine::new();
    let (root, _) = experience_root_and_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let mut learned_partition = RootRevisionState::default();
    learned_partition.learn(&root);
    let mut retracted_partition = RootRevisionState::default();
    retracted_partition.retract(&root);

    let forward = learned_partition.merged(&retracted_partition);
    let reverse = retracted_partition.merged(&learned_partition);
    assert_eq!(forward, reverse);
    assert!(forward.active_roots().is_empty());

    let mut stale_replay = forward;
    stale_replay.learn(&root);
    assert!(stale_replay.active_roots().is_empty());
}

#[test]
fn root_correction_preserves_auditable_chains_and_stale_replay_safety() {
    let mut pangine = Pangine::new();
    let (first_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let (second_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v2]:{[full-test]->[cli-runner]}");
    let (third_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v3]:{[full-test]->[task-launch]}");
    let mut state = RootRevisionState::default();
    state.learn(&first_root);
    state.correct(&first_root, &second_root);
    state.correct(&second_root, &third_root);
    state.learn(&first_root);

    assert_eq!(state.active_roots(), BTreeSet::from([third_root.clone()]));
    assert_eq!(state.replacements(&first_root), BTreeSet::from([second_root.clone()]));
    assert_eq!(state.replacements(&second_root), BTreeSet::from([third_root]));
}

#[test]
fn concurrent_root_corrections_remain_visible_without_an_invented_winner() {
    let mut pangine = Pangine::new();
    let (old_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let (first_replacement, _) = experience_root_and_closure(&mut pangine, "?[policy-v2a]:{[full-test]->[cli-runner]}");
    let (second_replacement, _) = experience_root_and_closure(&mut pangine, "?[policy-v2b]:{[full-test]->[task-launch]}");

    let mut first_partition = RootRevisionState::default();
    first_partition.learn(&old_root);
    first_partition.correct(&old_root, &first_replacement);
    let mut second_partition = RootRevisionState::default();
    second_partition.correct(&old_root, &second_replacement);

    let forward = first_partition.merged(&second_partition);
    let reverse = second_partition.merged(&first_partition);
    assert_eq!(forward, reverse);
    assert_eq!(forward.active_roots(), BTreeSet::from([first_replacement.clone(), second_replacement.clone()]));
    assert_eq!(forward.replacements(&old_root), BTreeSet::from([first_replacement, second_replacement]));
}

#[test]
fn correction_cycles_remain_visible_instead_of_inventing_a_winner() {
    let mut pangine = Pangine::new();
    let (first_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let (second_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v2]:{[full-test]->[cli-runner]}");
    let mut first_partition = RootRevisionState::default();
    first_partition.learn(&first_root);
    first_partition.correct(&first_root, &second_root);
    let mut second_partition = RootRevisionState::default();
    second_partition.correct(&second_root, &first_root);

    let forward = first_partition.merged(&second_partition);
    let reverse = second_partition.merged(&first_partition);
    assert_eq!(forward, reverse);
    assert!(forward.active_roots().is_empty());
    assert_eq!(forward.correction_cycle_nodes(), BTreeSet::from([first_root, second_root]));
}

#[test]
fn independently_active_revision_partitions_do_not_reduce_like_merged_state() {
    let mut pangine = Pangine::new();
    let (old_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let (new_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v2]:{[full-test]->[cli-runner]}");

    let mut learned_partition = RootRevisionState::default();
    learned_partition.learn(&old_root);
    let mut retracted_partition = RootRevisionState::default();
    retracted_partition.retract(&old_root);

    let locally_active_after_retraction = learned_partition.active_roots().union(&retracted_partition.active_roots()).cloned().collect::<BTreeSet<_>>();
    let merged_after_retraction = learned_partition.merged(&retracted_partition).active_roots();
    assert_eq!(locally_active_after_retraction, BTreeSet::from([old_root.clone()]));
    assert!(merged_after_retraction.is_empty());

    let mut correction_partition = RootRevisionState::default();
    correction_partition.correct(&old_root, &new_root);
    let locally_active_after_correction = learned_partition.active_roots().union(&correction_partition.active_roots()).cloned().collect::<BTreeSet<_>>();
    let merged_after_correction = learned_partition.merged(&correction_partition).active_roots();
    assert_eq!(locally_active_after_correction, BTreeSet::from([old_root, new_root.clone()]));
    assert_eq!(merged_after_correction, BTreeSet::from([new_root]));
}

#[test]
fn losing_a_revision_partition_can_reactivate_superseded_information() {
    let mut pangine = Pangine::new();
    let (old_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let (new_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v2]:{[full-test]->[cli-runner]}");
    let mut learned_partition = RootRevisionState::default();
    learned_partition.learn(&old_root);
    let mut correction_partition = RootRevisionState::default();
    correction_partition.correct(&old_root, &new_root);

    let merged = learned_partition.merged(&correction_partition);
    assert_eq!(merged.active_roots(), BTreeSet::from([new_root]));
    assert_eq!(learned_partition.active_roots(), BTreeSet::from([old_root]));
}

#[test]
fn reversing_a_correction_creates_a_cycle_instead_of_undoing_the_link() {
    let mut pangine = Pangine::new();
    let (first_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v1]:{[full-test]->[cargo]}");
    let (second_root, _) = experience_root_and_closure(&mut pangine, "?[policy-v2]:{[full-test]->[cli-runner]}");
    let mut state = RootRevisionState::default();
    state.learn(&first_root);
    state.correct(&first_root, &second_root);
    state.correct(&second_root, &first_root);

    assert!(state.active_roots().is_empty());
    assert_eq!(state.correction_cycle_nodes(), BTreeSet::from([first_root, second_root]));
}
