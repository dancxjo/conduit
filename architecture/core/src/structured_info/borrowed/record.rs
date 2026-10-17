//! Borrowed, bounded traversal of already validated canonical records.

use super::{
    malformed, split_type, validate_value, Cursor, StructuredInfoRefusal,
    ValidatedCanonicalStructuredValue, MAXIMUM_STRUCTURED_INFO_NODES,
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
            let mut scratch = MAXIMUM_STRUCTURED_INFO_NODES;
            let (field_type, remaining) = split_type(kind.remaining, 1, &mut scratch)?;
            kind.remaining = remaining;
            if value.text()? != field_name {
                return Err(malformed());
            }
            let beginning = value.remaining;
            validate_value(field_type, &mut value, 1, &mut nodes)?;
            if field_name == name {
                return Ok(Some(Self {
                    type_bytes: field_type,
                    value_node: &beginning[..beginning.len() - value.remaining.len()],
                }));
            }
        }
        Ok(None)
    }

    /// Explicitly inspect a nominal representation while retaining the original
    /// view at the caller. This is not semantic admission or resource authority.
    pub fn representation(self) -> Result<Self, StructuredInfoRefusal> {
        let mut representation = self.type_bytes;
        for _ in 0..super::MAXIMUM_STRUCTURED_INFO_DEPTH {
            let mut kind = Cursor::new(representation);
            if kind.byte()? != 5 {
                return Ok(Self {
                    type_bytes: representation,
                    value_node: self.value_node,
                });
            }
            kind.text()?;
            representation = kind.remaining;
        }
        Err(StructuredInfoRefusal::TooDeep)
    }

    /// Inspect a primitive representation through finite nominal wrappers.
    /// Retain/check the original semantic Type before using this representation.
    pub fn primitive_representation_bytes(
        self,
        identity: &str,
    ) -> Result<&'a [u8], StructuredInfoRefusal> {
        self.representation()?.primitive_bytes(identity)
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
