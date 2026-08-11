/// A signed integer coefficient used by Concept members and reductions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Relevance {
    /// The signed coefficient, written with `x` syntax when attached to a Concept member.
    pub x_coefficient: i64,
}

impl Relevance {
    /// The coefficient representing no contribution.
    pub const EMPTY: Self = Self::new(0);

    /// The default coefficient used when no prefix is written.
    pub const DEFAULT: Self = Self::new(1);

    /// Creates a value from a signed `x` coefficient.
    pub const fn new(x_coefficient: i64) -> Self {
        Self { x_coefficient }
    }

    /// Returns the exact sum, or `None` when it exceeds the signed 64-bit range.
    pub fn checked_add(self, adder: Self) -> Option<Self> {
        self.x_coefficient.checked_add(adder.x_coefficient).map(Self::new)
    }

    /// Returns the exact difference, or `None` when it exceeds the signed 64-bit range.
    pub fn checked_sub(self, subber: Self) -> Option<Self> {
        self.x_coefficient.checked_sub(subber.x_coefficient).map(Self::new)
    }

    /// Returns the exact product, or `None` when it exceeds the signed 64-bit range.
    pub fn checked_mul(self, multiplier: Self) -> Option<Self> {
        self.x_coefficient.checked_mul(multiplier.x_coefficient).map(Self::new)
    }

    /// Returns the exact inverse, or `None` for the one unrepresentable negation.
    pub fn checked_neg(self) -> Option<Self> {
        self.x_coefficient.checked_neg().map(Self::new)
    }

    /// Returns the coefficient used by the current deterministic selection rule.
    pub fn weight(self) -> i64 {
        self.x_coefficient
    }

    /// Returns whether the coefficient contributes no member.
    pub fn is_empty(self) -> bool {
        self.x_coefficient == 0
    }
}

impl Default for Relevance {
    fn default() -> Self {
        Self::DEFAULT
    }
}
