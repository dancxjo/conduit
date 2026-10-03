//! Borrowed shape observations from a fully validated canonical value.
use super::{malformed, Cursor, StructuredInfoRefusal, ValidatedCanonicalStructuredValue};

impl<'a> ValidatedCanonicalStructuredValue<'a> {
    /// The active case is data, never permission to use a resource.
    pub fn variant_tag(self) -> Result<&'a str, StructuredInfoRefusal> {
        let mut value = Cursor::new(self.value_node);
        if value.byte()? != 3 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        value.text()
    }
    /// Actual finite element count, independent of a schema's maximum capacity.
    pub fn collection_length(self) -> Result<u32, StructuredInfoRefusal> {
        let mut value = Cursor::new(self.value_node);
        if value.byte()? != 1 {
            return Err(StructuredInfoRefusal::WrongType);
        }
        u32::try_from(value.length()?).map_err(|_| malformed())
    }
}
