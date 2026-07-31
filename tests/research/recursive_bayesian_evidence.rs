//! Research-only oracle for incrementally maintained recursive Bayesian evidence.
//!
//! The fixture deliberately supplies a closed two-candidate outcome frame, one
//! equally weighted starting count per candidate, and occurrence identities as
//! explicit observers. These are evaluator inputs rather than Pangine semantics.
//! The authoritative state remains ordinary recursive Observations. The index
//! below is exact, contains no stored probability, and can be rebuilt from that
//! state at any time.

use std::collections::{BTreeMap, BTreeSet};

use pangine::{ConceptId, Pangine};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExactProbability {
    numerator: u64,
    denominator: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecursiveEvidenceIndex {
    root_context: ConceptId,
    frame: BTreeSet<ConceptId>,
    occurrences: BTreeMap<ConceptId, (ConceptId, ConceptId)>,
    recursive_sources: BTreeMap<ConceptId, BTreeMap<ConceptId, BTreeSet<ConceptId>>>,
}

impl RecursiveEvidenceIndex {
    fn new(root_context: ConceptId, frame: impl IntoIterator<Item = ConceptId>) -> Result<Self, &'static str> {
        let frame = frame.into_iter().collect::<BTreeSet<_>>();
        if frame.len() < 2 {
            return Err("Bayesian fixture requires at least two outcomes");
        }

        Ok(Self { root_context, frame, occurrences: BTreeMap::new(), recursive_sources: BTreeMap::new() })
    }

    fn context_path(&self, pangine: &Pangine, context: &ConceptId) -> Result<Vec<ConceptId>, &'static str> {
        let mut current = context.clone();
        let mut path = Vec::new();
        loop {
            if path.contains(&current) {
                return Err("recursive context contains a cycle");
            }
            path.push(current.clone());
            if current == self.root_context {
                path.reverse();
                return Ok(path);
            }
            current = pangine.get_correlation_a(&current).ok_or("context does not descend from the declared root")?;
        }
    }

    fn decode_observation(&self, pangine: &Pangine, observation: &ConceptId) -> Result<Option<(ConceptId, ConceptId, ConceptId)>, &'static str> {
        let Some(payload) = pangine.get_observation(observation) else {
            return Ok(None);
        };
        let Some(candidate) = pangine.get_correlation_b(&payload) else {
            return Ok(None);
        };
        if !self.frame.contains(&candidate) {
            return Ok(None);
        }

        let occurrence = pangine.get_observer(observation).ok_or("Bayesian fixture requires an explicit occurrence identity")?;
        let context = pangine.get_correlation_a(&payload).ok_or("outcome record has no context")?;
        self.context_path(pangine, &context)?;
        Ok(Some((occurrence, context, candidate)))
    }

    fn insert(&mut self, pangine: &Pangine, occurrence: ConceptId, context: ConceptId, candidate: ConceptId) -> Result<bool, &'static str> {
        if let Some(existing) = self.occurrences.get(&occurrence) {
            return if existing == &(context, candidate) { Ok(false) } else { Err("one occurrence identifies conflicting outcomes") };
        }

        for level in self.context_path(pangine, &context)? {
            self.recursive_sources.entry(level).or_default().entry(candidate.clone()).or_default().insert(occurrence.clone());
        }
        self.occurrences.insert(occurrence, (context, candidate));
        Ok(true)
    }

    fn ingest_observation(&mut self, pangine: &Pangine, observation: &ConceptId) -> Result<bool, &'static str> {
        let (occurrence, context, candidate) = self.decode_observation(pangine, observation)?.ok_or("input is not an outcome Observation")?;
        self.insert(pangine, occurrence, context, candidate)
    }

    fn rebuild(&self, pangine: &Pangine, state: &ConceptId) -> Result<Self, &'static str> {
        let mut rebuilt = Self::new(self.root_context.clone(), self.frame.iter().cloned())?;
        for observation in pangine.get_observations(state).ok_or("authoritative state is not Observation state")? {
            if let Some((occurrence, context, candidate)) = rebuilt.decode_observation(pangine, &observation)? {
                rebuilt.insert(pangine, occurrence, context, candidate)?;
            }
        }
        Ok(rebuilt)
    }

    fn merged(&self, pangine: &Pangine, other: &Self) -> Result<Self, &'static str> {
        if self.root_context != other.root_context || self.frame != other.frame {
            return Err("evidence indexes use different Bayesian fixtures");
        }

        let mut merged = self.clone();
        for (occurrence, (context, candidate)) in &other.occurrences {
            merged.insert(pangine, occurrence.clone(), context.clone(), candidate.clone())?;
        }
        Ok(merged)
    }

    fn pooled_sources(&self, pangine: &Pangine, context: &ConceptId) -> Result<BTreeMap<ConceptId, BTreeSet<ConceptId>>, &'static str> {
        let mut pooled = self.frame.iter().cloned().map(|candidate| (candidate, BTreeSet::new())).collect::<BTreeMap<_, _>>();
        for level in self.context_path(pangine, context)? {
            if let Some(level_sources) = self.recursive_sources.get(&level) {
                for (candidate, sources) in level_sources {
                    pooled.get_mut(candidate).ok_or("recursive summary contains an outcome outside the frame")?.extend(sources.iter().cloned());
                }
            }
        }
        Ok(pooled)
    }

    fn predictive_from_sources(
        &self,
        sources: BTreeMap<ConceptId, BTreeSet<ConceptId>>,
        prior_per_candidate: u64,
    ) -> Result<BTreeMap<ConceptId, ExactProbability>, &'static str> {
        if prior_per_candidate == 0 {
            return Err("fixture requires a positive prior count");
        }

        let observed = sources.values().map(BTreeSet::len).sum::<usize>() as u64;
        let denominator = observed + prior_per_candidate * self.frame.len() as u64;
        Ok(sources
            .into_iter()
            .map(|(candidate, sources)| (candidate, ExactProbability { numerator: sources.len() as u64 + prior_per_candidate, denominator }))
            .collect())
    }

    fn pooled_predictive(
        &self,
        pangine: &Pangine,
        context: &ConceptId,
        prior_per_candidate: u64,
    ) -> Result<BTreeMap<ConceptId, ExactProbability>, &'static str> {
        self.predictive_from_sources(self.pooled_sources(pangine, context)?, prior_per_candidate)
    }

    fn recursive_predictives(
        &self,
        pangine: &Pangine,
        context: &ConceptId,
        prior_per_candidate: u64,
    ) -> Result<BTreeMap<ConceptId, BTreeMap<ConceptId, ExactProbability>>, &'static str> {
        let mut predictives = BTreeMap::new();
        for level in self.context_path(pangine, context)? {
            let mut sources = self.frame.iter().cloned().map(|candidate| (candidate, BTreeSet::new())).collect::<BTreeMap<_, _>>();
            if let Some(level_sources) = self.recursive_sources.get(&level) {
                for (candidate, candidate_sources) in level_sources {
                    sources.get_mut(candidate).ok_or("recursive summary contains an outcome outside the frame")?.extend(candidate_sources.iter().cloned());
                }
            }
            predictives.insert(level, self.predictive_from_sources(sources, prior_per_candidate)?);
        }
        Ok(predictives)
    }

    fn naive_recursive_predictive(
        &self,
        pangine: &Pangine,
        context: &ConceptId,
        prior_per_candidate: u64,
    ) -> Result<BTreeMap<ConceptId, ExactProbability>, &'static str> {
        let mut counts = self.frame.iter().cloned().map(|candidate| (candidate, 0_u64)).collect::<BTreeMap<_, _>>();
        for level in self.context_path(pangine, context)? {
            if let Some(level_sources) = self.recursive_sources.get(&level) {
                for (candidate, sources) in level_sources {
                    *counts.get_mut(candidate).ok_or("recursive summary contains an outcome outside the frame")? += sources.len() as u64;
                }
            }
        }
        let denominator = counts.values().sum::<u64>() + prior_per_candidate * self.frame.len() as u64;
        Ok(counts.into_iter().map(|(candidate, count)| (candidate, ExactProbability { numerator: count + prior_per_candidate, denominator })).collect())
    }
}

