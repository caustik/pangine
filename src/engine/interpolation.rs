//! Probabilities read from projected evidence, interpolated across
//! completion grades with the Witten-Bell rule.
//!
//! Probabilities stay exact fractions while their arithmetic fits in 128
//! bits. Past that, the interpolation continues in fixed point with 64
//! fractional bits and integer rounding, so every platform computes the same
//! values.

use super::{CompletionProjectionSupport, ConceptId};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// One in fixed point: fixed-point probabilities count units of 2^-64.
const FIXED_ONE: u128 = 1 << 64;
/// A graded `$` shows millionths when its exact shares would not fit in
/// 64-bit evidence counts.
const MILLION: u128 = 1_000_000;
/// Fractions with larger denominators print as decimals.
const LARGEST_PRINTED_DENOMINATOR: i128 = 1_000_000;

/// A probability: an exact reduced fraction, or a fixed-point value once
/// exact arithmetic would no longer fit in 128 bits.
#[derive(Clone, Copy, Debug)]
pub(super) enum Probability {
    /// A reduced fraction with `0 <= numerator <= denominator`.
    Exact { numerator: i128, denominator: i128 },
    /// The probability in units of 2^-64.
    Fixed(u128),
}

impl Probability {
    pub(super) const ZERO: Self = Self::Exact { numerator: 0, denominator: 1 };

    pub(super) fn new(numerator: i128, denominator: i128) -> Option<Self> {
        if numerator < 0 || denominator <= 0 || numerator > denominator {
            return None;
        }
        let divisor = greatest_common_divisor(numerator, denominator);
        Some(Self::Exact { numerator: numerator / divisor, denominator: denominator / divisor })
    }

    /// Returns the reduced numerator and denominator of an exact probability.
    pub(super) fn fraction(self) -> Option<(i128, i128)> {
        match self {
            Self::Exact { numerator, denominator } => Some((numerator, denominator)),
            Self::Fixed(_) => None,
        }
    }

    pub(super) fn is_zero(self) -> bool {
        self.fixed() == 0
    }

    pub(super) fn as_f64(self) -> f64 {
        match self {
            Self::Exact { numerator, denominator } => numerator as f64 / denominator as f64,
            Self::Fixed(value) => value as f64 / FIXED_ONE as f64,
        }
    }

    /// Returns the probability in units of 2^-64, rounded down.
    fn fixed(self) -> u128 {
        match self {
            Self::Exact { numerator, denominator } => scaled_down(numerator.unsigned_abs(), denominator.unsigned_abs()),
            Self::Fixed(value) => value,
        }
    }
}

impl Ord for Probability {
    fn cmp(&self, other: &Self) -> Ordering {
        match (*self, *other) {
            (Self::Exact { numerator: a, denominator: b }, Self::Exact { numerator: c, denominator: d }) => {
                compare_fractions(a.unsigned_abs(), b.unsigned_abs(), c.unsigned_abs(), d.unsigned_abs())
            }
            _ => self.fixed().cmp(&other.fixed()),
        }
    }
}

impl PartialOrd for Probability {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Probability {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Probability {}

/// Prints a fraction such as `9/40`, or a decimal to six places when the
/// denominator is above one million or the probability is fixed point.
impl std::fmt::Display for Probability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Exact { numerator: 0, .. } => formatter.write_str("0"),
            Self::Exact { numerator, denominator: 1 } => write!(formatter, "{numerator}"),
            Self::Exact { numerator, denominator } if denominator <= LARGEST_PRINTED_DENOMINATOR => write!(formatter, "{numerator}/{denominator}"),
            _ => {
                // Round to the nearest millionth; the fixed value is at most 2^64.
                let millionths = (self.fixed() * MILLION + FIXED_ONE / 2) >> 64;
                write!(formatter, "{}.{:06}", millionths / MILLION, millionths % MILLION)
            }
        }
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
///
/// The probabilities are exact fractions while every step fits in 128 bits.
/// Otherwise every value is computed in fixed point instead, rounding down at
/// each step.
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

