//! Choice (`^`): the most probable value wins, and canonical order breaks ties. For an exact answer or a plain value, that is the greatest positive evidence count.
//!
//! Sampled choice (`^~`) draws a value with probability equal to its share instead, from a seeded generator that each engine owns.

use super::{
    interpolation::{draw_shares, Probability},
    ConceptId, ConceptKind, LiveConceptAnswer, Pangine,
};
use std::collections::BTreeMap;

/// The seed of a new engine's generator, so the same commands draw the same
/// values in every run.
const DEFAULT_SAMPLE_SEED: u64 = 0;

/// How `^` and `^~` pick one value.
#[derive(Clone, Copy)]
pub(super) enum Choice {
    /// `^`: the most probable value, with canonical order breaking a tie.
    MostProbable,
    /// `^~`: a value drawn with probability equal to its share.
    Sampled,
}

/// The generator that `^~` draws from: splitmix64, whose 64 bits of state and
/// integer arithmetic give the same sequence on every platform.
pub(super) struct SampleGenerator {
    state: u64,
}

impl Default for SampleGenerator {
    fn default() -> Self {
        Self { state: DEFAULT_SAMPLE_SEED }
    }
}

impl SampleGenerator {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// Returns an integer drawn uniformly from `0..bound`, or none when
    /// `bound` is zero.
    fn below(&mut self, bound: u128) -> Option<u128> {
        if bound == 0 {
            return None;
        }

        // Discarding the highest partial run of 128-bit draws leaves every
        // value below `bound` equally likely.
        let discarded = bound.wrapping_neg() % bound;
        loop {
            let high = u128::from(self.next_u64());
            let low = u128::from(self.next_u64());
            let draw = (high << 64) | low;
            if draw <= u128::MAX - discarded {
                return Some(draw % bound);
            }
        }
    }
}

impl Pangine {
    /// Seeds the generator that `^~` and
    /// [`AnswerView::sample`](crate::AnswerView::sample) draw from.
    ///
    /// A new engine starts from seed 0, so the same commands draw the same
    /// values in every run. Each draw advances the generator.
    pub fn set_sample_seed(&mut self, seed: u64) {
        self.sampler = SampleGenerator { state: seed };
    }

    pub(super) fn make_decision(&mut self, concept: &ConceptId, choice: Choice) -> Option<ConceptId> {
        if !self.owns(concept) {
            return None;
        }
        if self.shared_live_answer(concept).is_some() {
            return self.choose_from_live_answer(concept, choice);
        }

        let concept = self.get_value(concept)?;
        if !matches!(concept.0.kind, ConceptKind::Unordered) {
            return Some(concept);
        }

        let members = concept.0.subconcepts.iter();
        match choice {
            Choice::MostProbable => self.select_greatest_positive(members.map(|(candidate, relevance)| (candidate, relevance.count()))),
            Choice::Sampled => {
                self.draw_weighted(members.filter_map(|(candidate, relevance)| Some((candidate.clone(), u128::try_from(relevance.count()).ok()?))))
            }
        }
    }

    fn select_greatest_positive<'a>(&self, candidates: impl IntoIterator<Item = (&'a ConceptId, i64)>) -> Option<ConceptId> {
        let mut selected = None;
        for (candidate, count) in candidates {
            if count <= 0 {
                continue;
            }

            let canonical = self.format_concept(candidate, false);
            let replace = match &selected {
                None => true,
                Some((greatest, earliest, _)) => count > *greatest || (count == *greatest && canonical < *earliest),
            };
            if replace {
                selected = Some((count, canonical, candidate));
            }
        }
        selected.map(|(_, _, candidate)| candidate.clone())
    }

    fn choose_from_live_answer(&mut self, template: &ConceptId, choice: Choice) -> Option<ConceptId> {
        let (_, live) = self.shared_live_answer(template)?;
        let (selected, answer) = live.answer.choose(self, template, choice)?;
        let next = LiveConceptAnswer::successor(self, live.revision, answer)?;
        self.install_live_answer(next)?;
        Some(selected)
    }

    // A linked answer's values are chosen by the probabilities it reads: plain
    // shares of the positive counts for an exact answer, or the probabilities
    // a graded one reads from its complete rows.
    pub(super) fn select_projection_candidate(&mut self, probabilities: BTreeMap<ConceptId, Probability>, choice: Choice) -> Option<ConceptId> {
        match choice {
            Choice::MostProbable => self.select_most_probable(probabilities),
            Choice::Sampled => self.draw_weighted(draw_shares(&probabilities)),
        }
    }

    fn select_most_probable(&self, probabilities: BTreeMap<ConceptId, Probability>) -> Option<ConceptId> {
        let mut selected = None;
        for (candidate, probability) in probabilities {
            if probability.is_zero() {
                continue;
            }

            let canonical = self.format_concept(&candidate, false);
            let replace = match &selected {
                None => true,
                Some((greatest, earliest, _)) => probability > *greatest || (probability == *greatest && canonical < *earliest),
            };
            if replace {
                selected = Some((probability, canonical, candidate));
            }
        }
        selected.map(|(_, _, candidate)| candidate)
    }

    // Draws one candidate with probability proportional to its weight. The
    // candidates line up in canonical spelling order, so a draw depends only
    // on the weights and the generator, never on the order in which an engine
    // allocated its Concepts.
    fn draw_weighted(&mut self, weights: impl IntoIterator<Item = (ConceptId, u128)>) -> Option<ConceptId> {
        let mut candidates = weights
            .into_iter()
            .filter(|(_, weight)| *weight > 0)
            .map(|(candidate, weight)| (self.format_concept(&candidate, false), weight, candidate))
            .collect::<Vec<_>>();
        candidates.sort_by(|(left, ..), (right, ..)| left.cmp(right));

        let total = candidates.iter().try_fold(0_u128, |total, (_, weight, _)| total.checked_add(*weight))?;
        let mut draw = self.sampler.below(total)?;
        for (_, weight, candidate) in candidates {
            if draw < weight {
                return Some(candidate);
            }
            draw -= weight;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_is_splitmix64() {
        // The first outputs of the reference splitmix64 seeded with 0.
        let mut generator = SampleGenerator::default();
        let outputs = [generator.next_u64(), generator.next_u64(), generator.next_u64()];
        assert_eq!(outputs, [0xe220_a839_7b1d_cdaf, 0x6e78_9e6a_a1b9_65f4, 0x06c4_5d18_8009_454f]);
    }

    #[test]
    fn draws_cover_every_value_below_the_bound_and_none_beyond_it() {
        let mut generator = SampleGenerator { state: 7 };
        assert_eq!(generator.below(0), None);
        assert_eq!(generator.below(1), Some(0));

        let mut seen = [0_usize; 6];
        for _ in 0..6_000 {
            let draw = generator.below(6).expect("a positive bound");
            seen[usize::try_from(draw).expect("a small draw")] += 1;
        }
        assert!(seen.iter().all(|count| (900..=1_100).contains(count)), "uneven draws: {seen:?}");

        let near_the_top = u128::MAX / 3 * 2;
        assert!((0..100).all(|_| generator.below(near_the_top).is_some_and(|draw| draw < near_the_top)));
    }
}
