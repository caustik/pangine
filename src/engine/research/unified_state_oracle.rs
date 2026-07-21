//! Bounded, test-only prototypes for inspecting projection alternatives and
//! reducing Concept-native support across state partitions.
//!
//! This module deliberately favors explicit enumeration and readable
//! falsifiers over production efficiency. Its occurrence encoding, supported
//! question shape, and numeric oracle are research assumptions rather than
//! accepted Pangine semantics.

use super::super::*;

#[derive(Clone)]
struct ProjectionAlternative {
    weight: f64,
    bindings: Vec<(ConceptId, ConceptId)>,
    exact: bool,
}

impl ProjectionAlternative {
    fn wildcard() -> Self {
        Self { weight: 1.0, bindings: Vec::new(), exact: false }
    }

    fn exact() -> Self {
        Self { weight: 1.0, bindings: Vec::new(), exact: true }
    }

    fn binding(percept: ConceptId, candidate: ConceptId) -> Self {
        Self { weight: 1.0, bindings: vec![(percept, candidate)], exact: true }
    }

    fn multiply(&self, other: &Self) -> Self {
        let mut bindings = self.bindings.clone();
        bindings.extend(other.bindings.iter().cloned());
        Self { weight: self.weight * other.weight, bindings, exact: self.exact && other.exact }
    }

