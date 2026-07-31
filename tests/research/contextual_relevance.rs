//! Research-only one-association contextual relevance oracle.
//!
//! The 1.x source sketched a decision in which A and C share one exact
//! attribute, A has another observed attribute, and a question asks for C's
//! corresponding value. It left the actual decision as a TODO
//! (`1.x/pangine/src/test/common/test_reference_concept.cpp:663-682`). This
//! oracle makes that missing boundary explicit without changing `@`.
//!
//! The proposed step is structural rather than name-aware. Facts have the
//! ordinary Pangine form `{{entity->facet}->value}`. A candidate can be direct,
//! or it can be reached through one other entity with the same exact
//! context-facet and context-value. The oracle does not rank those routes,
//! chain them recursively, or assign statistical meaning to them.

use pangine::{ConceptId, Pangine};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
struct FacetedFact {
    source: ConceptId,
    entity: ConceptId,
    facet: ConceptId,
    value: ConceptId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ContextualRoute {
    target_context_source: ConceptId,
    peer_context_source: ConceptId,
    peer_answer_source: ConceptId,
    peer: ConceptId,
    shared_facet: ConceptId,
    shared_value: ConceptId,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CandidateEvidence {
    direct_sources: BTreeSet<ConceptId>,
    contextual_routes: BTreeSet<ContextualRoute>,
}

type FactMap = BTreeMap<ConceptId, FacetedFact>;
type EvidenceMap = BTreeMap<ConceptId, CandidateEvidence>;

fn must_reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a Concept from {source:?}"))
}

fn faceted_shape(pangine: &Pangine, concept: &ConceptId) -> Option<(ConceptId, ConceptId, ConceptId)> {
    let pair = pangine.get_correlation_a(concept)?;
    let value = pangine.get_correlation_b(concept)?;
    let entity = pangine.get_correlation_a(&pair)?;
    let facet = pangine.get_correlation_b(&pair)?;
    Some((entity, facet, value))
}

fn collect_faceted_facts(pangine: &Pangine, state: &ConceptId) -> FactMap {
    let mut facts = FactMap::new();
    for source in pangine.get_observations(state).unwrap_or_default() {
        let Some(root) = pangine.get_observation(&source) else {
            continue;
        };
        let Some((entity, facet, value)) = faceted_shape(pangine, &root) else {
            continue;
        };
        facts.insert(source.clone(), FacetedFact { source, entity, facet, value });
    }
    facts
}

fn question_shape(pangine: &Pangine, question: &ConceptId) -> Option<(ConceptId, ConceptId)> {
    let (entity, facet, output) = faceted_shape(pangine, question)?;
    pangine.get_percept(&output)?;
    Some((entity, facet))
}

fn derive_one_association(pangine: &Pangine, facts: &FactMap, question: &ConceptId) -> Result<EvidenceMap, &'static str> {
    let (target, answer_facet) = question_shape(pangine, question).ok_or("question must have the form {{entity->facet}->['output']}")?;
    let mut evidence = EvidenceMap::new();

    for fact in facts.values().filter(|fact| fact.entity == target && fact.facet == answer_facet) {
        evidence.entry(fact.value.clone()).or_default().direct_sources.insert(fact.source.clone());
    }

    for target_context in facts.values().filter(|fact| fact.entity == target && fact.facet != answer_facet) {
        for peer_context in facts.values().filter(|fact| fact.entity != target && fact.facet == target_context.facet && fact.value == target_context.value) {
            for peer_answer in facts.values().filter(|fact| fact.entity == peer_context.entity && fact.facet == answer_facet) {
                let route = ContextualRoute {
                    target_context_source: target_context.source.clone(),
                    peer_context_source: peer_context.source.clone(),
                    peer_answer_source: peer_answer.source.clone(),
                    peer: peer_context.entity.clone(),
                    shared_facet: target_context.facet.clone(),
                    shared_value: target_context.value.clone(),
                };
                evidence.entry(peer_answer.value.clone()).or_default().contextual_routes.insert(route);
            }
        }
    }

    Ok(evidence)
}

fn union_facts(target: &mut FactMap, source: &FactMap) {
    for (record, fact) in source {
        target.insert(record.clone(), fact.clone());
    }
}

