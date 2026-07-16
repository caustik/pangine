#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relevance {
    pub probability: f32,
    pub strength: f32,
}

impl Relevance {
    pub const DEFAULT: Self = Self::new(1.0, 1.0);

    pub const fn new(probability: f32, strength: f32) -> Self {
        Self {
            probability,
            strength,
        }
    }

    pub fn add(&mut self, adder: Self) {
        self.combine(adder, 1.0);
    }

    pub fn sub(&mut self, subber: Self) {
        self.combine(subber, -1.0);
    }

    fn combine(&mut self, other: Self, sign: f32) {
        let x = self.probability * self.strength + sign * other.probability * other.strength;
        let y = (1.0 - self.probability) * self.strength
            + sign * (1.0 - other.probability) * other.strength;

        let strength = x.abs() + y.abs();
        self.strength = if x > 0.0 { strength } else { -strength };
        self.probability = if self.strength == 0.0 {
            0.0
        } else {
            x / self.strength
        };
    }

    pub fn weight(self) -> f32 {
        self.probability * self.strength
    }

    pub fn is_empty(self) -> bool {
        self.probability == 0.0 || self.strength == 0.0
    }
}

impl Default for Relevance {
    fn default() -> Self {
        Self::DEFAULT
    }
}