    fn scale(mut self, scale: f64) -> Self {
        self.weight *= scale;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ContextObservation {
    source: ConceptId,
    context: ConceptId,
    candidate: ConceptId,
}

fn must_reference(pangine: &mut Pangine, script: &str) -> ConceptId {
    pangine.reference_concept(script).unwrap().unwrap_or_else(|| panic!("expected a concept from {script:?}"))
}

fn projection_alternatives(pangine: &Pangine, experience: &ConceptId, question: &ConceptId) -> Vec<ProjectionAlternative> {
    if pangine.is_percept(question) {
        return vec![ProjectionAlternative::wildcard(), ProjectionAlternative::binding(question.clone(), experience.clone())];
    }

    let mut alternatives = vec![ProjectionAlternative::wildcard()];
    let preserved = if let (ConceptKind::Named(experience_name), ConceptKind::Named(question_name)) = (&experience.0.kind, &question.0.kind) {
        (experience_name == question_name).then(|| vec![ProjectionAlternative::exact()])
    } else if let (Some((experience_kind, experience_a, experience_b)), Some((question_kind, question_a, question_b))) =
        (experience.0.relation(), question.0.relation())
    {
        (experience_kind == question_kind).then(|| {
            multiply_alternatives(&projection_alternatives(pangine, experience_a, question_a), &projection_alternatives(pangine, experience_b, question_b))
        })
    } else if experience.0.shape() == ConceptShape::Unordered && question.0.shape() == ConceptShape::Unordered {
        unordered_preserved_alternatives(pangine, experience, question)
    } else {
        None
    };

    if let Some(preserved) = preserved {
        alternatives.extend(preserved);
    }
    alternatives
}

fn multiply_alternatives(left: &[ProjectionAlternative], right: &[ProjectionAlternative]) -> Vec<ProjectionAlternative> {
    left.iter().flat_map(|left| right.iter().map(move |right| left.multiply(right))).collect()
}

fn unordered_preserved_alternatives(pangine: &Pangine, experience: &ConceptId, question: &ConceptId) -> Option<Vec<ProjectionAlternative>> {
    let experiences = experience.0.subconcepts.iter().collect::<Vec<_>>();
    let questions = question.0.subconcepts.iter().collect::<Vec<_>>();
    if experiences.len() != questions.len() {
        return None;
    }

    let mut alternatives = Vec::new();
    let mut used_experiences = vec![false; experiences.len()];
    collect_unordered_alternatives(pangine, &experiences, &questions, 0, &mut used_experiences, ProjectionAlternative::exact(), &mut alternatives);
    Some(alternatives)
}

fn collect_unordered_alternatives(
    pangine: &Pangine,
    experiences: &[(&ConceptId, &Relevance)],
    questions: &[(&ConceptId, &Relevance)],
    question_index: usize,
    used_experiences: &mut [bool],
    current: ProjectionAlternative,
    alternatives: &mut Vec<ProjectionAlternative>,
) {
    if question_index == questions.len() {
        alternatives.push(current);
        return;
    }

    let (question, question_relevance) = questions[question_index];
    for (experience_index, (experience, experience_relevance)) in experiences.iter().enumerate() {
        if used_experiences[experience_index] {
            continue;
        }

        used_experiences[experience_index] = true;
        let scale = (experience_relevance.weight() * question_relevance.weight()) as f64;
        for edge in projection_alternatives(pangine, experience, question) {
            collect_unordered_alternatives(
                pangine,
                experiences,
                questions,
                question_index + 1,
                used_experiences,
                current.multiply(&edge.scale(scale)),
                alternatives,
            );
        }
        used_experiences[experience_index] = false;
    }
}

fn fold_projection_alternatives(alternatives: &[ProjectionAlternative]) -> ProjectionSummary {
    let mut summary = ProjectionSummary { total: 0.0, bindings: ProjectionBindingWeights::new() };
    for alternative in alternatives {
        summary.total += alternative.weight;
        for (percept, candidate) in &alternative.bindings {
            *summary.bindings.entry(percept.clone()).or_default().entry(candidate.clone()).or_default() += alternative.weight;
        }
    }
    summary
}

fn assert_projection_parity(pangine: &Pangine, experience: &ConceptId, question: &ConceptId) {
    let expected = pangine.projection_summary(experience, question, &mut ProjectionCache::new());
    let actual = fold_projection_alternatives(&projection_alternatives(pangine, experience, question));
    assert!((actual.total - expected.total).abs() < f64::EPSILON);
    assert_eq!(actual.bindings.keys().collect::<Vec<_>>(), expected.bindings.keys().collect::<Vec<_>>());

    for (percept, expected_candidates) in expected.bindings {
        let actual_candidates = &actual.bindings[&percept];
        assert_eq!(actual_candidates.keys().collect::<Vec<_>>(), expected_candidates.keys().collect::<Vec<_>>());
        for (candidate, expected_weight) in expected_candidates {
            assert!((actual_candidates[&candidate] - expected_weight).abs() < f64::EPSILON);
        }
    }
}

fn encode_occurrence_state(pangine: &mut Pangine, occurrences: &[(ConceptId, ConceptId)]) -> Result<Option<ConceptId>, &'static str> {
    let mut sources = BTreeMap::<ConceptId, ConceptId>::new();
    for (source, root) in occurrences {
        if !pangine.owns(source) || !pangine.owns(root) {
            return Err("foreign concept");
        }

        match sources.get(source) {
            Some(existing) if existing != root => return Err("one source identifies two roots"),
            Some(_) => {}
            None => {
                sources.insert(source.clone(), root.clone());
            }
        }
    }

    let records = sources.into_iter().map(|(source, root)| (pangine.reference_observation(source, root), Relevance::DEFAULT)).collect::<ConceptMap>();
    Ok(pangine.reference_map(&records))
}

fn decode_occurrence_state(state: &ConceptId) -> Result<BTreeMap<ConceptId, ConceptId>, &'static str> {
    let records = if matches!(state.0.kind, ConceptKind::Observation { .. }) {
        vec![(state, Relevance::DEFAULT)]
    } else if matches!(state.0.kind, ConceptKind::Anonymous) {
        state.0.subconcepts.iter().map(|(record, &relevance)| (record, relevance)).collect()
    } else {
        return Err("occurrence state is not source-keyed");
    };

    let mut sources = BTreeMap::new();
    for (record, relevance) in records {
        if relevance != Relevance::DEFAULT {
            return Err("source records have structural relevance");
        }
        let ConceptKind::Observation { observer: source, observation: root } = &record.0.kind else {
            return Err("occurrence state contains a non-record");
        };
        match sources.get(source) {
            Some(existing) if existing != root => return Err("one source identifies two roots"),
            Some(_) => {}
            None => {
                sources.insert(source.clone(), root.clone());
            }
        }
    }
    Ok(sources)
}

