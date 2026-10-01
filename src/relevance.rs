/// A signed evidence count.
///
/// Members of unordered Concepts and remembered experiences carry one, written
/// as the `x` coefficient. Remembering an experience again adds one, an
/// inverted member counts minus one, and `@-=` subtracts matching evidence from
/// an answer. Counts combine by exact integer addition, and an operation that
/// would leave the signed 64-bit range fails instead of rounding.
///
/// Answers read counts as probabilities. A value's probability is its share of
/// the positive evidence among the alternatives, and choice takes the most
/// probable value. A graded answer interpolates those shares across its
/// grades. The reading is the relative frequency of remembered evidence, not a
/// calibrated confidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Relevance {
    count: i64,
}

impl Relevance {
    /// No evidence. A member with this count disappears when its Concept is built.
    pub const EMPTY: Self = Self::new(0);

    /// One observation, the count of a member written without a prefix.
    pub const DEFAULT: Self = Self::new(1);

    /// Creates a value from a signed evidence count.
    pub const fn new(count: i64) -> Self {
        Self { count }
    }

    /// Returns the exact sum, or `None` when it exceeds the signed 64-bit range.
    pub fn checked_add(self, adder: Self) -> Option<Self> {
        self.count.checked_add(adder.count).map(Self::new)
    }

    /// Returns the exact difference, or `None` when it exceeds the signed 64-bit range.
    pub fn checked_sub(self, subber: Self) -> Option<Self> {
        self.count.checked_sub(subber.count).map(Self::new)
    }

    /// Returns the exact product, or `None` when it exceeds the signed 64-bit range.
    pub fn checked_mul(self, multiplier: Self) -> Option<Self> {
        self.count.checked_mul(multiplier.count).map(Self::new)
    }

    /// Returns the exact inverse, or `None` for the one unrepresentable negation.
    pub fn checked_neg(self) -> Option<Self> {
        self.count.checked_neg().map(Self::new)
    }

    /// Returns the signed evidence count.
    pub fn count(self) -> i64 {
        self.count
    }

    /// Returns whether this value carries no evidence.
    pub fn is_empty(self) -> bool {
        self.count == 0
    }
}

impl Default for Relevance {
    fn default() -> Self {
        Self::DEFAULT
    }
}
