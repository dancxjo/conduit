//! Capacity-bounded natural meaning with caller-admitted finite output storage.
//!
//! The canonical encoding is a nonempty little-endian magnitude: zero is [0],
//! and a multi-byte value has a nonzero last byte. Every value carries an exact
//! finite semantic byte capacity selected by its caller. These pure operations
//! allocate no storage and start no work outside the call.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalRefusal {
    NonCanonical,
    /// The mathematical result is outside the value's declared finite domain.
    DomainOverflow,
    /// Finite realization exhaustion, never semantic zero, HALT, or wrapping.
    CapacityExhausted,
}

/// The finite semantic magnitude capacity chosen when a reusable operation is
/// specialized. Zero bytes cannot encode even the canonical natural zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NaturalDomain {
    maximum_bytes: usize,
}

impl NaturalDomain {
    pub fn new(maximum_bytes: usize) -> Result<Self, NaturalRefusal> {
        if maximum_bytes == 0 {
            return Err(NaturalRefusal::DomainOverflow);
        }
        Ok(Self { maximum_bytes })
    }

    pub fn maximum_bytes(self) -> usize {
        self.maximum_bytes
    }
}

/// A borrowed canonical natural. Construction validates the complete value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Natural<'a> {
    domain: NaturalDomain,
    bytes: &'a [u8],
}

impl<'a> Natural<'a> {
    pub fn from_bytes(domain: NaturalDomain, bytes: &'a [u8]) -> Result<Self, NaturalRefusal> {
        if bytes.is_empty() || (bytes.len() > 1 && bytes.last() == Some(&0)) {
            return Err(NaturalRefusal::NonCanonical);
        }
        if bytes.len() > domain.maximum_bytes {
            return Err(NaturalRefusal::DomainOverflow);
        }
        Ok(Self { domain, bytes })
    }

    pub fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    pub fn domain(self) -> NaturalDomain {
        self.domain
    }

    pub fn is_zero(self) -> bool {
        self.bytes == [0]
    }

    /// Computes n + 1. Refusal leaves the entire output buffer unchanged.
    /// Bytes beyond the returned canonical length are also unchanged.
    pub fn successor(self, output: &mut [u8]) -> Result<usize, NaturalRefusal> {
        let grows = self.bytes.iter().all(|byte| *byte == u8::MAX);
        let result_length = self.bytes.len() + usize::from(grows);
        if result_length > self.domain.maximum_bytes {
            return Err(NaturalRefusal::DomainOverflow);
        }
        if output.len() < result_length {
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
        Ok(result_length)
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