fn collect_context_observations(pangine: &Pangine, state: &ConceptId, question: &ConceptId) -> Result<Vec<ContextObservation>, &'static str> {
    let occurrences = decode_occurrence_state(state)?;
    let mut patterns = ConceptMap::new();
    pangine.collect_question_patterns(question, Relevance::DEFAULT, true, &mut patterns, &mut BTreeMap::new());

    let mut observations = BTreeSet::new();
    for (source, root) in occurrences {
        let mut source_nodes = BTreeSet::new();
        collect_nodes(&root, &mut source_nodes);
        let mut matches = BTreeMap::<ConceptId, BTreeSet<ConceptId>>::new();

        for context in patterns.keys() {
            if percept_occurrence_count(pangine, context) != 1 {
                return Err("oracle supports one output occurrence");
            }

            for experience in &source_nodes {
                for alternative in projection_alternatives(pangine, experience, context) {
                    if alternative.exact && alternative.bindings.len() == 1 {
                        let (_, candidate) = &alternative.bindings[0];
                        matches.entry(candidate.clone()).or_default().insert(context.clone());
                    }
                }
            }
        }

        for (candidate, contexts) in matches {
            for context in &contexts {
                let shadowed = contexts.iter().any(|other| other != context && contains_node(other, context, &mut BTreeSet::new()));
                if !shadowed {
                    observations.insert(ContextObservation { source: source.clone(), context: context.clone(), candidate: candidate.clone() });
                }
            }
        }
    }

    Ok(observations.into_iter().collect())
}

fn collect_nodes(concept: &ConceptId, nodes: &mut BTreeSet<ConceptId>) {
    if !nodes.insert(concept.clone()) {
        return;
    }
    for (child, _) in concept.0.children() {
        collect_nodes(child, nodes);
    }
}

fn percept_occurrence_count(pangine: &Pangine, concept: &ConceptId) -> usize {
    if pangine.is_percept(concept) {
        return 1;
    }
    concept.0.children().map(|(child, _)| percept_occurrence_count(pangine, child)).sum()
}

fn contains_node(outer: &ConceptId, inner: &ConceptId, visited: &mut BTreeSet<ConceptId>) -> bool {
    if outer == inner {
        return true;
    }
    if !visited.insert(outer.clone()) {
        return false;
    }
    outer.0.children().any(|(child, _)| contains_node(child, inner, visited))
}

fn encode_support_state(pangine: &mut Pangine, observations: &[ContextObservation]) -> Option<ConceptId> {
    let records = observations
        .iter()
        .map(|observation| {
            let support = pangine.reference_correlation(observation.context.clone(), observation.candidate.clone());
            (pangine.reference_observation(observation.source.clone(), support), Relevance::DEFAULT)
        })
        .collect::<ConceptMap>();
    pangine.reference_map(&records)
}

fn fold_support_state(pangine: &mut Pangine, state: &ConceptId, question: &ConceptId) -> Result<Option<ConceptId>, &'static str> {
    let observations = collect_context_observations(pangine, state, question)?;
    Ok(encode_support_state(pangine, &observations))
}

fn reduce_support_states(pangine: &mut Pangine, partials: &[Option<ConceptId>]) -> Option<ConceptId> {
    let mut records = ConceptMap::new();
    for partial in partials.iter().flatten() {
        pangine.add_merge_concept(&mut records, partial.clone(), false, Relevance::DEFAULT);
    }
    pangine.reference_map(&records)
}

fn candidate_sources(observations: &[ContextObservation], candidate: &ConceptId, context: &ConceptId, include_specializations: bool) -> BTreeSet<ConceptId> {
    observations
        .iter()
        .filter(|observation| {
            observation.candidate == *candidate
                && (observation.context == *context || (include_specializations && contains_node(&observation.context, context, &mut BTreeSet::new())))
        })
        .map(|observation| observation.source.clone())
        .collect()
}