struct Fixture {
    pangine: Pangine,
    memory: ConceptId,
    root: ConceptId,
    morning: ConceptId,
    workday: ConceptId,
    evening: ConceptId,
    tea: ConceptId,
    coffee: ConceptId,
}

impl Fixture {
    fn new() -> Self {
        let mut pangine = Pangine::new();
        let memory = pangine.reference_percept("memory");
        let root = reference(&mut pangine, "[choice]");
        let morning = reference(&mut pangine, "{[choice]->[morning]}");
        let workday = reference(&mut pangine, "{{[choice]->[morning]}->[workday]}");
        let evening = reference(&mut pangine, "{[choice]->[evening]}");
        let tea = reference(&mut pangine, "[tea]");
        let coffee = reference(&mut pangine, "[coffee]");
        Self { pangine, memory, root, morning, workday, evening, tea, coffee }
    }

    fn index(&self) -> RecursiveEvidenceIndex {
        RecursiveEvidenceIndex::new(self.root.clone(), [self.tea.clone(), self.coffee.clone()]).unwrap()
    }

    fn observation(&mut self, occurrence: &str, context: &ConceptId, candidate: &ConceptId) -> ConceptId {
        let context = self.pangine.format_concept(context, false);
        let candidate = self.pangine.format_concept(candidate, false);
        reference(&mut self.pangine, &format!("?[{occurrence}]:{{{context}->{candidate}}}"))
    }