fn union_evidence(target: &mut EvidenceMap, source: &EvidenceMap) {
    for (candidate, evidence) in source {
        let combined = target.entry(candidate.clone()).or_default();
        combined.direct_sources.extend(evidence.direct_sources.iter().cloned());
        combined.contextual_routes.extend(evidence.contextual_routes.iter().cloned());
    }
}

fn experience(pangine: &mut Pangine, memory: &str, roots: &[&str]) -> ConceptId {
    let mut state = None;
    for root in roots {
        state = Some(must_reference(pangine, &format!("['{memory}'] ~= {root}")));
    }
    state.unwrap_or_else(|| panic!("expected at least one root for {memory:?}"))
}

fn fixture(pangine: &mut Pangine, memory: &str) -> ConceptId {
    experience(
        pangine,
        memory,
        &[
            "?[a-species]:{{[A]->[species_is]}->[cat]}",
            "?[a-sound]:{{[A]->[sound_is]}->[meow]}",
            "?[b-species]:{{[B]->[species_is]}->[dog]}",
            "?[b-sound]:{{[B]->[sound_is]}->[bark]}",
            "?[c-species]:{{[C]->[species_is]}->[cat]}",
            "?[c-sound]:{{[C]->[sound_is]}->[purr]}",
            "?[d-color]:{{[D]->[color_is]}->[cat]}",
            "?[d-sound]:{{[D]->[sound_is]}->[chirp]}",
        ],
    )
}

fn formatted_candidates(pangine: &Pangine, evidence: &EvidenceMap) -> BTreeSet<String> {
    evidence.keys().map(|candidate| pangine.format_concept(candidate, false)).collect()
}

#[test]
fn one_contextual_association_keeps_direct_and_indirect_routes_without_admitting_unrelated_paths() {
    let mut pangine = Pangine::new();
    let state = fixture(&mut pangine, "memory");
    let question = must_reference(&mut pangine, "{{[C]->[sound_is]}->['answer']}");
    let facts = collect_faceted_facts(&pangine, &state);
    let evidence = derive_one_association(&pangine, &facts, &question).unwrap();

    assert_eq!(formatted_candidates(&pangine, &evidence), BTreeSet::from(["[meow]".to_owned(), "[purr]".to_owned()]));

    let purr = must_reference(&mut pangine, "[purr]");
    let purr_evidence = &evidence[&purr];
    assert_eq!(purr_evidence.direct_sources, BTreeSet::from([must_reference(&mut pangine, "?[c-sound]:{{[C]->[sound_is]}->[purr]}")]));
    assert!(purr_evidence.contextual_routes.is_empty());

    let meow = must_reference(&mut pangine, "[meow]");
    let meow_evidence = &evidence[&meow];
    assert!(meow_evidence.direct_sources.is_empty());
    assert_eq!(meow_evidence.contextual_routes.len(), 1);
    let route = meow_evidence.contextual_routes.first().unwrap();
    assert_eq!(route.target_context_source, must_reference(&mut pangine, "?[c-species]:{{[C]->[species_is]}->[cat]}"));
    assert_eq!(route.peer_context_source, must_reference(&mut pangine, "?[a-species]:{{[A]->[species_is]}->[cat]}"));
    assert_eq!(route.peer_answer_source, must_reference(&mut pangine, "?[a-sound]:{{[A]->[sound_is]}->[meow]}"));
    assert_eq!(route.peer, must_reference(&mut pangine, "[A]"));
    assert_eq!(route.shared_facet, must_reference(&mut pangine, "[species_is]"));
    assert_eq!(route.shared_value, must_reference(&mut pangine, "[cat]"));

    let mut without_target_context = facts;
    without_target_context.remove(&route.target_context_source);
    let reduced = derive_one_association(&pangine, &without_target_context, &question).unwrap();
    assert_eq!(formatted_candidates(&pangine, &reduced), BTreeSet::from(["[purr]".to_owned()]));
}