fn two_level_single_child_predictive_oracle(
    observations: &[ContextObservation],
    frame: &[ConceptId],
    candidate: &ConceptId,
    general_context: &ConceptId,
    specific_context: &ConceptId,
    parent_concentration: f64,
    child_concentration: f64,
) -> f64 {
    let parent_counts = frame.iter().map(|candidate| candidate_sources(observations, candidate, general_context, true).len() as f64).collect::<Vec<_>>();
    let child_counts = frame.iter().map(|candidate| candidate_sources(observations, candidate, specific_context, true).len() as f64).collect::<Vec<_>>();
    let candidate_index = frame.iter().position(|current| current == candidate).unwrap();
    let parent_total = parent_counts.iter().sum::<f64>();
    let child_total = child_counts.iter().sum::<f64>();
    let base_rate = 1.0 / frame.len() as f64;
    let parent_mean = (parent_concentration * base_rate + parent_counts[candidate_index]) / (parent_concentration + parent_total);
    (child_counts[candidate_index] + child_concentration * parent_mean) / (child_concentration + child_total)
}

#[test]
fn enumerated_matcher_cells_fold_back_to_the_current_projection_summary() {
    let mut pangine = Pangine::new();
    for (experience, question) in [
        ("[A]*[B]", "['X']*[B]"),
        ("{[C]->[A]}*{[B]->[D]}", "{['X']->[A]}*{[B]->[D]}"),
        ("{[E]->[A]}*{[P]->[Q]}", "{['X']->[A]}*{[B]->[D]}"),
        ("<x2[A], [B]>", "['X']*[B]"),
    ] {
        let experience = must_reference(&mut pangine, experience);
        let question = must_reference(&mut pangine, question);
        assert_projection_parity(&pangine, &experience, &question);
    }
}

#[test]
fn one_recursive_concept_can_preserve_source_and_structural_occurrence_boundaries() {
    let mut pangine = Pangine::new();
    let source_a = must_reference(&mut pangine, "[source-a]");
    let source_b = must_reference(&mut pangine, "[source-b]");
    let a = must_reference(&mut pangine, "[A]");
    let b = must_reference(&mut pangine, "[B]");
    let structural_repeat = must_reference(&mut pangine, "<x2[A]>");

    let one_structural_source = encode_occurrence_state(&mut pangine, &[(source_a.clone(), structural_repeat)]).unwrap().unwrap();
    let two_empirical_sources = encode_occurrence_state(&mut pangine, &[(source_a.clone(), a.clone()), (source_b.clone(), a.clone())]).unwrap().unwrap();
    assert_ne!(one_structural_source, two_empirical_sources);

    let once = encode_occurrence_state(&mut pangine, &[(source_a.clone(), a.clone())]).unwrap();
    let duplicate_delivery = encode_occurrence_state(&mut pangine, &[(source_a.clone(), a.clone()), (source_a.clone(), a)]).unwrap();
    assert_eq!(once, duplicate_delivery);
    assert!(encode_occurrence_state(&mut pangine, &[(source_a.clone(), b.clone()), (source_a, b.clone())]).is_ok());

    let source = must_reference(&mut pangine, "[conflicting-source]");
    let conflicting_a = must_reference(&mut pangine, "[A]");
    assert_eq!(encode_occurrence_state(&mut pangine, &[(source.clone(), conflicting_a), (source, b)]), Err("one source identifies two roots"));
}

#[test]
fn source_scoped_observations_remove_generic_swamping_without_parallel_state() {
    let mut pangine = Pangine::new();
    let mut occurrences = Vec::new();
    let mut legacy_generic_weight = 0.0;
    let question = must_reference(&mut pangine, "['X']*[B]");
    let generic = must_reference(&mut pangine, "[B]");

    for index in 0..8 {
        let source = must_reference(&mut pangine, &format!("[source-{index}]"));
        let root = must_reference(&mut pangine, &format!("[P{index}]*[B]"));
        let summary = fold_projection_alternatives(&projection_alternatives(&pangine, &root, &question));
        legacy_generic_weight += summary.bindings.values().map(|candidates| candidates.get(&generic).copied().unwrap_or_default()).sum::<f64>();
        occurrences.push((source, root));
    }
    assert_eq!(legacy_generic_weight, 8.0);

    let occurrence_state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let observations = collect_context_observations(&pangine, &occurrence_state, &question).unwrap();
    assert_eq!(observations.len(), 8);
    assert!(observations.iter().all(|observation| observation.context == question));
    assert!(observations.iter().all(|observation| observation.candidate != generic));

    let baseline_count = pangine.concept_count();
    let support_state = encode_support_state(&mut pangine, &observations).unwrap();
    assert!(pangine.concept_count() > baseline_count);
    drop(support_state);
    assert_eq!(pangine.concept_count(), baseline_count);
}

