//! Research-only comparison of policies for combining broad and specific evidence.
//!
//! The adapter supplies a binary candidate frame, an explicit single-parent
//! context tree, uniform outcome priors, and a prior over whether each context
//! should share its parent's outcome distribution. None of those inputs are
//! currently Pangine semantics.

use std::cmp::min;
use std::collections::{BTreeMap, BTreeSet};

use pangine::{ConceptId, Pangine};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fraction {
    numerator: u128,
    denominator: u128,
}

impl Fraction {
    const ZERO: Self = Self { numerator: 0, denominator: 1 };
    const ONE: Self = Self { numerator: 1, denominator: 1 };

    fn new(numerator: u128, denominator: u128) -> Self {
        assert_ne!(denominator, 0);
        let divisor = greatest_common_divisor(numerator, denominator);
        Self { numerator: numerator / divisor, denominator: denominator / divisor }
    }

    fn from_u64(value: u64) -> Self {
        Self::new(u128::from(value), 1)
    }

    fn add(self, other: Self) -> Self {
        let numerator = self
            .numerator
            .checked_mul(other.denominator)
            .and_then(|left| other.numerator.checked_mul(self.denominator).and_then(|right| left.checked_add(right)))
            .expect("research fraction addition overflowed");
        let denominator = self.denominator.checked_mul(other.denominator).expect("research fraction denominator overflowed");
        Self::new(numerator, denominator)
    }

    fn multiply(self, other: Self) -> Self {
        let numerator = self.numerator.checked_mul(other.numerator).expect("research fraction multiplication overflowed");
        let denominator = self.denominator.checked_mul(other.denominator).expect("research fraction denominator overflowed");
        Self::new(numerator, denominator)
    }

    fn divide(self, other: Self) -> Self {
        assert_ne!(other.numerator, 0);
        let numerator = self.numerator.checked_mul(other.denominator).expect("research fraction division overflowed");
        let denominator = self.denominator.checked_mul(other.numerator).expect("research fraction denominator overflowed");
        Self::new(numerator, denominator)
    }

    fn scale(self, value: u64) -> Self {
        self.multiply(Self::from_u64(value))
    }

    fn complement(self) -> Self {
        assert!(self.numerator <= self.denominator);
        Self::new(self.denominator - self.numerator, self.denominator)
    }

    fn is_greater_than(self, other: Self) -> bool {
        self.numerator.checked_mul(other.denominator).expect("research fraction comparison overflowed")
            > other.numerator.checked_mul(self.denominator).expect("research fraction comparison overflowed")
    }
}

fn greatest_common_divisor(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BinaryCounts {
    first: u64,
    second: u64,
}

impl BinaryCounts {
    fn total(self) -> u64 {
        self.first + self.second
    }

    fn add_candidate(&mut self, candidate: &ConceptId, frame: &[ConceptId; 2]) -> Result<(), &'static str> {
        if candidate == &frame[0] {
            self.first += 1;
        } else if candidate == &frame[1] {
            self.second += 1;
        } else {
            return Err("candidate is outside the declared frame");
        }
        Ok(())
    }
}

fn uniform_binary_posterior(counts: BinaryCounts) -> [Fraction; 2] {
    let denominator = u128::from(counts.total() + 2);
    [Fraction::new(u128::from(counts.first + 1), denominator), Fraction::new(u128::from(counts.second + 1), denominator)]
}

fn uniform_binary_marginal(counts: BinaryCounts) -> Fraction {
    let combinations = binomial(counts.total(), counts.first);
    let denominator = u128::from(counts.total() + 1).checked_mul(combinations).expect("research marginal denominator overflowed");
    Fraction::new(1, denominator)
}

