//! Bounded representation oracle for class-indexed recursive collections.
//!
//! This is an illustrative model, not a proposed production representation.
//! It tests which identities and reduction laws survive if relevance,
//! correlation, and observation are treated as distinct collection classes.

use pangine::{ConceptId, Pangine, Relevance};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ModelConcept {
    Null,
    Named(String),
    Relevance(BTreeMap<ModelConcept, i32>),
    Correlation(BTreeMap<DirectedPair, i32>),
    Observation(BTreeSet<ObservedPair>),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DirectedPair {
    source: ModelConcept,
    target: ModelConcept,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ObservedPair {
    observer: Option<ModelConcept>,
    observation: ModelConcept,
}

fn named(name: &str) -> ModelConcept {
    ModelConcept::Named(name.to_owned())
}

fn relevance(entries: impl IntoIterator<Item = (ModelConcept, i32)>) -> ModelConcept {
    let mut members = BTreeMap::new();
    for (concept, coefficient) in entries {
        *members.entry(concept).or_default() += coefficient;
    }
    members.retain(|_, coefficient| *coefficient != 0);

    if members.is_empty() {
        return ModelConcept::Null;
    }
    if members.len() == 1 {
        let (concept, coefficient) = members.first_key_value().unwrap();
        if *coefficient == 1 {
            return concept.clone();
        }
    }
    ModelConcept::Relevance(members)
}

fn correlation(entries: impl IntoIterator<Item = (DirectedPair, i32)>) -> ModelConcept {
    let mut pairs = BTreeMap::new();
    for (pair, coefficient) in entries {
        *pairs.entry(pair).or_default() += coefficient;
    }
    pairs.retain(|_, coefficient| *coefficient != 0);
    if pairs.is_empty() {
        ModelConcept::Null
    } else {
        ModelConcept::Correlation(pairs)
    }
}

fn correlate(source: ModelConcept, target: ModelConcept) -> ModelConcept {
    correlation([(DirectedPair { source, target }, 1)])
}

fn observation(entries: impl IntoIterator<Item = ObservedPair>) -> ModelConcept {
    let entries = entries.into_iter().collect::<BTreeSet<_>>();
    if entries.is_empty() {
        ModelConcept::Null
    } else {
        ModelConcept::Observation(entries)
    }
}

fn observe(observer: Option<ModelConcept>, observed: ModelConcept) -> ModelConcept {
    observation([ObservedPair { observer, observation: observed }])
}

fn fold_experiences(experiences: impl IntoIterator<Item = ModelConcept>) -> ModelConcept {
    let mut records = BTreeSet::new();
    for experience in experiences {
        match experience {
            ModelConcept::Observation(entries) => {
                for entry in entries {
                    add_observed(&mut records, entry.observer, entry.observation);
                }
            }
            other => add_observed(&mut records, None, other),
        }
    }
    observation(records)
}

fn add_observed(records: &mut BTreeSet<ObservedPair>, observer: Option<ModelConcept>, observed: ModelConcept) {
    if !records.insert(ObservedPair { observer: observer.clone(), observation: observed.clone() }) {
        return;
    }

    match observed {
        ModelConcept::Null | ModelConcept::Named(_) => {}
        ModelConcept::Relevance(members) => {
            for (child, coefficient) in members {
                add_observed(records, observer.clone(), relevance([(child, coefficient)]));
            }
        }
        ModelConcept::Correlation(pairs) => {
            for (pair, coefficient) in pairs {
                add_observed(records, observer.clone(), correlation([(pair.clone(), coefficient)]));
                add_observed(records, observer.clone(), pair.source);
                add_observed(records, observer.clone(), pair.target);
            }
        }
        ModelConcept::Observation(entries) => {
            for entry in entries {
                add_observed(records, entry.observer, entry.observation);
            }
        }
    }
}

fn relevance_plus(mut left: Relevance, right: Relevance) -> Relevance {
    left.add(right);
    left
}

fn must_reference(pangine: &mut Pangine, script: &str) -> ConceptId {
    pangine.reference_concept(script).unwrap().unwrap()
}

#[test]
fn classified_singletons_preserve_identity_without_a_universal_entry_tuple() {
    let rain = named("rain");
    let cloud = named("cloud");

    assert_eq!(relevance([(rain.clone(), 1)]), rain);
    assert_ne!(observe(None, rain.clone()), rain);
    assert_ne!(correlate(cloud.clone(), rain.clone()), rain);
    assert_ne!(correlate(cloud.clone(), rain.clone()), correlate(rain, cloud));
}

#[test]
fn correlation_preserves_direction_nesting_and_outer_multiplicity() {
    let a = named("A");
    let b = named("B");
    let c = named("C");
    let ab = correlate(a.clone(), b.clone());

    let right_nested = correlate(a.clone(), correlate(b.clone(), c.clone()));
    let two_edges = correlation([(DirectedPair { source: a, target: b }, 1), (DirectedPair { source: named("B"), target: c }, 1)]);

    assert_ne!(right_nested, two_edges);
    assert_eq!(relevance([(ab.clone(), 1), (ab.clone(), 1)]), relevance([(ab, 2)]));
}

#[test]
fn homogeneous_composition_and_one_multi_entry_relation_remain_distinct() {
    let ab_pair = DirectedPair { source: named("A"), target: named("B") };
    let ab = correlation([(ab_pair.clone(), 1)]);
    let cd = correlate(named("C"), named("D"));
    let composed = relevance([(ab.clone(), 1), (cd, 1)]);
    let one_relation = correlation([(ab_pair.clone(), 1), (DirectedPair { source: named("C"), target: named("D") }, 1)]);
    let repeated_composition = relevance([(ab, 2)]);
    let weighted_relation = correlation([(ab_pair.clone(), 1), (ab_pair, 1)]);

    assert_ne!(composed, one_relation);
    assert_ne!(repeated_composition, weighted_relation);
}

#[test]
fn ordinary_composition_and_experience_apply_different_duplicate_laws() {
    let mut pangine = Pangine::new();
    let composed = must_reference(&mut pangine, "(?[event-1]:[rain])(?[event-1]:[rain])");
    let composed_entries = pangine.get_relevance_map(&composed);

    assert_eq!(composed_entries.len(), 1);
    assert_eq!(composed_entries[0].0, Relevance::new(1.0, 2.0));

    let once = must_reference(&mut pangine, "['memory'] ~= ?[event-1]:[rain]");
    let replay = must_reference(&mut pangine, "['memory'] ~= ?[event-1]:[rain]");
    assert_eq!(once, replay);
}

#[test]
fn current_partial_state_detection_depends_on_outer_relevance_shape() {
    let mut pangine = Pangine::new();
    let distinct = must_reference(&mut pangine, "(?[event-1]:[A])(?[event-2]:[B])");
    let distinct_state = must_reference(&mut pangine, "['distinct'] ~= ((?[event-1]:[A])(?[event-2]:[B]))");
    assert_eq!(distinct_state, distinct);

    let repeated = must_reference(&mut pangine, "(?[event-1]:[A])(?[event-1]:[A])");
    let repeated_state = must_reference(&mut pangine, "['repeated'] ~= ((?[event-1]:[A])(?[event-1]:[A]))");
    assert_ne!(repeated_state, repeated);

    let entries = pangine.get_relevance_map(&repeated_state);
    assert_eq!(entries.len(), 1);
    assert_eq!(pangine.get_observation(&entries[0].1), Some(repeated));
    assert_eq!(pangine.get_observer(&entries[0].1), None);
}

#[test]
fn observation_collection_boundary_distinguishes_state_from_a_payload_lookalike() {
    let payload = relevance([(named("A"), 1), (named("B"), 1)]);
    let state = fold_experiences([payload]);
    let ModelConcept::Observation(records) = state.clone() else {
        panic!("experience must reduce to an Observation collection");
    };
    let lookalike = relevance(records.into_iter().map(|record| (observation([record]), 1)));

    assert_ne!(lookalike, state);
    assert_eq!(fold_experiences([state.clone()]), state);

    let fresh_lookalike = fold_experiences([lookalike.clone()]);
    let ModelConcept::Observation(fresh_records) = fresh_lookalike else {
        panic!("experience must reduce to an Observation collection");
    };
    assert!(fresh_records.contains(&ObservedPair { observer: None, observation: lookalike }));
}

#[test]
fn all_empty_collection_classes_share_null_identity() {
    assert_eq!(relevance([]), ModelConcept::Null);
    assert_eq!(correlation([]), ModelConcept::Null);
    assert_eq!(observation([]), ModelConcept::Null);
}

#[test]
fn classified_observation_fold_preserves_roots_replay_and_partitions() {
    let event = named("event-1");
    let rain_and_a = relevance([(named("rain"), 1), (named("A"), 1)]);
    let linked = correlate(named("B"), named("C"));
    let inputs = [observe(Some(event.clone()), rain_and_a.clone()), linked.clone(), observe(Some(event.clone()), named("rain"))];

    let forward = fold_experiences(inputs.clone());
    let reverse = fold_experiences(inputs.clone().into_iter().rev());
    let left = fold_experiences(inputs[..1].iter().cloned());
    let right = fold_experiences(inputs[1..].iter().cloned());
    let regrouped = fold_experiences([left, right]);
    let replayed = fold_experiences([forward.clone(), forward.clone()]);

    assert_eq!(forward, reverse);
    assert_eq!(forward, regrouped);
    assert_eq!(forward, replayed);

    let ModelConcept::Observation(records) = forward else {
        panic!("experience must reduce to an Observation collection");
    };
    assert!(records.contains(&ObservedPair { observer: Some(event), observation: rain_and_a }));
    assert!(records.contains(&ObservedPair { observer: None, observation: linked }));
}

#[test]
fn explicit_observers_scope_recursive_members() {
    let event_1 = named("event-1");
    let event_2 = named("event-2");
    let payload = relevance([(named("rain"), 1), (named("A"), 1)]);
    let state = fold_experiences([observe(Some(event_1.clone()), payload.clone()), observe(Some(event_2.clone()), payload)]);

    let ModelConcept::Observation(records) = state else {
        panic!("experience must reduce to an Observation collection");
    };
    for observer in [event_1, event_2] {
        assert!(records.contains(&ObservedPair { observer: Some(observer.clone()), observation: named("rain") }));
        assert!(records.contains(&ObservedPair { observer: Some(observer), observation: named("A") }));
    }
}

#[test]
fn signed_relevance_is_not_an_associative_partition_coefficient() {
    let a = Relevance::new(0.25, -2.0);
    let b = Relevance::new(0.25, -2.0);
    let c = Relevance::new(0.5, 2.0);

    let left_grouped = relevance_plus(relevance_plus(a, b), c);
    let right_grouped = relevance_plus(a, relevance_plus(b, c));

    assert_eq!(left_grouped, Relevance::new(0.0, -2.0));
    assert_eq!(right_grouped, Relevance::new(0.0, -1.0));
    assert_ne!(left_grouped, right_grouped);
}
