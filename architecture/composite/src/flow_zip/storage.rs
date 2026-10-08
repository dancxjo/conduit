//! Explicit typed preparation inventory. Selection/laws remain caller-owned.
use super::*;
use conduit_core::{
    PreparedStructuredCompositionStorageReceipt as Receipt,
    PreparedStructuredCompositionStorageRefusal as Error,
};
use core::mem::size_of;
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::Capacity)
}
fn transport_matches(
    ty: &StructuredInfoType,
    contract: &CheckedValueContract,
) -> Result<bool, StructuredInfoRefusal> {
    if let StructuredInfoTypeShape::Leaf(kind) = ty.shape() {
        return Ok(kind == &contract.value_kind);
    }
    let digest = ty.semantic_digest()?;
    let prefix = b"structured-info/profile-";
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0u8; "structured-info/profile-".len() + 64 + 2];
    // Exact profile identity is formatted into a fixed stack array.
    bytes[..prefix.len()].copy_from_slice(prefix);
    for (i, b) in digest.iter().enumerate() {
        bytes[prefix.len() + 2 * i] = HEX[(b >> 4) as usize];
        bytes[prefix.len() + 2 * i + 1] = HEX[(b & 15) as usize];
    }
    let end = prefix.len() + 64;
    bytes[end..end + 2].copy_from_slice(b"@1");
    Ok(contract.value_kind.as_str().as_bytes() == &bytes[..end + 2])
}
impl FlowZipBack {
    pub fn typed_storage_reservation(
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
    ) -> Result<Receipt, Error> {
        let mut r = PreparedTypedTuplePairEncoder::storage_reservation(
            left_type,
            left.maximum_bytes,
            right_type,
            right.maximum_bytes,
        )?;
        let buffers = add(
            add(left.maximum_bytes as usize, right.maximum_bytes as usize)?,
            left.maximum_bytes.max(right.maximum_bytes) as usize,
        )?;
        let retained = add(buffers, size_of::<PreparedTypedTuplePairEncoder>())?;
        r.retained_heap_bytes_bound = add(r.retained_heap_bytes_bound, retained)?;
        r.preparation_requested_bytes_bound = add(r.preparation_requested_bytes_bound, retained)?;
        for ty in [left_type, right_type] {
            if !matches!(ty.shape(), StructuredInfoTypeShape::Leaf(_)) {
                r.preparation_requested_bytes_bound = add(
                    r.preparation_requested_bytes_bound,
                    ty.canonical_byte_length().map_err(Error::Structured)?,
                )?;
            }
        }
        Ok(r)
    }
    /// Quotas precede all identity hashing, Type cloning, encoder boxing and
    /// buffers. Exact selected contracts must still be admitted by the caller.
    pub fn prepare_typed_with_storage_limits(
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<(Self, Receipt), Error> {
        let r = Self::typed_storage_reservation(left, left_type, right, right_type)?;
        if r.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || r.retained_heap_bytes_bound > maximum_retained_bytes
        {
            return Err(Error::Capacity);
        }
        if !transport_matches(left_type, left).map_err(Error::Structured)?
            || !transport_matches(right_type, right).map_err(Error::Structured)?
        {
            return Err(Error::Structured(StructuredInfoRefusal::WrongType));
        }
        let pair = PreparedTypedTuplePairEncoder::storage_reservation(
            left_type,
            left.maximum_bytes,
            right_type,
            right.maximum_bytes,
        )?;
        let (encoder, _) = PreparedTypedTuplePairEncoder::new_with_storage_limits(
            left_type,
            left.maximum_bytes,
            right_type,
            right.maximum_bytes,
            pair.preparation_requested_bytes_bound,
            pair.retained_heap_bytes_bound,
        )?;
        Ok((
            Self {
                left: vec![0; left.maximum_bytes as usize],
                left_len: None,
                right: vec![0; right.maximum_bytes as usize],
                right_len: None,
                candidate: vec![0; left.maximum_bytes.max(right.maximum_bytes) as usize],
                candidate_side: None,
                output_maximum: encoder.maximum_bytes(),
                encoder: PairEncoder::Typed(Box::new(encoder)),
                output_staged: false,
                finish_after_commit: false,
                closing: false,
                terminal: false,
                finite: false,
                feedback: false,
                return_owed: false,
                right_closed: false,
            },
            r,
        ))
    }
    pub fn prepare_typed_finite_with_storage_limits(
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<(Self, Receipt), Error> {
        let (mut back, r) = Self::prepare_typed_with_storage_limits(
            left,
            left_type,
            right,
            right_type,
            maximum_preparation_requested_bytes,
            maximum_retained_bytes,
        )?;
        back.finite = true;
        Ok((back, r))
    }
    pub fn prepare_typed_feedback_with_storage_limits(
        left: &CheckedValueContract,
        left_type: &StructuredInfoType,
        right: &CheckedValueContract,
        right_type: &StructuredInfoType,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<(Self, Receipt), Error> {
        let (mut back, r) = Self::prepare_typed_finite_with_storage_limits(
            left,
            left_type,
            right,
            right_type,
            maximum_preparation_requested_bytes,
            maximum_retained_bytes,
        )?;
        back.feedback = true;
        back.return_owed = true;
        Ok((back, r))
    }
    /// Actual payload capacities for the typed preparation profile. Primitive
    /// legacy encoder inventory is explicitly unsupported by this entrance.
    pub fn typed_owned_heap_bytes(&self) -> Result<usize, Error> {
        let PairEncoder::Typed(encoder) = &self.encoder else {
            return Err(Error::Structured(StructuredInfoRefusal::WrongType));
        };
        add(
            add(
                add(self.left.capacity(), self.right.capacity())?,
                self.candidate.capacity(),
            )?,
            add(
                size_of::<PreparedTypedTuplePairEncoder>(),
                encoder.owned_heap_bytes(),
            )?,
        )
    }
}