fn binomial(n: u64, k: u64) -> u128 {
    let k = min(k, n - k);
    (1..=k).fold(1_u128, |value, index| value.checked_mul(u128::from(n - k + index)).expect("research binomial overflowed") / u128::from(index))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecursivePolicyOracle {
    root: ConceptId,
    frame: [ConceptId; 2],
    parents: BTreeMap<ConceptId, Option<ConceptId>>,
    occurrences: BTreeMap<ConceptId, (ConceptId, ConceptId)>,
}

impl RecursivePolicyOracle {
    fn new(root: ConceptId, frame: [ConceptId; 2], edges: impl IntoIterator<Item = (ConceptId, ConceptId)>) -> Result<Self, &'static str> {
        if frame[0] == frame[1] {
            return Err("candidate frame contains duplicates");
        }

        let mut nodes = BTreeSet::from([root.clone()]);
        let mut parent_by_child = BTreeMap::new();
        for (child, parent) in edges {
            if child == root {
                return Err("root context cannot have a parent");
            }
            nodes.insert(child.clone());
            nodes.insert(parent.clone());
            if parent_by_child.insert(child, parent).is_some() {
                return Err("Bayesian context tree requires one parent per context");
            }
        }

        for node in &nodes {
            let mut current = node;
            let mut visited = BTreeSet::new();
            while current != &root {
                if !visited.insert(current.clone()) {
                    return Err("Bayesian context tree contains a cycle");
                }
                current = parent_by_child.get(current).ok_or("context does not descend from the declared root")?;
            }
        }

        let mut parents = BTreeMap::from([(root.clone(), None)]);
        parents.extend(parent_by_child.into_iter().map(|(child, parent)| (child, Some(parent))));
        Ok(Self { root, frame, parents, occurrences: BTreeMap::new() })
    }

    fn ingest_observation(&mut self, pangine: &Pangine, observation: &ConceptId) -> Result<bool, &'static str> {
        let payload = pangine.get_observation(observation).ok_or("input is not an Observation")?;
        let occurrence = pangine.get_observer(observation).ok_or("policy fixture requires an occurrence identity")?;
        let context = pangine.get_correlation_a(&payload).ok_or("outcome record has no context")?;
        let candidate = pangine.get_correlation_b(&payload).ok_or("outcome record has no candidate")?;
        if !self.parents.contains_key(&context) {
            return Err("outcome context is outside the declared tree");
        }
        if candidate != self.frame[0] && candidate != self.frame[1] {
            return Err("candidate is outside the declared frame");
        }
        if let Some(existing) = self.occurrences.get(&occurrence) {
            return if existing == &(context, candidate) { Ok(false) } else { Err("one occurrence identifies conflicting outcomes") };
        }
        self.occurrences.insert(occurrence, (context, candidate));
        Ok(true)
    }

    fn merged(&self, other: &Self) -> Result<Self, &'static str> {
        if self.root != other.root || self.frame != other.frame || self.parents != other.parents {
            return Err("policy oracles use different fixtures");
        }
        let mut merged = self.clone();
        for (occurrence, outcome) in &other.occurrences {
            if let Some(existing) = merged.occurrences.get(occurrence) {
                if existing != outcome {
                    return Err("one occurrence identifies conflicting outcomes");
                }
            } else {
                merged.occurrences.insert(occurrence.clone(), outcome.clone());
            }
        }
        Ok(merged)
    }

    fn retaining_context(&self, context: &ConceptId) -> Self {
        let mut retained = self.clone();
        retained.occurrences.retain(|_, (occurrence_context, _)| occurrence_context == context);
        retained
    }

    fn pooled_predictive(&self) -> [Fraction; 2] {
        uniform_binary_posterior(self.subtree_counts(&self.root, None).unwrap())
    }

    fn specific_predictive(&self, context: &ConceptId) -> Result<[Fraction; 2], &'static str> {
        let counts = self.direct_counts(context, None)?;
        if counts.total() == 0 {
            return Err("specific context has no direct evidence");
        }
        Ok(uniform_binary_posterior(counts))
    }

    fn fixed_background_predictive(&self, context: &ConceptId, background_weight: u64) -> Result<[Fraction; 2], &'static str> {
        if background_weight == 0 {
            return Err("background weight must be positive");
        }
        let local = self.direct_counts(context, None)?;
        let mut background = BinaryCounts::default();
        for (occurrence_context, candidate) in self.occurrences.values() {
            if !self.is_descendant(occurrence_context, context)? {
                background.add_candidate(candidate, &self.frame)?;
            }
        }
        let background = uniform_binary_posterior(background);
        let denominator = Fraction::from_u64(local.total() + background_weight);
        Ok([
            Fraction::from_u64(local.first).add(background[0].scale(background_weight)).divide(denominator),
            Fraction::from_u64(local.second).add(background[1].scale(background_weight)).divide(denominator),
        ])
    }

    fn model_averaged_predictive(&self, context: &ConceptId, split_prior: Fraction) -> Result<[Fraction; 2], &'static str> {
        if !self.parents.contains_key(context) {
            return Err("question context is outside the declared tree");
        }
        if split_prior == Fraction::ZERO || split_prior == Fraction::ONE || split_prior.numerator > split_prior.denominator {
            return Err("split prior must be strictly between zero and one");
        }

        let current = self.weighted_likelihood(&self.root, split_prior, None)?;
        let first = self.weighted_likelihood(&self.root, split_prior, Some((context, &self.frame[0])))?.divide(current);
        let second = self.weighted_likelihood(&self.root, split_prior, Some((context, &self.frame[1])))?.divide(current);
        assert_eq!(first.add(second), Fraction::ONE);
        Ok([first, second])
    }

    fn weighted_likelihood(&self, context: &ConceptId, split_prior: Fraction, addition: Option<(&ConceptId, &ConceptId)>) -> Result<Fraction, &'static str> {
        let pooled = uniform_binary_marginal(self.subtree_counts(context, addition)?);
        let children = self.children(context);
        if children.is_empty() {
            return Ok(pooled);
        }

        let mut split = uniform_binary_marginal(self.direct_counts(context, addition)?);
        for child in children {
            split = split.multiply(self.weighted_likelihood(&child, split_prior, addition)?);
        }
        Ok(pooled.multiply(split_prior.complement()).add(split.multiply(split_prior)))
    }

    fn direct_counts(&self, context: &ConceptId, addition: Option<(&ConceptId, &ConceptId)>) -> Result<BinaryCounts, &'static str> {
        if !self.parents.contains_key(context) {
            return Err("context is outside the declared tree");
        }
        let mut counts = BinaryCounts::default();
        for (occurrence_context, candidate) in self.occurrences.values() {
            if occurrence_context == context {
                counts.add_candidate(candidate, &self.frame)?;
            }
        }
        if let Some((addition_context, candidate)) = addition {
            if addition_context == context {
                counts.add_candidate(candidate, &self.frame)?;
            }
        }
        Ok(counts)
    }

    fn subtree_counts(&self, context: &ConceptId, addition: Option<(&ConceptId, &ConceptId)>) -> Result<BinaryCounts, &'static str> {
        if !self.parents.contains_key(context) {
            return Err("context is outside the declared tree");
        }
        let mut counts = BinaryCounts::default();
        for (occurrence_context, candidate) in self.occurrences.values() {
            if self.is_descendant(occurrence_context, context)? {
                counts.add_candidate(candidate, &self.frame)?;
            }
        }
        if let Some((addition_context, candidate)) = addition {
            if self.is_descendant(addition_context, context)? {
                counts.add_candidate(candidate, &self.frame)?;
            }
        }
        Ok(counts)
    }

    fn is_descendant(&self, context: &ConceptId, ancestor: &ConceptId) -> Result<bool, &'static str> {
        let mut current = context;
        loop {
            if current == ancestor {
                return Ok(true);
            }
            match self.parents.get(current).ok_or("context is outside the declared tree")? {
                Some(parent) => current = parent,
                None => return Ok(false),
            }
        }
    }

    fn children(&self, context: &ConceptId) -> Vec<ConceptId> {
        self.parents.iter().filter_map(|(child, parent)| (parent.as_ref() == Some(context)).then_some(child.clone())).collect()
    }

    fn recursive_exposure_count(&self) -> usize {
        self.occurrences
            .values()
            .map(|(context, _)| {
                let mut levels = 1;
                let mut current = context;
                while let Some(parent) = self.parents[current].as_ref() {
                    levels += 1;
                    current = parent;
                }
                levels
            })
            .sum()
    }
}

