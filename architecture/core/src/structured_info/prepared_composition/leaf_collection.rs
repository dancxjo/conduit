//! Compose primitive sequences with the same prepared schema and output buffer.
use super::super::canonical::Cursor;
use super::*;
impl PreparedStructuredComposer {
    /// Preflight primitive identity, counts and extent before replacing output.
    /// The iterator is replayed after validation; no child values are allocated.
    /// Changed replay refuses and clears the attempt; it cannot grow storage.
    pub fn leaf_collection<'a>(
        &mut self,
        values: impl ExactSizeIterator<Item = &'a [u8]> + Clone,
    ) -> Result<&[u8], Refusal> {
        let CompositionShape::Collection {
            element_type,
            minimum_items,
            maximum_items,
        } = self.shape
        else {
            return Err(Refusal::WrongType);
        };
        if values.len() < usize::from(minimum_items) || values.len() > usize::from(maximum_items) {
            return Err(Refusal::WrongCollectionLength);
        }
        let mut schema = Cursor::new(element_type.bytes(&self.prefix));
        if schema.byte()? != 0 {
            return Err(Refusal::WrongType);
        }
        let kind = schema.text_ref()?;
        if !schema.remaining.is_empty() {
            return Err(Refusal::WrongType);
        }
        let count = values.len();
        let mut seen = 0usize;
        let mut size = 5usize;
        for value in values.clone() {
            if seen == count {
                return Err(Refusal::WrongCollectionLength);
            }
            seen += 1;
            size = size
                .checked_add(5)
                .and_then(|n| n.checked_add(value.len()))
                .ok_or(Refusal::CanonicalEncodingTooLarge)?;
            self.admit_size(size)?;
            crate::validate_primitive_info(kind, value).map_err(Refusal::InvalidPrimitiveLeaf)?;
        }
        if seen != count {
            return Err(Refusal::WrongCollectionLength);
        }
        self.admit_size(size)?;
        self.output.clear();
        self.output.extend_from_slice(&self.prefix);
        self.output.push(1);
        self.output
            .extend_from_slice(&(values.len() as u32).to_le_bytes());
        let mut seen = 0usize;
        for value in values {
            if seen == count {
                self.output.clear();
                return Err(Refusal::WrongCollectionLength);
            }
            seen += 1;
            if self
                .output
                .len()
                .checked_add(5)
                .and_then(|n| n.checked_add(value.len()))
                .filter(|n| *n <= self.maximum_bytes && *n <= self.output.capacity())
                .is_none()
            {
                self.output.clear();
                return Err(Refusal::CanonicalEncodingTooLarge);
            }
            self.output.push(0);
            push_bytes(value, &mut self.output);
        }
        if seen != count {
            self.output.clear();
            return Err(Refusal::WrongCollectionLength);
        }
        self.finish()
    }
}
