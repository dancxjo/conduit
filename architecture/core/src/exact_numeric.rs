//! Bounded exact mathematical integers and rationals. This is denotation, not a
//! choice of machine realization. Multi-limb arithmetic spends explicit work;
//! no operation allocates, rounds or changes existing Scalar/Quantity semantics.
mod natural;
mod projection;
mod rational;
pub use projection::*;
pub use rational::*;

pub const EXACT_NUMERIC_MAX_BITS: usize = 4096;
pub const EXACT_NUMERIC_MAX_DECIMAL_BYTES: usize = 1236;
pub const EXACT_RATIONAL_MAX_ENCODED_BYTES: usize = 6 + 2 * EXACT_NUMERIC_MAX_BITS / 8;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExactNumericRefusal {
    InvalidLiteral,
    ExtentExceeded,
    WorkLimit,
    DivisionByZero,
    NonCanonicalEncoding,
    UnsupportedVersion,
    OutOfRange,
    InexactProjection,
    EncodingBufferTooSmall,
}

/// Counts conservative limb work, independently of empirical time measurements.
/// Exhaustion is a refusal; callers admit this budget before numerical work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactNumericWork {
    remaining: u64,
    spent: u64,
}
impl ExactNumericWork {
    pub const fn new(maximum_limb_work: u64) -> Self {
        Self {
            remaining: maximum_limb_work,
            spent: 0,
        }
    }
    pub const fn spent(self) -> u64 {
        self.spent
    }
    pub const fn remaining(self) -> u64 {
        self.remaining
    }
    fn charge(&mut self, amount: u64) -> Result<(), ExactNumericRefusal> {
        if amount > self.remaining {
            return Err(ExactNumericRefusal::WorkLimit);
        }
        self.remaining -= amount;
        self.spent += amount;
        Ok(())
    }
}
