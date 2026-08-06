/// The signed `x` coefficient attached to an unordered member.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relevance {
    /// The signed coefficient written with `x` syntax.
    pub x_coefficient: f32,
}

impl Relevance {
    /// The default coefficient used when no prefix is written.
    pub const DEFAULT: Self = Self::new(1.0);

    /// Creates a value from a signed `x` coefficient.
    pub const fn new(x_coefficient: f32) -> Self {
        Self { x_coefficient }
    }

    /// Adds another coefficient.
    pub fn add(&mut self, adder: Self) {
        self.x_coefficient += adder.x_coefficient;
    }

    /// Subtracts another coefficient.
    pub fn sub(&mut self, subber: Self) {
        self.x_coefficient -= subber.x_coefficient;
    }

    /// Returns the coefficient used by the current deterministic selection rule.
    pub fn weight(self) -> f32 {
        self.x_coefficient
    }

    /// Returns whether the coefficient contributes no member.
    pub fn is_empty(self) -> bool {
        self.x_coefficient == 0.0
    }
}

impl Default for Relevance {
    fn default() -> Self {
        Self::DEFAULT
    }
}
