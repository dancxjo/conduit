//! Sequential traversal never rescans already consumed immutable elements.
use super::validated_extent::{skip_validated_value, split_validated_type};
use super::{
    malformed, Cursor, StructuredInfoRefusal, ValidatedCanonicalStructuredValue,
    MAXIMUM_STRUCTURED_INFO_NODES,
};

pub struct ValidatedCanonicalCollectionIter<'a> {
    element_type: &'a [u8],
    values: Cursor<'a>,
    remaining: usize,
    nodes: usize,
}

impl<'a> ValidatedCanonicalStructuredValue<'a> {
    /// Borrows each sequence or fixed collection element once, in exact order.
    /// The iterator retains the complete immutable element Type established by
    /// validation of the original canonical value. It allocates no storage.
    pub fn collection_elements(
        self,
    ) -> Result<ValidatedCanonicalCollectionIter<'a>, StructuredInfoRefusal> {
        let mut kind = Cursor::new(self.type_bytes);
        match kind.byte()? {
            1 => {
                kind.u16()?;
            }
            4 => {
                kind.u16()?;
                kind.u16()?;
            }
            _ => return Err(StructuredInfoRefusal::WrongType),
        }
        let (element_type, rest) = split_validated_type(kind.remaining)?;
        if !rest.is_empty() {
            return Err(malformed());
        }
        let mut values = Cursor::new(self.value_node);
        if values.byte()? != 1 {
            return Err(malformed());
        }
        let remaining = values.length()?;
        Ok(ValidatedCanonicalCollectionIter {
            element_type,
            values,
            remaining,
            nodes: MAXIMUM_STRUCTURED_INFO_NODES,
        })
    }
}

impl<'a> Iterator for ValidatedCanonicalCollectionIter<'a> {
    type Item = Result<ValidatedCanonicalStructuredValue<'a>, StructuredInfoRefusal>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let beginning = self.values.remaining;
        if let Err(refusal) = skip_validated_value(&mut self.values, &mut self.nodes) {
            self.remaining = 0;
            return Some(Err(refusal));
        }
        self.remaining -= 1;
        Some(Ok(ValidatedCanonicalStructuredValue {
            type_bytes: self.element_type,
            value_node: &beginning[..beginning.len() - self.values.remaining.len()],
        }))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for ValidatedCanonicalCollectionIter<'_> {}
impl core::iter::FusedIterator for ValidatedCanonicalCollectionIter<'_> {}