#[test]
fn exact_source_fact_union_is_replay_and_regrouping_independent_before_route_derivation() {
    let mut pangine = Pangine::new();
    let target = experience(&mut pangine, "target", &["?[c-species]:{{[C]->[species_is]}->[cat]}", "?[c-sound]:{{[C]->[sound_is]}->[purr]}"]);
    let peer = experience(&mut pangine, "peer", &["?[a-species]:{{[A]->[species_is]}->[cat]}", "?[a-sound]:{{[A]->[sound_is]}->[meow]}"]);
    let unrelated = experience(
        &mut pangine,
        "unrelated",
        &[
            "?[b-species]:{{[B]->[species_is]}->[dog]}",
            "?[b-sound]:{{[B]->[sound_is]}->[bark]}",
            "?[d-color]:{{[D]->[color_is]}->[cat]}",
            "?[d-sound]:{{[D]->[sound_is]}->[chirp]}",
        ],
    );
    let combined = fixture(&mut pangine, "combined");
    let question = must_reference(&mut pangine, "{{[C]->[sound_is]}->['answer']}");

    let target_facts = collect_faceted_facts(&pangine, &target);
    let peer_facts = collect_faceted_facts(&pangine, &peer);
    let unrelated_facts = collect_faceted_facts(&pangine, &unrelated);
    let combined_facts = collect_faceted_facts(&pangine, &combined);
    let expected = derive_one_association(&pangine, &combined_facts, &question).unwrap();

    let mut forward = FactMap::new();
    union_facts(&mut forward, &target_facts);
    union_facts(&mut forward, &peer_facts);
    union_facts(&mut forward, &unrelated_facts);
    union_facts(&mut forward, &peer_facts);

    let mut reverse = FactMap::new();
    union_facts(&mut reverse, &unrelated_facts);
    union_facts(&mut reverse, &peer_facts);
    union_facts(&mut reverse, &target_facts);

    assert_eq!(derive_one_association(&pangine, &forward, &question).unwrap(), expected);
    assert_eq!(derive_one_association(&pangine, &reverse, &question).unwrap(), expected);

    let lost_peer = derive_one_association(&pangine, &target_facts, &question).unwrap();
    assert_eq!(formatted_candidates(&pangine, &lost_peer), BTreeSet::from(["[purr]".to_owned()]));
}

#[test]
fn independently_completed_local_answers_cannot_discover_a_cross_partition_contextual_route() {
    let mut pangine = Pangine::new();
    let target = experience(&mut pangine, "target", &["?[c-species]:{{[C]->[species_is]}->[cat]}", "?[c-sound]:{{[C]->[sound_is]}->[purr]}"]);
    let peer_context = experience(&mut pangine, "peer-context", &["?[a-species]:{{[A]->[species_is]}->[cat]}"]);
    let peer_answer = experience(&mut pangine, "peer-answer", &["?[a-sound]:{{[A]->[sound_is]}->[meow]}"]);
    let question = must_reference(&mut pangine, "{{[C]->[sound_is]}->['answer']}");

    let target_facts = collect_faceted_facts(&pangine, &target);
    let peer_context_facts = collect_faceted_facts(&pangine, &peer_context);
    let peer_answer_facts = collect_faceted_facts(&pangine, &peer_answer);

    let mut combined_facts = FactMap::new();
    union_facts(&mut combined_facts, &target_facts);
    union_facts(&mut combined_facts, &peer_context_facts);
    union_facts(&mut combined_facts, &peer_answer_facts);
    let combined = derive_one_association(&pangine, &combined_facts, &question).unwrap();

    let mut reduced_local_answers = EvidenceMap::new();
    for facts in [&target_facts, &peer_context_facts, &peer_answer_facts] {
        union_evidence(&mut reduced_local_answers, &derive_one_association(&pangine, facts, &question).unwrap());
    }

    assert_eq!(formatted_candidates(&pangine, &combined), BTreeSet::from(["[meow]".to_owned(), "[purr]".to_owned()]));
    assert_eq!(formatted_candidates(&pangine, &reduced_local_answers), BTreeSet::from(["[purr]".to_owned()]));
}

#[test]
fn current_question_projection_does_not_yet_isolate_the_contextual_route() {
    let mut pangine = Pangine::new();
    fixture(&mut pangine, "memory");
    must_reference(&mut pangine, "['memory'] @ {{[C]->[sound_is]}->['answer']}");
    let current = must_reference(&mut pangine, "$['answer']");
    let candidates = pangine.get_relevance_map(&current).into_iter().map(|(_, candidate)| pangine.format_concept(&candidate, false)).collect::<BTreeSet<_>>();

    assert!(candidates.contains("[purr]"));
    assert!(candidates.contains("[meow]"));
    assert!(candidates.contains("[bark]"));
    assert!(candidates.contains("[chirp]"));
}
