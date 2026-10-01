//! Exact probabilities read from projected evidence, interpolated across
//! completion grades with the Witten-Bell rule.

use super::{CompletionProjectionSupport, ConceptId};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// An exact probability, kept as a reduced nonnegative fraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Probability {
    numerator: i128,
    denominator: i128,
}

impl Probability {
    pub(super) const ZERO: Self = Self { numerator: 0, denominator: 1 };

    pub(super) fn new(numerator: i128, denominator: i128) -> Option<Self> {
        if numerator < 0 || denominator <= 0 {
            return None;
        }
        let divisor = greatest_common_divisor(numerator, denominator);
        Some(Self { numerator: numerator / divisor, denominator: denominator / divisor })
    }

    /// Returns the reduced numerator and denominator.
    pub(super) fn fraction(self) -> (i128, i128) {
        (self.numerator, self.denominator)
    }

    pub(super) fn is_zero(self) -> bool {
        self.numerator == 0
    }

    pub(super) fn as_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

impl Ord for Probability {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.numerator.checked_mul(other.denominator), other.numerator.checked_mul(self.denominator)) {
            (Some(left), Some(right)) => left.cmp(&right),
            _ => self.as_f64().total_cmp(&other.as_f64()).then_with(|| (self.numerator, self.denominator).cmp(&(other.numerator, other.denominator))),
        }
    }
}

impl PartialOrd for Probability {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Reads each projected value's probability from its graded evidence.
///
/// Every grade present forms one cumulative level that holds the positive
/// evidence of that grade and every more exact one. The most general level
/// is read as plain shares. Each more exact level then mixes its own evidence
/// with the level after it, `(c(v) + T * P_next(v)) / (c + T)`, where `c` is
/// the level's total and `T` the number of values it supports. That is
/// Witten-Bell interpolation, and an answer with one grade keeps plain shares.
/// A level without evidence, or one that adds nothing, is skipped.
pub(super) fn interpolated_probabilities(support: &CompletionProjectionSupport) -> Option<BTreeMap<ConceptId, Probability>> {
    let grades = support.values().flat_map(|derivations| derivations.keys().map(|(grade, _, _)| *grade)).collect::<BTreeSet<_>>();
    let mut levels: Vec<BTreeMap<&ConceptId, i128>> = Vec::new();
    for grade in grades {
        let mut level = BTreeMap::new();
        for (value, derivations) in support {
            let evidence = derivations
                .iter()
                .filter(|((derivation_grade, _, _), _)| *derivation_grade <= grade)
                .try_fold(0_i128, |evidence, (_, weight)| evidence.checked_add(i128::from(weight.count())))?;
            if evidence > 0 {
                level.insert(value, evidence);
            }
        }
        if !level.is_empty() && levels.last() != Some(&level) {
            levels.push(level);
        }
    }

    let Some(most_general) = levels.pop() else {
        return Some(BTreeMap::new());
    };
    let total = level_total(&most_general)?;
    let mut probabilities =
        most_general.into_iter().map(|(value, evidence)| Some((value, Probability::new(evidence, total)?))).collect::<Option<BTreeMap<_, _>>>()?;
    for level in levels.into_iter().rev() {
        let total = level_total(&level)?;
        let kinds = i128::try_from(level.len()).ok()?;
        let mixed = total.checked_add(kinds)?;
        let values = probabilities.keys().chain(level.keys()).copied().collect::<BTreeSet<_>>();
        probabilities = values
            .into_iter()
            .map(|value| {
                let next = probabilities.get(value).copied().unwrap_or(Probability::ZERO);
                let own = level.get(value).copied().unwrap_or(0);
                let numerator = own.checked_mul(next.denominator)?.checked_add(kinds.checked_mul(next.numerator)?)?;
                Some((value, Probability::new(numerator, next.denominator.checked_mul(mixed)?)?))
            })
            .collect::<Option<_>>()?;
    }
    Some(probabilities.into_iter().map(|(value, probability)| (value.clone(), probability)).collect())
}

/// Restates each probability as a whole-number share over the probabilities'
/// common denominator, keeping their exact ratios.
pub(super) fn whole_number_shares<K: Ord>(probabilities: BTreeMap<K, Probability>) -> Option<BTreeMap<K, i128>> {
    let denominator = common_denominator(probabilities.values())?;
    probabilities.into_iter().map(|(value, probability)| Some((value, probability.numerator.checked_mul(denominator / probability.denominator)?))).collect()
}

/// Returns the least common multiple of the probabilities' denominators.
fn common_denominator<'a>(probabilities: impl IntoIterator<Item = &'a Probability>) -> Option<i128> {
    probabilities.into_iter().try_fold(1_i128, |common, probability| {
        let divisor = greatest_common_divisor(common, probability.denominator);
        (common / divisor).checked_mul(probability.denominator)
    })
}

fn level_total(level: &BTreeMap<&ConceptId, i128>) -> Option<i128> {
    level.values().try_fold(0_i128, |total, evidence| total.checked_add(*evidence))
}

fn greatest_common_divisor(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probabilities_reduce_and_compare_exactly() {
        assert_eq!(Probability::new(2, 4).unwrap().fraction(), (1, 2));
        assert_eq!(Probability::new(0, 7).unwrap().fraction(), (0, 1));
        assert!(Probability::new(-1, 2).is_none());
        assert!(Probability::new(1, 0).is_none());
        assert!(Probability::new(1, 3).unwrap() < Probability::new(1, 2).unwrap());
        assert_eq!(Probability::new(9, 40).unwrap().cmp(&Probability::new(18, 80).unwrap()), Ordering::Equal);
        assert_eq!(common_denominator([Probability::new(9, 40).unwrap(), Probability::new(23, 120).unwrap()].iter()), Some(120));
        let shares = whole_number_shares(BTreeMap::from([("C-D", Probability::new(9, 40).unwrap()), ("E-D", Probability::new(23, 120).unwrap())]));
        assert_eq!(shares, Some(BTreeMap::from([("C-D", 27), ("E-D", 23)])));
    }
}
