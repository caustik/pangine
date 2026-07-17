/// The probability and signed strength assigned to a concept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relevance {
    /// The probability of the concept, normally between zero and one.
    pub probability: f32,
    /// The signed amount of evidence supporting the probability.
    pub strength: f32,
}

impl Relevance {
    /// Default relevance: fully probable with one unit of strength.
    pub const DEFAULT: Self = Self::new(1.0, 1.0);

    /// Creates a relevance value from its probability and signed strength.
    pub const fn new(probability: f32, strength: f32) -> Self {
        Self { probability, strength }
    }

    /// Accumulates supporting or opposing relevance.
    pub fn add(&mut self, adder: Self) {
        self.combine(adder, 1.0);
    }

    /// Removes supporting or opposing relevance.
    pub fn sub(&mut self, subber: Self) {
        self.combine(subber, -1.0);
    }

    fn combine(&mut self, other: Self, sign: f32) {
        let x = self.probability * self.strength + sign * other.probability * other.strength;
        let y = (1.0 - self.probability) * self.strength + sign * (1.0 - other.probability) * other.strength;

        let strength = x.abs() + y.abs();
        self.strength = if x > 0.0 { strength } else { -strength };
        self.probability = if self.strength == 0.0 { 0.0 } else { x / self.strength };
    }

    /// Returns the probability weighted by signed strength.
    pub fn weight(self) -> f32 {
        self.probability * self.strength
    }

    /// Returns whether the relevance contributes no weighted evidence.
    pub fn is_empty(self) -> bool {
        self.probability == 0.0 || self.strength == 0.0
    }
}

impl Default for Relevance {
    fn default() -> Self {
        Self::DEFAULT
    }
}