struct Fixture {
    pangine: Pangine,
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
        let root = reference(&mut pangine, "[choice]");
        let morning = reference(&mut pangine, "{[choice]->[morning]}");
        let workday = reference(&mut pangine, "{{[choice]->[morning]}->[workday]}");
        let evening = reference(&mut pangine, "{[choice]->[evening]}");
        let tea = reference(&mut pangine, "[tea]");
        let coffee = reference(&mut pangine, "[coffee]");
        Self { pangine, root, morning, workday, evening, tea, coffee }
    }

    fn oracle(&self) -> RecursivePolicyOracle {
        RecursivePolicyOracle::new(
            self.root.clone(),
            [self.tea.clone(), self.coffee.clone()],
            [(self.morning.clone(), self.root.clone()), (self.workday.clone(), self.morning.clone()), (self.evening.clone(), self.root.clone())],
        )
        .unwrap()
    }

    fn observation(&mut self, occurrence: &str, context: &ConceptId, candidate: &ConceptId) -> ConceptId {
        let context = self.pangine.format_concept(context, false);
        let candidate = self.pangine.format_concept(candidate, false);
        reference(&mut self.pangine, &format!("?[{occurrence}]:{{{context}->{candidate}}}"))
    }
}

fn reference(pangine: &mut Pangine, source: &str) -> ConceptId {
    pangine.reference_concept(source).unwrap().unwrap_or_else(|| panic!("expected a Concept from {source:?}"))
}