    fn experience(&mut self, observation: &ConceptId) -> ConceptId {
        let state = self.pangine.perform_experience(&self.memory, Some(observation)).unwrap();
        assert!(self.pangine.set_percept_value(&self.memory, Some(state.clone())));
        state
    }
}

fn reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a Concept from {source:?}"))
}

fn fixture_observations(fixture: &mut Fixture) -> Vec<ConceptId> {
    let workday = fixture.workday.clone();
    let morning = fixture.morning.clone();
    let root = fixture.root.clone();
    let tea = fixture.tea.clone();
    let coffee = fixture.coffee.clone();
    vec![
        fixture.observation("event-1", &workday, &tea),
        fixture.observation("event-2", &workday, &tea),
        fixture.observation("event-3", &workday, &tea),
        fixture.observation("event-4", &morning, &coffee),
        fixture.observation("event-5", &root, &coffee),
    ]
}

#[test]
fn incremental_exact_evidence_matches_a_rebuild_and_derives_probability_only_at_question_time() {
    let mut fixture = Fixture::new();
    let observations = fixture_observations(&mut fixture);
    let mut incremental = fixture.index();
    let mut state = None;
    for observation in &observations {
        assert!(incremental.ingest_observation(&fixture.pangine, observation).unwrap());
        state = Some(fixture.experience(observation));
    }

    let state = state.unwrap();
    let rebuilt = incremental.rebuild(&fixture.pangine, &state).unwrap();
    assert_eq!(incremental, rebuilt);
    assert!(!fixture.pangine.format_concept(&state, false).contains('%'));

    let predictive = rebuilt.pooled_predictive(&fixture.pangine, &fixture.workday, 1).unwrap();
    assert_eq!(predictive[&fixture.tea], ExactProbability { numerator: 4, denominator: 7 });
    assert_eq!(predictive[&fixture.coffee], ExactProbability { numerator: 3, denominator: 7 });
}

#[test]
fn replay_and_recursive_exposure_do_not_count_one_occurrence_more_than_once() {
    let mut fixture = Fixture::new();
    let workday = fixture.workday.clone();
    let tea = fixture.tea.clone();
    let event = fixture.observation("event-1", &workday, &tea);
    let mut index = fixture.index();
    assert!(index.ingest_observation(&fixture.pangine, &event).unwrap());
    assert!(!index.ingest_observation(&fixture.pangine, &event).unwrap());

    let first_state = fixture.experience(&event);
    let replayed_state = fixture.experience(&event);
    assert_eq!(first_state, replayed_state);

    let predictive = index.pooled_predictive(&fixture.pangine, &fixture.workday, 1).unwrap();
    let naive = index.naive_recursive_predictive(&fixture.pangine, &fixture.workday, 1).unwrap();
    assert_eq!(predictive[&fixture.tea], ExactProbability { numerator: 2, denominator: 3 });
    assert_eq!(naive[&fixture.tea], ExactProbability { numerator: 4, denominator: 5 });
}

