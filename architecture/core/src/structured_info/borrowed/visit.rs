//! Bounded, allocation-free traversal of complete validated canonical subtrees.
use super::validated_extent::split_validated_type;
use super::{
    Cursor, StructuredInfoRefusal, ValidatedCanonicalStructuredValue,
    MAXIMUM_STRUCTURED_INFO_DEPTH, MAXIMUM_STRUCTURED_INFO_NODES,
};
impl<'a> ValidatedCanonicalStructuredValue<'a> {
    /// Visits children before their parent, preserving each complete original
    /// Type and immutable value range. Traversal grants no semantic admission.
    pub fn try_visit_nodes<E: From<StructuredInfoRefusal>>(
        self,
        visitor: &mut impl FnMut(Self) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut remaining = MAXIMUM_STRUCTURED_INFO_NODES;
        self.visit(1, &mut remaining, visitor)
    }
    fn visit<E: From<StructuredInfoRefusal>>(
        self,
        depth: usize,
        remaining: &mut usize,
        visitor: &mut impl FnMut(Self) -> Result<(), E>,
    ) -> Result<(), E> {
        if depth > MAXIMUM_STRUCTURED_INFO_DEPTH {
            return Err(StructuredInfoRefusal::TooDeep.into());
        }
        *remaining = remaining
            .checked_sub(1)
            .ok_or(StructuredInfoRefusal::TooManyNodes)?;
        match self.type_bytes[0] {
            0 => {}
            1 | 4 => {
                for child in self.collection_elements()? {
                    child?.visit(depth + 1, remaining, visitor)?;
                }
            }
            2 => {
                let mut kind = Cursor::new(self.type_bytes);
                kind.byte()?;
                kind.text()?;
                let count = kind.length()?;
                for _ in 0..count {
                    let name = kind.text()?;
                    let (_, rest) = split_validated_type(kind.remaining)?;
                    kind.remaining = rest;
                    self.record_field(name)?
                        .ok_or(StructuredInfoRefusal::WrongType)?
                        .visit(depth + 1, remaining, visitor)?;
                }
            }
            3 => self
                .variant_payload(self.variant_tag()?)?
                .ok_or(StructuredInfoRefusal::WrongType)?
                .visit(depth + 1, remaining, visitor)?,
            5 => self
                .nominal_representation()?
                .visit(depth + 1, remaining, visitor)?,
            _ => return Err(StructuredInfoRefusal::WrongType.into()),
        }
        visitor(self)
    }
}