fn sibling_fixture(local_tea_count: usize) -> (Fixture, RecursivePolicyOracle) {
    let mut fixture = Fixture::new();
    let mut oracle = fixture.oracle();
    let workday = fixture.workday.clone();
    let morning = fixture.morning.clone();
    let root = fixture.root.clone();
    let evening = fixture.evening.clone();
    let tea = fixture.tea.clone();
    let coffee = fixture.coffee.clone();
    let mut observations = Vec::new();
    for index in 1..=local_tea_count {
        observations.push(fixture.observation(&format!("workday-tea-{index}"), &workday, &tea));
    }
    observations.push(fixture.observation("morning-coffee", &morning, &coffee));
    observations.push(fixture.observation("root-coffee", &root, &coffee));
    for index in 1..=6 {
        observations.push(fixture.observation(&format!("evening-coffee-{index}"), &evening, &coffee));
    }
    for observation in &observations {
        assert!(oracle.ingest_observation(&fixture.pangine, observation).unwrap());
    }
    (fixture, oracle)
}

#[test]
fn broad_specific_and_fixed_weight_policies_expose_different_assumptions() {
    let (fixture, oracle) = sibling_fixture(3);
    let pooled = oracle.pooled_predictive();
    let specific = oracle.specific_predictive(&fixture.workday).unwrap();
    let light_background = oracle.fixed_background_predictive(&fixture.workday, 2).unwrap();
    let heavy_background = oracle.fixed_background_predictive(&fixture.workday, 100).unwrap();
    let model_averaged = oracle.model_averaged_predictive(&fixture.workday, Fraction::new(1, 2)).unwrap();

    assert_eq!(pooled, [Fraction::new(4, 13), Fraction::new(9, 13)]);
    assert_eq!(specific, [Fraction::new(4, 5), Fraction::new(1, 5)]);
    assert_eq!(light_background, [Fraction::new(16, 25), Fraction::new(9, 25)]);
    assert_eq!(heavy_background, [Fraction::new(13, 103), Fraction::new(90, 103)]);
    assert_eq!(model_averaged, [Fraction::new(7_088, 9_737), Fraction::new(2_649, 9_737)]);
}

#[test]
fn recursive_model_averaging_crosses_over_only_after_specific_evidence_accumulates() {
    let (mut fixture, mut oracle) = sibling_fixture(1);
    let sparse = oracle.model_averaged_predictive(&fixture.workday, Fraction::new(1, 2)).unwrap();
    assert!(sparse[1].is_greater_than(sparse[0]));

    let workday = fixture.workday.clone();
    let tea = fixture.tea.clone();
    for index in 2..=3 {
        let observation = fixture.observation(&format!("workday-tea-{index}"), &workday, &tea);
        assert!(oracle.ingest_observation(&fixture.pangine, &observation).unwrap());
    }
    let accumulated = oracle.model_averaged_predictive(&fixture.workday, Fraction::new(1, 2)).unwrap();
    assert_eq!(accumulated, [Fraction::new(7_088, 9_737), Fraction::new(2_649, 9_737)]);
}

#[test]
fn direct_context_only_cannot_be_a_model_independent_question_result() {
    let (fixture, complete) = sibling_fixture(1);
    let query_local = complete.retaining_context(&fixture.workday);

    let complete_answer = complete.model_averaged_predictive(&fixture.workday, Fraction::new(1, 2)).unwrap();
    let query_local_answer = query_local.model_averaged_predictive(&fixture.workday, Fraction::new(1, 2)).unwrap();

    assert!(complete_answer[1].is_greater_than(complete_answer[0]));
    assert!(query_local_answer[0].is_greater_than(query_local_answer[1]));
}

