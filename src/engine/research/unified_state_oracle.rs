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

#[derive(Clone, Debug)]
struct BackoffProfile {
    path: Vec<ConceptId>,
    predictive: BTreeMap<ConceptId, f64>,
}

struct BackoffEvaluator<'a> {
    pangine: &'a Pangine,
    contexts: &'a BTreeSet<ConceptId>,
    observations: &'a [ContextObservation],
    frame: &'a [ConceptId],
    background: &'a BTreeMap<ConceptId, f64>,
    concentration: f64,
    cache: BTreeMap<ConceptId, Vec<BackoffProfile>>,
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
            let b = projection_alternatives(pangine, experience_b, question_b);
            match (experience_a, question_a) {
                (Some(experience_a), Some(question_a)) => multiply_alternatives(&projection_alternatives(pangine, experience_a, question_a), &b),
                (None, None) => b,
                _ => Vec::new(),
            }
        })
    } else if experience.0.shape() == question.0.shape() && matches!(experience.0.shape(), ConceptShape::Relevance | ConceptShape::ObservationSet) {
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

fn fold_projection_alternatives(alternatives: &[ProjectionAlternative], shared_percepts: &BTreeSet<ConceptId>) -> ProjectionSummary {
    let mut profiles = ProjectionProfiles::new();
    for alternative in alternatives {
        let mut shared_bindings = ProjectionSharedBindings::new();
        let mut bindings = ProjectionBindingWeights::new();
        let mut compatible = true;
        for (percept, candidate) in &alternative.bindings {
            if shared_percepts.contains(percept) {
                match shared_bindings.get(percept) {
                    Some(current) if current != candidate => {
                        compatible = false;
                        break;
                    }
                    Some(_) => {}
                    None => {
                        shared_bindings.insert(percept.clone(), candidate.clone());
                    }
                }
            } else {
                *bindings.entry(percept.clone()).or_default().entry(candidate.clone()).or_default() += alternative.weight;
            }
        }
        if compatible {
            ProjectionSummary::accumulate_profile(&mut profiles, shared_bindings, ProjectionProfile { total: alternative.weight, bindings });
        }
    }
    ProjectionSummary::from_profiles(profiles)
}

fn assert_projection_parity(pangine: &Pangine, experience: &ConceptId, question: &ConceptId) {
    let shared_percepts = pangine.shared_output_percepts(question);
    let expected = pangine.projection_summary(experience, question, &shared_percepts, &mut ProjectionCache::new());
    let actual = fold_projection_alternatives(&projection_alternatives(pangine, experience, question), &shared_percepts);
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
    Ok(pangine.reference_observation_set(&records))
}

fn decode_occurrence_state(state: &ConceptId) -> Result<BTreeMap<ConceptId, ConceptId>, &'static str> {
    let records = state.observation_records().ok_or("occurrence state is not source-keyed")?;

    let mut sources = BTreeMap::new();
    for (record, relevance) in records {
        if relevance != Relevance::DEFAULT {
            return Err("source records have structural relevance");
        }
        let ConceptKind::Observation { observer: Some(source), observation: root } = &record.0.kind else {
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

fn unordered_context_signature(pangine: &Pangine, context: &ConceptId) -> Option<(ConceptMap, ConceptId)> {
    let ConceptKind::Correlation { a: fixed, b: output } = &context.0.kind else {
        return None;
    };
    if !pangine.is_percept(output) || percept_occurrence_count(pangine, fixed) != 0 {
        return None;
    }

    let members =
        if matches!(fixed.0.kind, ConceptKind::Relevance) { fixed.0.subconcepts.clone() } else { ConceptMap::from([(fixed.clone(), Relevance::DEFAULT)]) };
    Some((members, output.clone()))
}

fn context_subsumes(pangine: &Pangine, outer: &ConceptId, inner: &ConceptId) -> bool {
    if contains_node(outer, inner, &mut BTreeSet::new()) {
        return true;
    }

    let (Some((outer_members, outer_output)), Some((inner_members, inner_output))) =
        (unordered_context_signature(pangine, outer), unordered_context_signature(pangine, inner))
    else {
        return false;
    };
    outer_output == inner_output && inner_members.iter().all(|(member, relevance)| outer_members.get(member) == Some(relevance))
}

fn collect_flat_unordered_context_observations(
    pangine: &mut Pangine,
    state: &ConceptId,
    question: &ConceptId,
) -> Result<Vec<ContextObservation>, &'static str> {
    let occurrences = decode_occurrence_state(state)?;
    let ConceptKind::Correlation { a: question_fixed, b: output } = &question.0.kind else {
        return Err("flat unordered question is not a correlation");
    };
    let question_fixed = question_fixed.clone();
    let output = output.clone();
    if !pangine.is_percept(&output) || percept_occurrence_count(pangine, &question_fixed) != 0 {
        return Err("flat unordered question must have one direct output");
    }
    let question_members = if matches!(question_fixed.0.kind, ConceptKind::Relevance) {
        question_fixed.0.subconcepts.clone()
    } else {
        ConceptMap::from([(question_fixed, Relevance::DEFAULT)])
    };

    let mut observations = BTreeSet::new();
    for (source, root) in occurrences {
        let ConceptKind::Correlation { a: experience_fixed, b: candidate } = &root.0.kind else {
            return Err("flat unordered experience is not a correlation");
        };
        if percept_occurrence_count(pangine, experience_fixed) != 0 {
            return Err("flat unordered experience context contains an output");
        }
        let experience_members = if matches!(experience_fixed.0.kind, ConceptKind::Relevance) {
            experience_fixed.0.subconcepts.clone()
        } else {
            ConceptMap::from([(experience_fixed.clone(), Relevance::DEFAULT)])
        };
        let matched = question_members
            .iter()
            .filter(|(member, relevance)| experience_members.get(*member) == Some(*relevance))
            .map(|(member, relevance)| (member.clone(), *relevance))
            .collect::<ConceptMap>();
        let Some(matched_fixed) = pangine.reference_map(&matched) else {
            continue;
        };
        let context = pangine.reference_correlation(matched_fixed, output.clone());
        observations.insert(ContextObservation { source, context, candidate: candidate.clone() });
    }
    Ok(observations.into_iter().collect())
}

fn encode_support_state(pangine: &mut Pangine, observations: &[ContextObservation]) -> Option<ConceptId> {
    let records = observations
        .iter()
        .map(|observation| {
            let support = pangine.reference_correlation(observation.context.clone(), observation.candidate.clone());
            (pangine.reference_observation(observation.source.clone(), support), Relevance::DEFAULT)
        })
        .collect::<ConceptMap>();
    pangine.reference_observation_set(&records)
}

fn decode_support_state(state: &ConceptId) -> Result<Vec<ContextObservation>, &'static str> {
    let records = state.observation_records().ok_or("support state is not source-keyed")?;
    let mut observations = BTreeSet::new();
    for (record, relevance) in records {
        if relevance != Relevance::DEFAULT {
            return Err("support records have structural relevance");
        }
        let ConceptKind::Observation { observer: Some(source), observation: support } = &record.0.kind else {
            return Err("support state contains a non-record");
        };
        let ConceptKind::Correlation { a: context, b: candidate } = &support.0.kind else {
            return Err("support record payload is not context-to-candidate");
        };
        observations.insert(ContextObservation { source: source.clone(), context: context.clone(), candidate: candidate.clone() });
    }
    Ok(observations.into_iter().collect())
}

fn fold_support_state(pangine: &mut Pangine, state: &ConceptId, question: &ConceptId) -> Result<Option<ConceptId>, &'static str> {
    let observations = collect_context_observations(pangine, state, question)?;
    Ok(encode_support_state(pangine, &observations))
}

