//! Borrowed, bounded traversal of already validated canonical records.

use super::validated_extent::{skip_validated_value, split_validated_type};
use super::{
    malformed, Cursor, StructuredInfoRefusal, ValidatedCanonicalStructuredValue,
    MAXIMUM_STRUCTURED_INFO_NODES,
};

impl<'a> ValidatedCanonicalStructuredValue<'a> {
    /// Borrow one exact record member without allocating an owned value tree.
    /// The returned view retains its original type, including nominal identity.
    pub fn record_field(self, name: &str) -> Result<Option<Self>, StructuredInfoRefusal> {
        let mut kind = Cursor::new(self.type_bytes);
        if kind.byte()? != 2 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        kind.text()?;
        let count = kind.length()?;
        let mut value = Cursor::new(self.value_node);
        if value.byte()? != 2 || value.length()? != count {
            return Err(malformed());
        }
        let mut nodes = MAXIMUM_STRUCTURED_INFO_NODES;
        for _ in 0..count {
            let field_name = kind.text()?;
            let (field_type, remaining) = split_validated_type(kind.remaining)?;
            kind.remaining = remaining;
            if value.text()? != field_name {
                return Err(malformed());
            }
            let beginning = value.remaining;
            skip_validated_value(&mut value, &mut nodes)?;
            if field_name == name {
                return Ok(Some(Self {
                    type_bytes: field_type,
                    value_node: &beginning[..beginning.len() - value.remaining.len()],
                }));
            }
        }
        Ok(None)
    }

    /// Borrow the primitive bytes of a leaf with the exact supplied identity.
    pub fn primitive_bytes(self, identity: &str) -> Result<&'a [u8], StructuredInfoRefusal> {
        let mut kind = Cursor::new(self.type_bytes);
        if kind.byte()? != 0 || kind.text()? != identity || !kind.remaining.is_empty() {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let mut value = Cursor::new(self.value_node);
        if value.byte()? != 0 {
            return Err(malformed());
        }
        let bytes = value.bytes()?;
        if !value.remaining.is_empty() {
            return Err(malformed());
        }
        Ok(bytes)
    }
}