#[test]
fn the_prior_that_contexts_differ_is_a_material_adapter_input() {
    let (fixture, oracle) = sibling_fixture(3);
    let equal_model_prior = oracle.model_averaged_predictive(&fixture.workday, Fraction::new(1, 2)).unwrap();
    let strong_shared_prior = oracle.model_averaged_predictive(&fixture.workday, Fraction::new(1, 100)).unwrap();

    assert!(equal_model_prior[0].is_greater_than(equal_model_prior[1]));
    assert!(strong_shared_prior[1].is_greater_than(strong_shared_prior[0]));
}

#[test]
fn exact_policy_results_are_replay_and_partition_grouping_invariant() {
    let mut fixture = Fixture::new();
    let template = fixture.oracle();
    let workday = fixture.workday.clone();
    let morning = fixture.morning.clone();
    let root = fixture.root.clone();
    let evening = fixture.evening.clone();
    let tea = fixture.tea.clone();
    let coffee = fixture.coffee.clone();
    let observations = vec![
        fixture.observation("workday-tea-1", &workday, &tea),
        fixture.observation("workday-tea-2", &workday, &tea),
        fixture.observation("workday-tea-3", &workday, &tea),
        fixture.observation("morning-coffee", &morning, &coffee),
        fixture.observation("root-coffee", &root, &coffee),
        fixture.observation("evening-coffee-1", &evening, &coffee),
        fixture.observation("evening-coffee-2", &evening, &coffee),
        fixture.observation("evening-coffee-3", &evening, &coffee),
        fixture.observation("evening-coffee-4", &evening, &coffee),
        fixture.observation("evening-coffee-5", &evening, &coffee),
        fixture.observation("evening-coffee-6", &evening, &coffee),
    ];
    let mut combined = template.clone();
    for observation in &observations {
        assert!(combined.ingest_observation(&fixture.pangine, observation).unwrap());
    }
    assert!(!combined.ingest_observation(&fixture.pangine, &observations[0]).unwrap());

    let mut a = template.clone();
    let mut b = template.clone();
    let mut c = template;
    for (index, observation) in observations.iter().enumerate() {
        match index % 3 {
            0 => assert!(a.ingest_observation(&fixture.pangine, observation).unwrap()),
            1 => assert!(b.ingest_observation(&fixture.pangine, observation).unwrap()),
            _ => assert!(c.ingest_observation(&fixture.pangine, observation).unwrap()),
        }
    }

    let left = a.merged(&b).unwrap().merged(&c).unwrap();
    let right = a.merged(&b.merged(&c).unwrap()).unwrap();
    let reversed = c.merged(&b).unwrap().merged(&a).unwrap();
    assert_eq!(left, combined);
    assert_eq!(right, combined);
    assert_eq!(reversed, combined);
    assert_eq!(
        left.model_averaged_predictive(&fixture.workday, Fraction::new(1, 2)).unwrap(),
        combined.model_averaged_predictive(&fixture.workday, Fraction::new(1, 2)).unwrap()
    );
}

#[test]
fn recursive_likelihoods_derive_from_each_occurrence_once() {
    let (_, oracle) = sibling_fixture(3);
    assert_eq!(oracle.occurrences.len(), 11);
    assert_eq!(oracle.subtree_counts(&oracle.root, None).unwrap().total(), 11);
    assert_eq!(oracle.recursive_exposure_count(), 24);
}

#[test]
fn a_general_concept_graph_does_not_implicitly_supply_a_bayesian_tree() {
    let mut pangine = Pangine::new();
    let root = reference(&mut pangine, "[choice]");
    let morning = reference(&mut pangine, "[morning]");
    let weekday = reference(&mut pangine, "[weekday]");
    let workday = reference(&mut pangine, "[workday]");
    let tea = reference(&mut pangine, "[tea]");
    let coffee = reference(&mut pangine, "[coffee]");

    assert_eq!(
        RecursivePolicyOracle::new(
            root.clone(),
            [tea, coffee],
            [(morning.clone(), root.clone()), (weekday.clone(), root), (workday.clone(), morning), (workday, weekday)]
        )
        .unwrap_err(),
        "Bayesian context tree requires one parent per context"
    );
}