fn reduce_support_states(pangine: &mut Pangine, partials: &[Option<ConceptId>]) -> Option<ConceptId> {
    let mut records = ConceptMap::new();
    for partial in partials.iter().flatten() {
        for (record, relevance) in partial.observation_records()? {
            if relevance != Relevance::DEFAULT {
                return None;
            }
            records.entry(record).or_insert(Relevance::DEFAULT);
        }
    }
    pangine.reference_observation_set(&records)
}

fn candidate_sources(observations: &[ContextObservation], candidate: &ConceptId, context: &ConceptId) -> BTreeSet<ConceptId> {
    observations
        .iter()
        .filter(|observation| observation.candidate == *candidate && observation.context == *context)
        .map(|observation| observation.source.clone())
        .collect()
}

impl BackoffEvaluator<'_> {
    fn profiles_for_context(&mut self, context: &ConceptId) -> Vec<BackoffProfile> {
        if let Some(profiles) = self.cache.get(context) {
            return profiles.clone();
        }

        let parents = immediate_backoff_parents(self.pangine, context, self.contexts);
        let mut profiles = if parents.is_empty() {
            vec![BackoffProfile { path: Vec::new(), predictive: self.background.clone() }]
        } else {
            let mut profiles = Vec::new();
            for parent in parents {
                profiles.extend(self.profiles_for_context(&parent));
            }
            profiles
        };
        let counts = self.frame.iter().map(|candidate| candidate_sources(self.observations, candidate, context).len() as f64).collect::<Vec<_>>();
        let total = counts.iter().sum::<f64>();
        for profile in &mut profiles {
            profile.path.push(context.clone());
            profile.predictive = self
                .frame
                .iter()
                .zip(&counts)
                .map(|(candidate, count)| {
                    let probability = (count + self.concentration * profile.predictive[candidate]) / (total + self.concentration);
                    (candidate.clone(), probability)
                })
                .collect();
        }
        self.cache.insert(context.clone(), profiles.clone());
        profiles
    }
}

