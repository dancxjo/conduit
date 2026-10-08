//! Aggregate quotas for an explicitly owned ordered law bank.
use super::{PreparedNativeInvariantAdmission, PreparedNativeInvariantRefusal as Refusal};
use crate::{
    PortableExpressionProgram, PreparedExpressionStorageRefusal,
    PreparedPortableExpressionEvaluator,
};
use alloc::vec::Vec;
use conduit_core::StructuredInfoType;
use core::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedNativeInvariantStorageLimits {
    pub maximum_laws: usize,
    pub maximum_input_bytes: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedNativeInvariantStorageReceipt {
    pub retained_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
}
impl PreparedNativeInvariantAdmission {
    /// Decodes one complete supplied law at a time, retaining only its evaluator.
    /// Both decoding and preparation allocations are admitted before allocation.
    /// Peak accounting includes the borrowed caller Type; encoded input buffers
    /// and other external owners must be accounted for by the enclosing family.
    pub fn from_canonical_laws_with_storage_limits(
        value_type: &StructuredInfoType,
        encoded_laws: &[&[u8]],
        limits: PreparedNativeInvariantStorageLimits,
    ) -> Result<(Self, PreparedNativeInvariantStorageReceipt), Refusal> {
        if encoded_laws.len() > limits.maximum_laws {
            return Err(Refusal::Capacity);
        }
        let mut retained = encoded_laws
            .len()
            .checked_mul(size_of::<PreparedPortableExpressionEvaluator>())
            .ok_or(Refusal::Capacity)?;
        let caller_type = value_type.owned_heap_bytes();
        let mut peak = retained.checked_add(caller_type).ok_or(Refusal::Capacity)?;
        if caller_type == usize::MAX
            || retained > limits.maximum_retained_bytes
            || peak > limits.maximum_preparation_peak_bytes
        {
            return Err(Refusal::Capacity);
        }
        let mut laws = Vec::with_capacity(encoded_laws.len());
        for (index, encoded) in encoded_laws.iter().enumerate() {
            let invalid_encoding = |refusal| Refusal::InvalidEncoding { index, refusal };
            let invalid_law = |refusal| Refusal::InvalidLaw { index, refusal };
            let base = retained.checked_add(caller_type).ok_or(Refusal::Capacity)?;
            let available_peak = limits
                .maximum_preparation_peak_bytes
                .checked_sub(base)
                .ok_or(Refusal::Capacity)?;
            let decode_bound = PortableExpressionProgram::canonical_decode_storage_bound(encoded)
                .map_err(invalid_encoding)?;
            if decode_bound > available_peak {
                return Err(Refusal::Capacity);
            }
            peak = peak.max(base.checked_add(decode_bound).ok_or(Refusal::Capacity)?);
            let law = PortableExpressionProgram::from_canonical_bytes_with_storage_limit(
                encoded,
                available_peak,
            )
            .map_err(invalid_encoding)?;
            if &law.input_type != value_type
                || !matches!(law.output_type.shape(), conduit_core::StructuredInfoTypeShape::Leaf(name) if name.as_str() == conduit_core::BOOL_INFO_ID)
            {
                return Err(invalid_law(
                    crate::PortableExpressionEvaluationRefusal::InvalidProgram,
                ));
            }
            if law.maximum_prepared_input_bytes().map_err(invalid_law)? as usize
                > limits.maximum_input_bytes
            {
                return Err(Refusal::Capacity);
            }
            let remaining_retained = limits
                .maximum_retained_bytes
                .checked_sub(retained)
                .ok_or(Refusal::Capacity)?;
            let (prepared, receipt) = PreparedPortableExpressionEvaluator::new_with_storage_limits(
                &law,
                available_peak,
                available_peak,
                remaining_retained,
            )
            .map_err(|refusal| match refusal {
                PreparedExpressionStorageRefusal::Capacity => Refusal::Capacity,
                PreparedExpressionStorageRefusal::UnsupportedTemporaryOperation => {
                    Refusal::UnsupportedTemporaryLaw { index }
                }
                PreparedExpressionStorageRefusal::Expression(refusal) => invalid_law(refusal),
            })?;
            peak = peak.max(
                base.checked_add(receipt.decoded_program_heap_bytes)
                    .and_then(|bytes| bytes.checked_add(receipt.preparation_requested_bytes_bound))
                    .ok_or(Refusal::Capacity)?,
            );
            retained = retained
                .checked_add(receipt.retained_heap_bytes_bound)
                .ok_or(Refusal::Capacity)?;
            peak = peak.max(retained.checked_add(caller_type).ok_or(Refusal::Capacity)?);
            if peak > limits.maximum_preparation_peak_bytes {
                return Err(Refusal::Capacity);
            }
            drop(law); // no decoded AST survives into the next law's phase
            laws.push(prepared);
        }
        Ok((
            Self {
                laws,
                maximum_input_bytes: limits.maximum_input_bytes,
            },
            PreparedNativeInvariantStorageReceipt {
                retained_heap_bytes_bound: retained,
                preparation_peak_heap_bytes_bound: peak,
            },
        ))
    }
    pub fn owned_heap_bytes(&self) -> usize {
        self.laws.iter().fold(
            self.laws
                .capacity()
                .saturating_mul(size_of::<PreparedPortableExpressionEvaluator>()),
            |bytes, law| bytes.saturating_add(law.owned_heap_bytes()),
        )
    }
}
