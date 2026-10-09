//! Finite field recipes pinned to one complete canonical record Type.
use super::validated_extent::{skip_validated_value, split_validated_type};
use super::{
    Cursor, StructuredInfoRefusal, ValidatedCanonicalStructuredValue, MAXIMUM_STRUCTURED_INFO_NODES,
};
use alloc::vec::Vec;

struct Field<'a> {
    name: &'a str,
    ty: &'a [u8],
}
/// Reuses only framing work. This grants no Native or law admission.
pub struct PreparedCanonicalRecordAccess<'a> {
    ty: &'a [u8],
    fields: Vec<Field<'a>>,
}
impl<'a> PreparedCanonicalRecordAccess<'a> {
    /// Validates the complete Type and reports the exact requested field storage
    /// without allocating. The caller must reserve this before construction.
    pub fn storage_bound(ty: &[u8]) -> Result<usize, StructuredInfoRefusal> {
        let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        let (_, rest) = super::split_type(ty, 1, &mut nodes)?;
        if !rest.is_empty() {
            return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
        }
        let mut cursor = Cursor::new(ty);
        if cursor.byte()? != 2 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        cursor.text()?;
        cursor
            .length()?
            .checked_mul(core::mem::size_of::<Field<'_>>())
            .ok_or(StructuredInfoRefusal::TooManyFields)
    }
    /// A one-under ceiling refuses before any allocation.
    pub fn prepare(
        ty: &'a [u8],
        maximum_storage_bytes: usize,
    ) -> Result<Self, StructuredInfoRefusal> {
        let bound = Self::storage_bound(ty)?;
        if bound > maximum_storage_bytes {
            return Err(StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        let mut cursor = Cursor::new(ty);
        cursor.byte()?;
        cursor.text()?;
        let count = cursor.length()?;
        let mut fields = Vec::with_capacity(count);
        for _ in 0..count {
            let name = cursor.text()?;
            let (ty, remaining) = split_validated_type(cursor.remaining)?;
            cursor.remaining = remaining;
            fields.push(Field { name, ty });
        }
        Ok(Self { ty, fields })
    }
    /// Checks the full original Type, then delimits immutable value extents.
    pub fn field<'v>(
        &self,
        value: ValidatedCanonicalStructuredValue<'v>,
        name: &str,
    ) -> Result<Option<ValidatedCanonicalStructuredValue<'v>>, StructuredInfoRefusal>
    where
        'a: 'v,
    {
        if value.type_bytes() != self.ty {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let mut cursor = Cursor::new(value.value_node());
        if cursor.byte()? != 2 || cursor.length()? != self.fields.len() {
            return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
        }
        let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        for field in &self.fields {
            if cursor.text()? != field.name {
                return Err(StructuredInfoRefusal::MalformedCanonicalEncoding);
            }
            let start = cursor.remaining;
            skip_validated_value(&mut cursor, &mut nodes)?;
            if field.name == name {
                return Ok(Some(ValidatedCanonicalStructuredValue {
                    type_bytes: field.ty,
                    value_node: &start[..start.len() - cursor.remaining.len()],
                }));
            }
        }
        Ok(None)
    }
}