fn partial_order_backoff_profiles(
    pangine: &Pangine,
    observations: &[ContextObservation],
    frame: &[ConceptId],
    target: &ConceptId,
    background: &BTreeMap<ConceptId, f64>,
    concentration: f64,
) -> Result<Vec<BackoffProfile>, &'static str> {
    if frame.is_empty() {
        return Err("candidate frame is empty");
    }
    if !concentration.is_finite() || concentration <= 0.0 {
        return Err("backoff concentration must be positive and finite");
    }
    if frame.iter().collect::<BTreeSet<_>>().len() != frame.len() {
        return Err("candidate frame contains duplicates");
    }

    let candidates = frame.iter().cloned().collect::<BTreeSet<_>>();
    if background.len() != frame.len()
        || frame.iter().any(|candidate| !background.contains_key(candidate))
        || background.values().any(|probability| !probability.is_finite() || *probability < 0.0)
        || (background.values().sum::<f64>() - 1.0).abs() > 1e-12
    {
        return Err("candidate background is not a probability distribution over the frame");
    }
    let eligible = observations
        .iter()
        .filter(|observation| candidates.contains(&observation.candidate) && context_subsumes(pangine, target, &observation.context))
        .cloned()
        .collect::<Vec<_>>();
    let contexts = eligible.iter().map(|observation| observation.context.clone()).collect::<BTreeSet<_>>();
    if contexts.is_empty() {
        return Ok(Vec::new());
    }

    let frontier = contexts
        .iter()
        .filter(|context| !contexts.iter().any(|other| other != *context && context_subsumes(pangine, other, context)))
        .cloned()
        .collect::<Vec<_>>();
    let mut evaluator = BackoffEvaluator { pangine, contexts: &contexts, observations: &eligible, frame, background, concentration, cache: BTreeMap::new() };
    let mut profiles = Vec::new();
    for context in frontier {
        profiles.extend(evaluator.profiles_for_context(&context));
    }
    if !contexts.contains(target) {
        for profile in &mut profiles {
            profile.path.push(target.clone());
        }
    }
    Ok(profiles)
}

fn immediate_backoff_parents(pangine: &Pangine, context: &ConceptId, contexts: &BTreeSet<ConceptId>) -> Vec<ConceptId> {
    let ancestors = contexts.iter().filter(|ancestor| *ancestor != context && context_subsumes(pangine, context, ancestor)).collect::<Vec<_>>();
    ancestors
        .iter()
        .filter(|ancestor| !ancestors.iter().any(|middle| middle != *ancestor && context_subsumes(pangine, middle, ancestor)))
        .map(|ancestor| (*ancestor).clone())
        .collect()
}

fn recursive_grammar_node_count(concept: &ConceptId) -> usize {
    1 + concept.0.children().map(|(child, _)| recursive_grammar_node_count(child)).sum::<usize>()
}