    if levels.is_empty() {
        return Some(BTreeMap::new());
    }
    let probabilities = exact_interpolation(&levels).or_else(|| fixed_interpolation(&levels))?;
    Some(probabilities.into_iter().map(|(value, probability)| (value.clone(), probability)).collect())
}

fn exact_interpolation<'a>(levels: &[BTreeMap<&'a ConceptId, i128>]) -> Option<BTreeMap<&'a ConceptId, Probability>> {
    let (most_general, specific) = levels.split_last()?;
    let total = level_total(most_general)?;
    let mut probabilities =
        most_general.iter().map(|(value, evidence)| Some((*value, Probability::new(*evidence, total)?))).collect::<Option<BTreeMap<_, _>>>()?;
    for level in specific.iter().rev() {
        let total = level_total(level)?;
        let kinds = i128::try_from(level.len()).ok()?;
        let mixed = total.checked_add(kinds)?;
        let values = probabilities.keys().chain(level.keys()).copied().collect::<BTreeSet<_>>();
        probabilities = values
            .into_iter()
            .map(|value| {
                let (next_numerator, next_denominator) = probabilities.get(value).copied().unwrap_or(Probability::ZERO).fraction()?;
                let own = level.get(value).copied().unwrap_or(0);
                let numerator = own.checked_mul(next_denominator)?.checked_add(kinds.checked_mul(next_numerator)?)?;
                Some((value, Probability::new(numerator, next_denominator.checked_mul(mixed)?)?))
            })
            .collect::<Option<_>>()?;
    }
    Some(probabilities)
}

fn fixed_interpolation<'a>(levels: &[BTreeMap<&'a ConceptId, i128>]) -> Option<BTreeMap<&'a ConceptId, Probability>> {
    let (most_general, specific) = levels.split_last()?;
    let total = u128::try_from(level_total(most_general)?).ok()?;
    let mut probabilities =
        most_general.iter().map(|(value, evidence)| Some((*value, scaled_down(u128::try_from(*evidence).ok()?, total)))).collect::<Option<BTreeMap<_, _>>>()?;
    for level in specific.iter().rev() {
        let total = u128::try_from(level_total(level)?).ok()?;
        let kinds = u128::try_from(level.len()).ok()?;
        let mixed = total.checked_add(kinds)?;
        let values = probabilities.keys().chain(level.keys()).copied().collect::<BTreeSet<_>>();
        probabilities = values
            .into_iter()
            .map(|value| {
                let next = probabilities.get(value).copied().unwrap_or(0);
                let own = u128::try_from(level.get(value).copied().unwrap_or(0)).ok()?;
                let numerator = own.checked_mul(FIXED_ONE)?.checked_add(kinds.checked_mul(next)?)?;
                Some((value, numerator / mixed))
            })
            .collect::<Option<_>>()?;
    }
    Some(probabilities.into_iter().map(|(value, fixed)| (value, Probability::Fixed(fixed))).collect())
}

/// Restates the probabilities as whole-number shares to draw from: exact
/// shares over their common denominator when those fit in 128 bits,
/// otherwise each probability in units of 2^-64.
pub(super) fn draw_shares<K: Ord + Clone>(probabilities: &BTreeMap<K, Probability>) -> BTreeMap<K, u128> {
    if let Some(shares) = exact_shares(probabilities) {
        return shares.into_iter().map(|(value, share)| (value, share.unsigned_abs())).collect();
    }
    probabilities.iter().map(|(value, probability)| (value.clone(), probability.fixed())).collect()
}

