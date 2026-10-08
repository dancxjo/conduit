//! Separately admitted preparation with identical canonical tuple identity.
use super::super::{
    MAXIMUM_STRUCTURED_INFO_DEPTH, PreparedStructuredCompositionStorageReceipt as Receipt,
    PreparedStructuredCompositionStorageRefusal as Error, StructuredFieldType,
};
use super::*;
use alloc::string::String;
use core::mem::size_of;
const FIELD_NAMES: [&str; 2] = ["item-00000", "item-00001"];
const PROFILE_PREFIX: &str = "structured-info/profile-";
const TUPLE_PREFIX: &str = "conduitese/anonymous-tuple-";
const HEX: &[u8; 16] = b"0123456789abcdef";
fn add(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_add(b).ok_or(Error::Capacity)
}
struct Reservation {
    receipt: Receipt,
    maximum: usize,
    prefix: usize,
    identity: usize,
}
fn reservation(
    left: &StructuredInfoType,
    lmax: u32,
    right: &StructuredInfoType,
    rmax: u32,
) -> Result<Reservation, Error> {
    let lengths = [
        left.canonical_byte_length().map_err(Error::Structured)?,
        right.canonical_byte_length().map_err(Error::Structured)?,
    ];
    let types = [left, right];
    let maxima = [lmax as usize, rmax as usize];
    let mut nodes = 1usize;
    for ty in types {
        nodes = add(nodes, maximum_value_nodes(ty).map_err(Error::Structured)?)?;
        if nodes > MAXIMUM_STRUCTURED_INFO_NODES {
            return Err(Error::Structured(Refusal::TooManyNodes));
        }
        let (depth, _) = super::super::canonical::type_extent(ty);
        if depth >= MAXIMUM_STRUCTURED_INFO_DEPTH {
            return Err(Error::Structured(Refusal::TooDeep));
        }
    }
    let kind_len = TUPLE_PREFIX.len() + 64 + 2;
    let prefix = add(
        add(9 + kind_len, 2 * (4 + FIELD_NAMES[0].len()))?,
        add(lengths[0], lengths[1])?,
    )?;
    if prefix > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(Error::Structured(Refusal::CanonicalEncodingTooLarge));
    }
    let mut maximum = add(prefix, 5 + 2 * (4 + FIELD_NAMES[0].len()))?;
    let mut member_retained = 0usize;
    for i in 0..2 {
        if maxima[i] > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(Error::Structured(Refusal::CanonicalEncodingTooLarge));
        }
        let node = if matches!(types[i].shape(), StructuredInfoTypeShape::Leaf(_)) {
            add(5, maxima[i])?
        } else {
            maxima[i]
                .checked_sub(lengths[i])
                .ok_or(Error::Structured(Refusal::CanonicalEncodingTooLarge))?
        };
        maximum = add(maximum, node)?;
        member_retained = add(member_retained, lengths[i])?;
        if matches!(types[i].shape(), StructuredInfoTypeShape::Leaf(_)) {
            let primitive_max = add(add(lengths[i], 5)?, maxima[i])?;
            let r = PreparedStructuredComposer::storage_reservation(types[i], primitive_max)?;
            member_retained = add(member_retained, r.retained_heap_bytes_bound)?;
        }
    }
    if maximum > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(Error::Structured(Refusal::CanonicalEncodingTooLarge));
    }
    let owned = add(left.owned_heap_bytes(), right.owned_heap_bytes())?;
    let type_heap = add(
        add(
            2 * size_of::<StructuredFieldType>(),
            2 * FIELD_NAMES[0].len() + kind_len,
        )?,
        owned,
    )?;
    // Exact final record composer fields, child canonical buffers, prefix/output.
    let composer = add(
        add(prefix, maximum)?,
        add(
            2 * size_of::<super::super::prepared_composition::Field>() + 2 * FIELD_NAMES[0].len(),
            add(lengths[0], lengths[1])?,
        )?,
    )?;
    let retained = add(add(type_heap, member_retained)?, composer)?;
    let identity = 2 * (8 + FIELD_NAMES[0].len() + 8 + PROFILE_PREFIX.len() + 64 + 2);
    // Temporary child digest encodings, tuple identity buffer, and the record
    // constructor's complete canonical limit-validation buffer. No Type profile
    // clone, formatting scratch, growth, or digest hex String is constructed.
    let preparation = add(
        retained,
        add(add(lengths[0], lengths[1])?, add(identity, prefix)?)?,
    )?;
    Ok(Reservation {
        receipt: Receipt {
            preparation_requested_bytes_bound: preparation,
            retained_heap_bytes_bound: retained,
        },
        maximum,
        prefix,
        identity,
    })
}
fn push_hex(bytes: &[u8], out: &mut Vec<u8>) {
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize]);
        out.push(HEX[(byte & 15) as usize]);
    }
}
fn tuple(
    left: &StructuredInfoType,
    right: &StructuredInfoType,
    identity_bytes: usize,
) -> Result<StructuredInfoType, Refusal> {
    let mut identity = Vec::with_capacity(identity_bytes);
    for (name, ty) in FIELD_NAMES.into_iter().zip([left, right]) {
        identity.extend_from_slice(&(name.len() as u64).to_le_bytes());
        identity.extend_from_slice(name.as_bytes());
        let kind_len = PROFILE_PREFIX.len() + 64 + 2;
        identity.extend_from_slice(&(kind_len as u64).to_le_bytes());
        identity.extend_from_slice(PROFILE_PREFIX.as_bytes());
        push_hex(&ty.semantic_digest()?, &mut identity);
        identity.extend_from_slice(b"@1");
    }
    let digest = crate::semantic_digest("conduit.conduitese.anonymous-tuple.v1", &identity);
    let mut kind = String::with_capacity(TUPLE_PREFIX.len() + 64 + 2);
    kind.push_str(TUPLE_PREFIX);
    for byte in digest {
        kind.push(char::from(HEX[(byte >> 4) as usize]));
        kind.push(char::from(HEX[(byte & 15) as usize]));
    }
    kind.push_str("@1");
    let mut fields = Vec::with_capacity(2);
    for (name, ty) in FIELD_NAMES.into_iter().zip([left, right]) {
        fields.push(StructuredFieldType::new(name, ty.clone())?);
    }
    StructuredInfoType::record(crate::KindId::new(kind), fields)
}
impl PreparedTypedTuplePairEncoder {
    /// Borrows original Types; admitted preparation includes retained clones.
    /// Reservation performs no allocation and retains full canonical semantics.
    pub fn storage_reservation(
        left: &StructuredInfoType,
        left_maximum: u32,
        right: &StructuredInfoType,
        right_maximum: u32,
    ) -> Result<Receipt, Error> {
        Ok(reservation(left, left_maximum, right, right_maximum)?.receipt)
    }
    pub fn new_with_storage_limits(
        left: &StructuredInfoType,
        left_maximum: u32,
        right: &StructuredInfoType,
        right_maximum: u32,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<(Self, Receipt), Error> {
        let r = reservation(left, left_maximum, right, right_maximum)?;
        if r.receipt.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || r.receipt.retained_heap_bytes_bound > maximum_retained_bytes
        {
            return Err(Error::Capacity);
        }
        let value_type = tuple(left, right, r.identity).map_err(Error::Structured)?;
        debug_assert_eq!(
            value_type
                .canonical_byte_length()
                .map_err(Error::Structured)?,
            r.prefix
        );
        maximum_value_nodes(&value_type).map_err(Error::Structured)?;
        let composer =
            PreparedStructuredComposer::new(&value_type, r.maximum).map_err(Error::Structured)?;
        let encoder = Self {
            left: Member::new(left, left_maximum).map_err(Error::Structured)?,
            right: Member::new(right, right_maximum).map_err(Error::Structured)?,
            value_type,
            composer,
            maximum_bytes: r.maximum as u32,
        };
        Ok((encoder, r.receipt))
    }
    pub fn owned_heap_bytes(&self) -> usize {
        let member = |m: &Member| {
            m.schema.capacity().saturating_add(
                m.primitive
                    .as_ref()
                    .map_or(0, PreparedStructuredComposer::owned_heap_bytes),
            )
        };
        self.value_type
            .owned_heap_bytes()
            .saturating_add(self.composer.owned_heap_bytes())
            .saturating_add(member(&self.left))
            .saturating_add(member(&self.right))
    }
}