#[test]
fn partitioned_incremental_summaries_reduce_to_the_combined_summary_in_any_grouping() {
    let mut fixture = Fixture::new();
    let observations = fixture_observations(&mut fixture);
    let mut combined = fixture.index();
    for observation in &observations {
        combined.ingest_observation(&fixture.pangine, observation).unwrap();
    }

    let mut a = fixture.index();
    let mut b = fixture.index();
    let mut c = fixture.index();
    for &index in &[0, 3] {
        a.ingest_observation(&fixture.pangine, &observations[index]).unwrap();
    }
    for &index in &[1, 4] {
        b.ingest_observation(&fixture.pangine, &observations[index]).unwrap();
    }
    c.ingest_observation(&fixture.pangine, &observations[2]).unwrap();

    let left_grouped = a.merged(&fixture.pangine, &b).unwrap().merged(&fixture.pangine, &c).unwrap();
    let right_grouped = a.merged(&fixture.pangine, &b.merged(&fixture.pangine, &c).unwrap()).unwrap();
    let reversed = c.merged(&fixture.pangine, &b).unwrap().merged(&fixture.pangine, &a).unwrap();
    assert_eq!(left_grouped, combined);
    assert_eq!(right_grouped, combined);
    assert_eq!(reversed, combined);
    assert_eq!(
        left_grouped.pooled_predictive(&fixture.pangine, &fixture.workday, 1).unwrap(),
        combined.pooled_predictive(&fixture.pangine, &fixture.workday, 1).unwrap()
    );
}

#[test]
fn pooling_recursive_levels_erases_specific_context_and_is_rejected() {
    let mut fixture = Fixture::new();
    let observations = fixture_observations(&mut fixture);
    let mut index = fixture.index();
    for observation in &observations {
        index.ingest_observation(&fixture.pangine, observation).unwrap();
    }

    let evening = fixture.evening.clone();
    let coffee = fixture.coffee.clone();
    for number in 6..=11 {
        let observation = fixture.observation(&format!("event-{number}"), &evening, &coffee);
        index.ingest_observation(&fixture.pangine, &observation).unwrap();
    }

    let levels = index.recursive_predictives(&fixture.pangine, &fixture.workday, 1).unwrap();
    assert!(levels[&fixture.root][&fixture.coffee].numerator > levels[&fixture.root][&fixture.tea].numerator);
    assert!(levels[&fixture.morning][&fixture.tea].numerator > levels[&fixture.morning][&fixture.coffee].numerator);
    assert!(levels[&fixture.workday][&fixture.tea].numerator > levels[&fixture.workday][&fixture.coffee].numerator);

    let pooled = index.pooled_predictive(&fixture.pangine, &fixture.workday, 1).unwrap();
    assert_eq!(pooled, levels[&fixture.root]);
    assert!(pooled[&fixture.coffee].numerator > pooled[&fixture.tea].numerator);
}

#[test]
fn the_closed_outcome_adapter_rejects_conflicting_reuse_of_an_occurrence_identity() {
    let mut fixture = Fixture::new();
    let workday = fixture.workday.clone();
    let tea = fixture.tea.clone();
    let coffee = fixture.coffee.clone();
    let tea_event = fixture.observation("event-1", &workday, &tea);
    let coffee_event = fixture.observation("event-1", &workday, &coffee);
    let mut index = fixture.index();

    assert!(index.ingest_observation(&fixture.pangine, &tea_event).unwrap());
    assert_eq!(index.ingest_observation(&fixture.pangine, &coffee_event), Err("one occurrence identifies conflicting outcomes"));
}