fn recursive_facet_sources(pangine: &Pangine, observations: &[ContextObservation]) -> Result<BTreeMap<ConceptId, BTreeSet<ConceptId>>, &'static str> {
    let mut sources = BTreeMap::<ConceptId, BTreeSet<ConceptId>>::new();
    for observation in observations {
        let Some((members, _)) = unordered_context_signature(pangine, &observation.context) else {
            return Err("support context is not an unordered recursive-facet context");
        };
        for (member, relevance) in members {
            if relevance != Relevance::DEFAULT {
                return Err("recursive facet has non-default relevance");
            }
            sources.entry(member).or_default().insert(observation.source.clone());
        }
    }
    Ok(sources)
}

#[test]
fn enumerated_matcher_cells_fold_back_to_the_current_projection_summary() {
    let mut pangine = Pangine::new();
    for (experience, question) in [
        ("[A]*[B]", "['X']*[B]"),
        ("{[C]->[A]}*{[B]->[D]}", "{['X']->[A]}*{[B]->[D]}"),
        ("{[E]->[A]}*{[P]->[Q]}", "{['X']->[A]}*{[B]->[D]}"),
        ("x2[A][B]", "['X']*[B]"),
        ("{[A]->[B]}", "{['X']->['X']}"),
        ("{([C]*[P])->([C]*[Q])}", "{(['X']*[P])->(['X']*[Q])}"),
        ("{[A]->[A]}*{[B]->[B]}", "{['X']->['X']}*{['Y']->['Y']}"),
        ("{[A]->{[A]->[C]}}", "{['X']->{['X']->['Y']}}"),
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
    let structural_repeat = must_reference(&mut pangine, "x2[A]");

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
    let shared_percepts = pangine.shared_output_percepts(&question);

    for index in 0..8 {
        let source = must_reference(&mut pangine, &format!("[source-{index}]"));
        let root = must_reference(&mut pangine, &format!("[P{index}]*[B]"));
        let summary = fold_projection_alternatives(&projection_alternatives(&pangine, &root, &question), &shared_percepts);
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
fn source_scoped_observations_separate_literal_support_from_a_generic_tie() {
    let mut pangine = Pangine::new();
    let question = must_reference(&mut pangine, "['X']*[B]");
    let a = must_reference(&mut pangine, "[A]");
    let b = must_reference(&mut pangine, "[B]");
    let c = must_reference(&mut pangine, "[C]");
    let shared_percepts = pangine.shared_output_percepts(&question);
    let occurrences = [
        (must_reference(&mut pangine, "[left-source]"), must_reference(&mut pangine, "[A]*[B]")),
        (must_reference(&mut pangine, "[right-source]"), must_reference(&mut pangine, "[B]*[C]")),
    ];

    let mut legacy_weights = BTreeMap::<ConceptId, f64>::new();
    for (_, root) in &occurrences {
        let summary = fold_projection_alternatives(&projection_alternatives(&pangine, root, &question), &shared_percepts);
        for candidates in summary.bindings.values() {
            for (candidate, weight) in candidates {
                *legacy_weights.entry(candidate.clone()).or_default() += weight;
            }
        }
    }
    assert_eq!(legacy_weights.get(&a), Some(&2.0));
    assert_eq!(legacy_weights.get(&b), Some(&2.0));
    assert_eq!(legacy_weights.get(&c), Some(&2.0));

    let occurrence_state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let observations = collect_context_observations(&pangine, &occurrence_state, &question).unwrap();
    assert_eq!(candidate_sources(&observations, &a, &question).len(), 1);
    assert_eq!(candidate_sources(&observations, &b, &question).len(), 0);
    assert_eq!(candidate_sources(&observations, &c, &question).len(), 1);
}

#[test]
fn nested_agent_memory_retains_conflicting_updates_until_correction_is_defined() {
    let mut pangine = Pangine::new();
    let source_v1 = must_reference(&mut pangine, "[policy-v1]");
    let source_v2 = must_reference(&mut pangine, "[policy-v2]");
    let cargo = must_reference(&mut pangine, "[cargo]");
    let cli_runner = must_reference(&mut pangine, "[cli-runner]");
    let old_root = must_reference(&mut pangine, "{({[repo]->[pangine]}*{[operation]->[test]}*{[scope]->[full]})->[cargo]}");
    let new_root = must_reference(&mut pangine, "{({[repo]->[pangine]}*{[operation]->[test]}*{[scope]->[full]})->[cli-runner]}");
    let question = must_reference(&mut pangine, "{({[repo]->[pangine]}*{[operation]->[test]}*{[scope]->[full]})->['route']}");
    let state = encode_occurrence_state(&mut pangine, &[(source_v1, old_root), (source_v2, new_root)]).unwrap().unwrap();

    let observations = collect_context_observations(&pangine, &state, &question).unwrap();
    assert_eq!(candidate_sources(&observations, &cargo, &question).len(), 1);
    assert_eq!(candidate_sources(&observations, &cli_runner, &question).len(), 1);
    assert!(encode_support_state(&mut pangine, &observations).is_some());
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
fn concept_native_support_fold_is_partition_independent_for_overlapping_sources() {
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
    assert_eq!(candidate_sources(&combined_observations, &c, &question).len(), 1);
    assert_eq!(candidate_sources(&combined_observations, &e, &question).len(), 2);
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
    assert_eq!(reduce_support_states(&mut pangine, &[source_partials[0].clone(), combined_support.clone(), source_partials[0].clone()]), combined_support);
    assert_eq!(decode_support_state(&combined_support.unwrap()).unwrap(), combined_observations);
}

#[test]
fn concept_native_support_replay_is_idempotent() {
    let mut pangine = Pangine::new();
    let source = must_reference(&mut pangine, "[source]");
    let root = must_reference(&mut pangine, "{[E]->[A]}");
    let question = must_reference(&mut pangine, "{['X']->[A]}");
    let state = encode_occurrence_state(&mut pangine, &[(source, root)]).unwrap().unwrap();
    let partial = fold_support_state(&mut pangine, &state, &question).unwrap().unwrap();

    let reduced_once = reduce_support_states(&mut pangine, &[Some(partial.clone())]).unwrap();
    let reduced_replay = reduce_support_states(&mut pangine, &[Some(partial.clone()), Some(partial)]).unwrap();

    assert_eq!(reduced_once, reduced_replay);
    assert_eq!(pangine.format_concept(&reduced_replay, false), "?[source]:{{['X']->[A]}->[E]}");
}

#[test]
fn flat_unordered_support_excludes_zero_matches_and_reduces_overlapping_partitions() {
    let mut pangine = Pangine::new();
    let question = must_reference(&mut pangine, "{({[f0]->[fixed]}*{[f1]->[fixed]})->['X']}");
    let occurrences = [
        (must_reference(&mut pangine, "[source-a]"), must_reference(&mut pangine, "{({[f0]->[fixed]}*{[noise]->[a]})->[C]}")),
        (must_reference(&mut pangine, "[source-b]"), must_reference(&mut pangine, "{({[f1]->[fixed]}*{[noise]->[b]})->[E]}")),
        (must_reference(&mut pangine, "[source-zero]"), must_reference(&mut pangine, "{{[noise]->[zero]}->[C]}")),
    ];
    let combined_state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let combined_observations = collect_flat_unordered_context_observations(&mut pangine, &combined_state, &question).unwrap();
    assert_eq!(combined_observations.len(), 2);
    assert!(combined_observations.iter().all(|observation| observation.source != occurrences[2].0));
    let combined_support = encode_support_state(&mut pangine, &combined_observations);

    let partitions = [vec![occurrences[0].clone(), occurrences[2].clone()], vec![occurrences[1].clone(), occurrences[0].clone()]];
    let partials = partitions
        .iter()
        .map(|partition| {
            let state = encode_occurrence_state(&mut pangine, partition).unwrap().unwrap();
            let observations = collect_flat_unordered_context_observations(&mut pangine, &state, &question).unwrap();
            encode_support_state(&mut pangine, &observations)
        })
        .collect::<Vec<_>>();
    assert_eq!(reduce_support_states(&mut pangine, &partials), combined_support);
}

#[test]
fn recursive_facets_separate_domain_units_from_grammar_depth_and_direction() {
    let mut pangine = Pangine::new();
    let source_a = must_reference(&mut pangine, "[source-a]");
    let source_b = must_reference(&mut pangine, "[source-b]");
    let c = must_reference(&mut pangine, "[C]");
    let e = must_reference(&mut pangine, "[E]");
    let question = must_reference(&mut pangine, "{({[direct]->[value]}*{[nested]->{[slot]->[value]}}*{{[namespace]->[two-sided]}->{[slot]->[value]}})->['X']}");
    let occurrences = [
        (source_a.clone(), must_reference(&mut pangine, "{({[direct]->[value]}*{[nested]->{[slot]->[value]}})->[C]}")),
        (source_b.clone(), must_reference(&mut pangine, "{({[direct]->[value]}*{[value]->[nested]}*{{[namespace]->[two-sided]}->{[slot]->[value]}})->[E]}")),
    ];
    let direct = must_reference(&mut pangine, "{[direct]->[value]}");
    let nested = must_reference(&mut pangine, "{[nested]->{[slot]->[value]}}");
    let two_sided = must_reference(&mut pangine, "{{[namespace]->[two-sided]}->{[slot]->[value]}}");
    let reversed = must_reference(&mut pangine, "{[value]->[nested]}");

    assert_eq!(recursive_grammar_node_count(&direct), 3);
    assert_eq!(recursive_grammar_node_count(&nested), 5);
    assert_eq!(recursive_grammar_node_count(&two_sided), 7);

    let state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let observations = collect_flat_unordered_context_observations(&mut pangine, &state, &question).unwrap();
    assert_eq!(observations.len(), 2);

    let c_context = &observations.iter().find(|observation| observation.candidate == c).unwrap().context;
    let e_context = &observations.iter().find(|observation| observation.candidate == e).unwrap().context;
    let c_members = unordered_context_signature(&pangine, c_context).unwrap().0;
    let e_members = unordered_context_signature(&pangine, e_context).unwrap().0;
    assert_eq!(c_members.keys().cloned().collect::<BTreeSet<_>>(), BTreeSet::from([direct.clone(), nested.clone()]));
    assert_eq!(e_members.keys().cloned().collect::<BTreeSet<_>>(), BTreeSet::from([direct.clone(), two_sided.clone()]));
    assert!(!c_members.contains_key(&reversed));
    assert!(!e_members.contains_key(&reversed));

    let facet_sources = recursive_facet_sources(&pangine, &observations).unwrap();
    assert_eq!(facet_sources[&direct], BTreeSet::from([source_a.clone(), source_b.clone()]));
    assert_eq!(facet_sources[&nested], BTreeSet::from([source_a]));
    assert_eq!(facet_sources[&two_sided], BTreeSet::from([source_b]));
    assert!(!facet_sources.contains_key(&reversed));
}

#[test]
fn partial_order_backoff_requires_an_explicit_frame_background() {
    let mut pangine = Pangine::new();
    let target = must_reference(&mut pangine, "{{[fixed]->[value]}->['X']}");
    let c = must_reference(&mut pangine, "[C]");
    let e = must_reference(&mut pangine, "[E]");
    let frame = [c.clone(), e.clone()];
    let incomplete_background = BTreeMap::from([(c.clone(), 1.0)]);
    assert_eq!(
        partial_order_backoff_profiles(&pangine, &[], &frame, &target, &incomplete_background, 2.0).unwrap_err(),
        "candidate background is not a probability distribution over the frame"
    );

    let valid_background = BTreeMap::from([(c, 0.5), (e, 0.5)]);
    assert!(partial_order_backoff_profiles(&pangine, &[], &frame, &target, &valid_background, 2.0).unwrap().is_empty());
}

#[test]
fn maximal_unordered_subset_contexts_pass_the_accumulation_gate() {
    let mut pangine = Pangine::new();
    let complete_source = must_reference(&mut pangine, "[complete-source]");
    let complete_root = must_reference(&mut pangine, "{({[left]->[fixed]}*{[right]->[fixed]})->[C]}");
    let question = must_reference(&mut pangine, "{({[left]->[fixed]}*{[right]->[fixed]})->['X']}");
    let general_context = must_reference(&mut pangine, "{{[left]->[fixed]}->['X']}");
    let c = must_reference(&mut pangine, "[C]");
    let e = must_reference(&mut pangine, "[E]");

    let mut occurrences = vec![(complete_source, complete_root)];
    let mut partial_sources = Vec::new();
    for index in 1..=3 {
        let source = must_reference(&mut pangine, &format!("[partial-source-{index}]"));
        let root = must_reference(&mut pangine, &format!("{{({{[left]->[fixed]}}*{{[noise]->[e-{index}]}})->[E]}}"));
        partial_sources.push(source.clone());
        occurrences.push((source, root));
    }
    for index in 1..=2 {
        occurrences.push((
            must_reference(&mut pangine, &format!("[background-source-{index}]")),
            must_reference(&mut pangine, &format!("{{{{[noise]->[c-{index}]}}->[C]}}")),
        ));
    }

    let occurrence_state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let observations = collect_flat_unordered_context_observations(&mut pangine, &occurrence_state, &question).unwrap();
    assert_eq!(observations.len(), 4);
    assert_eq!(candidate_sources(&observations, &c, &question).len(), 1);
    assert_eq!(candidate_sources(&observations, &e, &general_context).len(), 3);
    assert!(context_subsumes(&pangine, &question, &general_context));
    assert!(!contains_node(&question, &general_context, &mut BTreeSet::new()));
    let frame = [c.clone(), e.clone()];
    let background = BTreeMap::from([(c.clone(), 0.5), (e.clone(), 0.5)]);
    for count in [1, 3] {
        let selected_sources = partial_sources.iter().take(count).cloned().collect::<BTreeSet<_>>();
        let selected =
            observations.iter().filter(|observation| observation.candidate == c || selected_sources.contains(&observation.source)).cloned().collect::<Vec<_>>();
        let support = encode_support_state(&mut pangine, &selected).unwrap();
        let decoded = decode_support_state(&support).unwrap();
        let profiles = partial_order_backoff_profiles(&pangine, &decoded, &frame, &question, &background, 2.0).unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].path, vec![general_context.clone(), question.clone()]);

        if count == 1 {
            assert!(profiles[0].predictive[&c] > profiles[0].predictive[&e]);
        } else {
            assert!(profiles[0].predictive[&e] > profiles[0].predictive[&c]);
        }
    }
}

#[test]
fn maximal_unordered_subset_contexts_resist_many_weak_matches() {
    let mut pangine = Pangine::new();
    let target = must_reference(&mut pangine, "{({[f0]->[fixed]}*{[f1]->[fixed]}*{[f2]->[fixed]}*{[f3]->[fixed]}*{[f4]->[fixed]}*{[f5]->[fixed]})->['X']}");
    let weak_context = must_reference(&mut pangine, "{{[f0]->[fixed]}->['X']}");
    let close_context = must_reference(&mut pangine, "{({[f0]->[fixed]}*{[f1]->[fixed]}*{[f2]->[fixed]}*{[f3]->[fixed]}*{[f4]->[fixed]})->['X']}");
    let c = must_reference(&mut pangine, "[C]");
    let e = must_reference(&mut pangine, "[E]");
    let mut occurrences = Vec::new();
    for index in 0..64 {
        occurrences.push((
            must_reference(&mut pangine, &format!("[weak-source-{index}]")),
            must_reference(&mut pangine, &format!("{{({{[f0]->[fixed]}}*{{[noise]->[weak-{index}]}})->[E]}}")),
        ));
    }
    for index in 0..3 {
        occurrences.push((
            must_reference(&mut pangine, &format!("[close-source-{index}]")),
            must_reference(
                &mut pangine,
                &format!("{{({{[f0]->[fixed]}}*{{[f1]->[fixed]}}*{{[f2]->[fixed]}}*{{[f3]->[fixed]}}*{{[f4]->[fixed]}}*{{[noise]->[close-{index}]}})->[C]}}"),
            ),
        ));
    }

    let occurrence_state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let observations = collect_flat_unordered_context_observations(&mut pangine, &occurrence_state, &target).unwrap();
    assert_eq!(candidate_sources(&observations, &c, &close_context).len(), 3);
    assert_eq!(candidate_sources(&observations, &e, &weak_context).len(), 64);
    assert!(context_subsumes(&pangine, &target, &close_context));
    assert!(context_subsumes(&pangine, &close_context, &weak_context));
    assert!(!contains_node(&close_context, &weak_context, &mut BTreeSet::new()));
    let support = encode_support_state(&mut pangine, &observations).unwrap();
    let decoded = decode_support_state(&support).unwrap();
    let background = BTreeMap::from([(c.clone(), 4.0 / 69.0), (e.clone(), 65.0 / 69.0)]);
    let profiles = partial_order_backoff_profiles(&pangine, &decoded, &[c.clone(), e.clone()], &target, &background, 2.0).unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].path, vec![weak_context, close_context, target]);
    assert!(profiles[0].predictive[&c] > profiles[0].predictive[&e]);
}

