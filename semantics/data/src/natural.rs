//! Extensible natural meaning with caller-admitted finite output storage.
//!
//! The canonical encoding is a nonempty little-endian magnitude: zero is [0],
//! and a multi-byte value has a nonzero last byte. There is no fixed semantic
//! integer width. These pure operations allocate no storage and start no work
//! outside the call. They do not register an authored universal extension.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalRefusal {
    NonCanonical,
    /// Finite realization exhaustion, never semantic zero, HALT, or wrapping.
    CapacityExhausted,
}

/// A borrowed canonical natural. Construction validates the complete value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Natural<'a> {
    bytes: &'a [u8],
}

impl<'a> Natural<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, NaturalRefusal> {
        if bytes.is_empty() || (bytes.len() > 1 && bytes.last() == Some(&0)) {
            return Err(NaturalRefusal::NonCanonical);
        }
        Ok(Self { bytes })
    }

    pub fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    pub fn is_zero(self) -> bool {
        self.bytes == [0]
    }

    /// Computes n + 1. Refusal leaves the entire output buffer unchanged.
    /// Bytes beyond the returned canonical length are also unchanged.
    pub fn successor(self, output: &mut [u8]) -> Result<usize, NaturalRefusal> {
        let grows = self.bytes.iter().all(|byte| *byte == u8::MAX);
        if output.len() < self.bytes.len() || (grows && output.len() == self.bytes.len()) {
            return Err(NaturalRefusal::CapacityExhausted);
        }
        let mut carry = true;
        for (target, source) in output.iter_mut().zip(self.bytes) {
            let (value, overflow) = source.overflowing_add(u8::from(carry));
            *target = value;
            carry = overflow;
        }
        if grows {
            output[self.bytes.len()] = 1;
        }
        Ok(self.bytes.len() + usize::from(grows))
    }

    /// Computes max(n - 1, 0). `is_zero` supplies the exact zero branch;
    /// predecessor of zero is zero, not underflow or an exhaustion result.
    /// Capacity refusal leaves the entire output buffer unchanged.
    pub fn predecessor(self, output: &mut [u8]) -> Result<usize, NaturalRefusal> {
        let shrinks = self.bytes.len() > 1
            && self.bytes.last() == Some(&1)
            && self.bytes[..self.bytes.len() - 1]
                .iter()
                .all(|byte| *byte == 0);
        let length = self.bytes.len() - usize::from(shrinks);
        if output.len() < length {
            return Err(NaturalRefusal::CapacityExhausted);
        }
        let mut borrow = !self.is_zero();
        for (target, source) in output[..length].iter_mut().zip(self.bytes) {
            let (value, underflow) = source.overflowing_sub(u8::from(borrow));
            *target = value;
            borrow = underflow;
        }
        Ok(length)
    }
}