/// Restates the probabilities as the whole-number shares a graded `$` shows:
/// exact shares over their common denominator when each fits in 64 bits,
/// otherwise millionths, rounded so that they add to one million.
pub(super) fn displayed_shares<K: Ord + Clone>(probabilities: &BTreeMap<K, Probability>) -> BTreeMap<K, i64> {
    let exact = exact_shares(probabilities)
        .and_then(|shares| shares.into_iter().map(|(value, share)| Some((value, i64::try_from(share).ok()?))).collect::<Option<BTreeMap<_, _>>>());
    if let Some(shares) = exact {
        return shares;
    }

    // Round every probability down to millionths, then give the units still
    // missing from one million to the largest remainders, earlier values
    // first among equal remainders.
    let mut shares = probabilities
        .iter()
        .map(|(value, probability)| {
            let scaled = probability.fixed() * MILLION;
            (value.clone(), scaled >> 64, scaled & (FIXED_ONE - 1))
        })
        .collect::<Vec<_>>();
    let missing = MILLION.saturating_sub(shares.iter().map(|(_, share, _)| share).sum());
    let mut order = (0..shares.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| shares[*right].2.cmp(&shares[*left].2).then(left.cmp(right)));
    for index in order.into_iter().take(usize::try_from(missing).unwrap_or(usize::MAX)) {
        shares[index].1 += 1;
    }
    shares.into_iter().map(|(value, share, _)| (value, i64::try_from(share).unwrap_or(i64::MAX))).collect()
}

/// Returns exact whole-number shares over the probabilities' common
/// denominator, or none when a probability is fixed point or a share would
/// not fit in 128 bits.
fn exact_shares<K: Ord + Clone>(probabilities: &BTreeMap<K, Probability>) -> Option<BTreeMap<K, i128>> {
    let fractions = probabilities.iter().map(|(value, probability)| Some((value, probability.fraction()?))).collect::<Option<Vec<_>>>()?;
    let common = fractions.iter().try_fold(1_i128, |common, (_, (_, denominator))| {
        let divisor = greatest_common_divisor(common, *denominator);
        (common / divisor).checked_mul(*denominator)
    })?;
    fractions.into_iter().map(|(value, (numerator, denominator))| Some((value.clone(), numerator.checked_mul(common / denominator)?))).collect()
}

/// Returns `numerator / denominator` in units of 2^-64, rounded down, for
/// `numerator <= denominator`. Binary long division keeps every step within
/// 128 bits.
fn scaled_down(numerator: u128, denominator: u128) -> u128 {
    if numerator >= denominator {
        return FIXED_ONE;
    }
    let (mut remainder, mut quotient) = (numerator, 0_u128);
    for _ in 0..64 {
        // The remainder stays below a denominator of at most 2^127, so
        // doubling it cannot overflow.
        remainder <<= 1;
        quotient <<= 1;
        if remainder >= denominator {
            remainder -= denominator;
            quotient |= 1;
        }
    }
    quotient
}