#[test]
fn incomparable_unordered_parent_contexts_remain_separate_backoff_profiles() {
    let mut pangine = Pangine::new();
    let target = must_reference(&mut pangine, "{({[f0]->[fixed]}*{[f1]->[fixed]}*{[f2]->[fixed]})->['X']}");
    let parent_a = must_reference(&mut pangine, "{({[f0]->[fixed]}*{[f1]->[fixed]})->['X']}");
    let parent_b = must_reference(&mut pangine, "{({[f0]->[fixed]}*{[f2]->[fixed]})->['X']}");
    let c = must_reference(&mut pangine, "[C]");
    let e = must_reference(&mut pangine, "[E]");
    let occurrences = [
        (must_reference(&mut pangine, "[source-a]"), must_reference(&mut pangine, "{({[f0]->[fixed]}*{[f1]->[fixed]}*{[noise]->[a]})->[C]}")),
        (must_reference(&mut pangine, "[source-b]"), must_reference(&mut pangine, "{({[f0]->[fixed]}*{[f2]->[fixed]}*{[noise]->[b]})->[E]}")),
    ];

    let occurrence_state = encode_occurrence_state(&mut pangine, &occurrences).unwrap().unwrap();
    let observations = collect_flat_unordered_context_observations(&mut pangine, &occurrence_state, &target).unwrap();
    assert!(context_subsumes(&pangine, &target, &parent_a));
    assert!(context_subsumes(&pangine, &target, &parent_b));
    assert!(!context_subsumes(&pangine, &parent_a, &parent_b));
    assert!(!context_subsumes(&pangine, &parent_b, &parent_a));
    let support = encode_support_state(&mut pangine, &observations).unwrap();
    let decoded = decode_support_state(&support).unwrap();
    let background = BTreeMap::from([(c.clone(), 0.5), (e.clone(), 0.5)]);
    let profiles = partial_order_backoff_profiles(&pangine, &decoded, &[c.clone(), e.clone()], &target, &background, 2.0).unwrap();
    assert_eq!(profiles.len(), 2);

    let through_a = profiles.iter().find(|profile| profile.path.first() == Some(&parent_a)).unwrap();
    let through_b = profiles.iter().find(|profile| profile.path.first() == Some(&parent_b)).unwrap();
    assert_eq!(through_a.path, vec![parent_a, target.clone()]);
    assert_eq!(through_b.path, vec![parent_b, target]);
    assert!(through_a.predictive[&c] > through_a.predictive[&e]);
    assert!(through_b.predictive[&e] > through_b.predictive[&c]);
}

#[test]
fn maximal_unordered_subset_extraction_does_not_materialize_a_powerset() {
    let mut pangine = Pangine::new();
    let question_facets = (0..20).map(|index| format!("{{[f{index}]->[fixed]}}")).collect::<Vec<_>>();
    let mut experience_facets = question_facets.iter().take(19).cloned().collect::<Vec<_>>();
    experience_facets.push("{[noise]->[different]}".to_owned());
    let question = must_reference(&mut pangine, &format!("{{({})->['X']}}", question_facets.join("*")));
    let root = must_reference(&mut pangine, &format!("{{({})->[C]}}", experience_facets.join("*")));
    let source = must_reference(&mut pangine, "[source]");
    let state = encode_occurrence_state(&mut pangine, &[(source, root)]).unwrap().unwrap();
    let baseline_count = pangine.concept_count();

    let observations = collect_flat_unordered_context_observations(&mut pangine, &state, &question).unwrap();
    assert_eq!(observations.len(), 1);
    assert_eq!(unordered_context_signature(&pangine, &observations[0].context).unwrap().0.len(), 19);
    assert_eq!(pangine.concept_count() - baseline_count, 2);
}