#[test]
fn source_identity_deduplicates_paths_and_delivery_but_not_independent_occurrences() {
    let mut pangine = Pangine::new();
    let source_a = must_reference(&mut pangine, "[source-a]");
    let source_b = must_reference(&mut pangine, "[source-b]");
    let repeated_subtree = must_reference(&mut pangine, "{[E]->[A]}*{{[E]->[A]}->[Z]}");
    let question = must_reference(&mut pangine, "{['X']->[A]}");
    let e = must_reference(&mut pangine, "[E]");

    let once = encode_occurrence_state(&mut pangine, &[(source_a.clone(), repeated_subtree.clone())]).unwrap().unwrap();
    let duplicate_delivery =
        encode_occurrence_state(&mut pangine, &[(source_a.clone(), repeated_subtree.clone()), (source_a.clone(), repeated_subtree.clone())]).unwrap().unwrap();
    assert_eq!(once, duplicate_delivery);

    let independent =
        encode_occurrence_state(&mut pangine, &[(source_a.clone(), repeated_subtree.clone()), (source_b.clone(), repeated_subtree.clone())]).unwrap().unwrap();
    let reversed = encode_occurrence_state(&mut pangine, &[(source_b, repeated_subtree.clone()), (source_a, repeated_subtree)]).unwrap().unwrap();
    assert_eq!(independent, reversed);

    let once_observations = collect_context_observations(&pangine, &once, &question).unwrap();
    let independent_observations = collect_context_observations(&pangine, &independent, &question).unwrap();
    assert_eq!(once_observations.len(), 1);
    assert_eq!(independent_observations.len(), 2);
    assert!(independent_observations.iter().all(|observation| observation.context == question && observation.candidate == e));
}