/// Compares `a / b` with `c / d` exactly by comparing their continued
/// fraction terms, so no product can overflow.
fn compare_fractions(mut a: u128, mut b: u128, mut c: u128, mut d: u128) -> Ordering {
    let mut reversed = false;
    loop {
        let order = match (a / b).cmp(&(c / d)) {
            Ordering::Equal => match (a % b, c % d) {
                (0, 0) => Ordering::Equal,
                (0, _) => Ordering::Less,
                (_, 0) => Ordering::Greater,
                (left, right) => {
                    // Equal whole parts: compare the remaining fractions by
                    // their reciprocals, which reverses the order.
                    (a, b, c, d) = (b, left, d, right);
                    reversed = !reversed;
                    continue;
                }
            },
            order => order,
        };
        return if reversed { order.reverse() } else { order };
    }
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
    use super::super::{CompletionGrade, Pangine, QuestionSource};
    use super::*;
    use crate::Relevance;

    #[test]
    fn probabilities_reduce_and_compare_exactly() {
        assert_eq!(Probability::new(2, 4).unwrap().fraction(), Some((1, 2)));
        assert_eq!(Probability::new(0, 7).unwrap().fraction(), Some((0, 1)));
        assert!(Probability::new(-1, 2).is_none());
        assert!(Probability::new(1, 0).is_none());
        assert!(Probability::new(3, 2).is_none());
        assert!(Probability::new(1, 3).unwrap() < Probability::new(1, 2).unwrap());
        assert_eq!(Probability::new(9, 40).unwrap().cmp(&Probability::new(18, 80).unwrap()), Ordering::Equal);

        // Fractions whose cross products overflow 128 bits still compare exactly.
        let close = Probability::new(i128::MAX - 2, i128::MAX - 1).unwrap();
        let closer = Probability::new(i128::MAX - 1, i128::MAX).unwrap();
        assert!(close < closer);
        assert_eq!(closer.cmp(&closer), Ordering::Equal);
    }

    #[test]
    fn shares_are_exact_while_they_fit_and_millionths_after() {
        let small = BTreeMap::from([("C-D", Probability::new(9, 40).unwrap()), ("E-D", Probability::new(23, 120).unwrap())]);
        assert_eq!(displayed_shares(&small), BTreeMap::from([("C-D", 27), ("E-D", 23)]));
        assert_eq!(draw_shares(&small), BTreeMap::from([("C-D", 27), ("E-D", 23)]));

        // Two thirds and one third over a denominator past 64 bits: the
        // displayed shares round to millionths and still add to one million.
        let wide = (1_i128 << 70) * 3;
        let large = BTreeMap::from([("a", Probability::new(1 << 71, wide).unwrap()), ("b", Probability::new((1 << 70) + 1, wide).unwrap())]);
        assert_eq!(displayed_shares(&large), BTreeMap::from([("a", 666_667), ("b", 333_333)]));
    }

    #[test]
    fn interpolation_moves_to_fixed_point_past_128_bits() {
        // An exact case and cases at distances 1 to 22 with prime counts: the
        // exact answer would need a 137-bit denominator.
        let mut pangine = Pangine::new();
        let counts = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 2, 3];
        let mut support = CompletionProjectionSupport::new();
        for (level, count) in counts.into_iter().enumerate() {
            let value = pangine.reference_name(&format!("b{level}"));
            let grade = if level == 0 { CompletionGrade::Exact } else { CompletionGrade::Generalized { distance: level } };
            let source = QuestionSource::from_subject(value.clone());
            support.insert(value, BTreeMap::from([((grade, Relevance::DEFAULT, BTreeSet::from([source])), Relevance::new(count))]));
        }

        let probabilities = interpolated_probabilities(&support).expect("fixed-point probabilities");
        let named = |name: &str| probabilities.iter().find(|(value, _)| pangine.get_name(value) == Some(name)).map(|(_, probability)| *probability).unwrap();
        assert!(probabilities.values().all(|probability| probability.fraction().is_none()));
        let total = probabilities.values().map(|probability| probability.fixed()).sum::<u128>();
        assert!(FIXED_ONE - total < 32, "rounding down loses only a few units of 2^-64");

        // The exact values, computed separately with arbitrary precision.
        for (name, expected) in [("b0", 0.778_933_339_697_455), ("b1", 0.168_400_009_546_183), ("b2", 0.042_571_444_481_734), ("b9", 0.000_000_015_010_673)] {
            assert!((named(name).as_f64() - expected).abs() < 1e-15, "{name}");
        }
        assert!(named("b0") > named("b1") && named("b1") > named("b2"));
        assert_eq!(named("b0").to_string(), "0.778933");
        assert_eq!(draw_shares(&probabilities).values().sum::<u128>(), total);
        assert_eq!(displayed_shares(&probabilities).values().sum::<i64>(), 1_000_000);
    }

    #[test]
    fn probabilities_print_as_fractions_or_decimals() {
        assert_eq!(Probability::new(0, 1).unwrap().to_string(), "0");
        assert_eq!(Probability::new(1, 1).unwrap().to_string(), "1");
        assert_eq!(Probability::new(9, 40).unwrap().to_string(), "9/40");
        assert_eq!(Probability::new(1, 1_000_000).unwrap().to_string(), "1/1000000");
        assert_eq!(Probability::new(2, 3_000_001).unwrap().to_string(), "0.000001");
        assert_eq!(Probability::new(108_340_675_114_792_281_569, 156_302_554_713_764_659_200).unwrap().to_string(), "0.693147");
        assert_eq!(Probability::Fixed(FIXED_ONE / 4).to_string(), "0.250000");
    }
}