#[test]
fn concept_native_support_fold_is_partition_independent_for_disjoint_sources() {
    let mut pangine = Pangine::new();
    let source_a = must_reference(&mut pangine, "[source-a]");
    let source_b = must_reference(&mut pangine, "[source-b]");
    let source_c = must_reference(&mut pangine, "[source-c]");
    let repeated_subtree = must_reference(&mut pangine, "{[E]->[A]}*{{[E]->[A]}->[Z]}");
    let c_root = must_reference(&mut pangine, "{[C]->[A]}");
    let question = must_reference(&mut pangine, "{['X']->[A]}");
    let c = must_reference(&mut pangine, "[C]");
    let e = must_reference(&mut pangine, "[E]");

    let occurrences = [(source_a, repeated_subtree.clone()), (source_b, c_root), (source_c, repeated_subtree)];
    let combined_state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let combined_observations = collect_context_observations(&pangine, &combined_state, &question).unwrap();
    assert_eq!(candidate_sources(&combined_observations, &c, &question, false).len(), 1);
    assert_eq!(candidate_sources(&combined_observations, &e, &question, false).len(), 2);
    let combined_support = encode_support_state(&mut pangine, &combined_observations);

    for partitions in [
        vec![vec![occurrences[0].clone()], vec![occurrences[1].clone(), occurrences[2].clone()]],
        vec![vec![occurrences[2].clone()], vec![occurrences[0].clone()], vec![occurrences[1].clone()]],
        vec![vec![occurrences[1].clone(), occurrences[0].clone()], vec![occurrences[2].clone()]],
    ] {
        let partials = partitions
            .iter()
            .map(|partition| {
                let state = encode_occurrence_state(&mut pangine, partition).unwrap().unwrap();
                fold_support_state(&mut pangine, &state, &question).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(reduce_support_states(&mut pangine, &partials), combined_support);
    }

    let source_partials = occurrences
        .iter()
        .map(|occurrence| {
            let state = encode_occurrence_state(&mut pangine, std::slice::from_ref(occurrence)).unwrap().unwrap();
            fold_support_state(&mut pangine, &state, &question).unwrap()
        })
        .collect::<Vec<_>>();
    let left_pair = reduce_support_states(&mut pangine, &source_partials[..2]);
    let left_grouped = reduce_support_states(&mut pangine, &[left_pair, source_partials[2].clone()]);
    let right_pair = reduce_support_states(&mut pangine, &source_partials[1..]);
    let right_grouped = reduce_support_states(&mut pangine, &[source_partials[0].clone(), right_pair]);
    assert_eq!(left_grouped, combined_support);
    assert_eq!(right_grouped, combined_support);
    assert_eq!(reduce_support_states(&mut pangine, &[None, combined_support.clone()]), combined_support);
}

#[test]
fn replaying_a_partial_support_concept_currently_becomes_structural_multiplicity() {
    let mut pangine = Pangine::new();
    let source = must_reference(&mut pangine, "[source]");
    let root = must_reference(&mut pangine, "{[E]->[A]}");
    let question = must_reference(&mut pangine, "{['X']->[A]}");
    let state = encode_occurrence_state(&mut pangine, &[(source, root)]).unwrap().unwrap();
    let partial = fold_support_state(&mut pangine, &state, &question).unwrap().unwrap();

    let reduced_once = reduce_support_states(&mut pangine, &[Some(partial.clone())]).unwrap();
    let reduced_replay = reduce_support_states(&mut pangine, &[Some(partial.clone()), Some(partial)]).unwrap();

    assert_ne!(reduced_once, reduced_replay);
    assert_eq!(reduced_replay.0.subconcepts.values().copied().collect::<Vec<_>>(), vec![Relevance::new(1.0, 2.0)]);
    assert_eq!(pangine.format_concept(&reduced_replay, false), "<x2?[source]:{{['X']->[A]}->[E]}>");
}

#[test]
fn maximal_recursive_contexts_preserve_the_full_vs_partial_crossover() {
    let mut pangine = Pangine::new();
    let complete_source = must_reference(&mut pangine, "[complete-source]");
    let complete_root = must_reference(&mut pangine, "{[C]->[A]}*{[B]->[D]}");
    let question = must_reference(&mut pangine, "{['X']->[A]}*{[B]->[D]}");
    let general_context = must_reference(&mut pangine, "{['X']->[A]}");
    let c = must_reference(&mut pangine, "[C]");
    let e = must_reference(&mut pangine, "[E]");

    let mut occurrences = vec![(complete_source, complete_root)];
    let mut partial_sources = Vec::new();
    for index in 1..=3 {
        let source = must_reference(&mut pangine, &format!("[partial-source-{index}]"));
        let root = must_reference(&mut pangine, &format!("{{[E]->[A]}}*{{[P{index}]->[Q{index}]}}"));
        partial_sources.push(source.clone());
        occurrences.push((source, root));
    }

    let occurrence_state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let observations = collect_context_observations(&pangine, &occurrence_state, &question).unwrap();
    assert_eq!(candidate_sources(&observations, &c, &question, false).len(), 1);
    assert_eq!(candidate_sources(&observations, &e, &general_context, false).len(), 3);
    assert!(encode_support_state(&mut pangine, &observations).is_some());

    let frame = [c.clone(), e.clone()];
    for count in 1..=3 {
        let selected_sources = partial_sources.iter().take(count).cloned().collect::<BTreeSet<_>>();
        let selected =
            observations.iter().filter(|observation| observation.candidate == c || selected_sources.contains(&observation.source)).cloned().collect::<Vec<_>>();
        let c_predictive = two_level_single_child_predictive_oracle(&selected, &frame, &c, &general_context, &question, 2.0, 5.0);
        let e_predictive = two_level_single_child_predictive_oracle(&selected, &frame, &e, &general_context, &question, 2.0, 5.0);

        match count {
            1 => assert!(c_predictive > e_predictive),
            2 => assert!((c_predictive - e_predictive).abs() < f64::EPSILON),
            3 => assert!(e_predictive > c_predictive),
            _ => unreachable!(),
        }
    }
}
