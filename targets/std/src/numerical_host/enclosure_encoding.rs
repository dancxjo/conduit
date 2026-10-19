//! Prepared record encoding retains the caller's exact symbolic source bytes.
use conduit_core::*;
use conduit_semantic_catalog::{mathematical_enclosure_type, MATHEMATICAL_ENCLOSE_MAX_BYTES};
pub struct EnclosureEncoding {
    source: PreparedStructuredComposer,
    lower: PreparedStructuredComposer,
    upper: PreparedStructuredComposer,
    width: PreparedStructuredComposer,
    maximum_width: PreparedStructuredComposer,
    result: PreparedStructuredComposer,
}
impl EnclosureEncoding {
    pub fn new() -> Result<Self, StructuredInfoRefusal> {
        let ty = mathematical_enclosure_type();
        let math = StructuredInfoType::leaf(kind_id(MATHEMATICAL_VALUE_ID))?;
        let leaf = |bytes| {
            PreparedStructuredComposer::new(&math, math.canonical_bytes()?.len() + 5 + bytes)
        };
        Ok(Self {
            source: leaf(MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)?,
            lower: leaf(2 + EXACT_RATIONAL_MAX_ENCODED_BYTES)?,
            upper: leaf(2 + EXACT_RATIONAL_MAX_ENCODED_BYTES)?,
            width: leaf(2 + EXACT_RATIONAL_MAX_ENCODED_BYTES)?,
            maximum_width: leaf(2 + EXACT_RATIONAL_MAX_ENCODED_BYTES)?,
            result: PreparedStructuredComposer::new(&ty, MATHEMATICAL_ENCLOSE_MAX_BYTES as usize)?,
        })
    }
    pub fn prepared_heap_bytes(&self) -> usize {
        [
            &self.source,
            &self.lower,
            &self.upper,
            &self.width,
            &self.maximum_width,
            &self.result,
        ]
        .iter()
        .map(|v| v.prepared_heap_bytes())
        .sum()
    }
    pub fn encoded(&self) -> &[u8] {
        self.result.encoded()
    }
    pub fn encode(
        &mut self,
        source: &[u8],
        result: &BorrowedMathematicalEnclosure<'_>,
    ) -> Result<(), StructuredInfoRefusal> {
        let mut lower = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let mut upper = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let mut width = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let mut maximum_width = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let write = |value, out: &mut [u8]| {
            MathematicalValue::rational(value)
                .encode(out)
                .map_err(|_| StructuredInfoRefusal::WrongType)
        };
        let lo = write(result.target().lower(), &mut lower)?;
        let hi = write(result.target().upper(), &mut upper)?;
        let n = write(result.width(), &mut width)?;
        let m = write(result.requirements().maximum_width, &mut maximum_width)?;
        let source = self.source.leaf(source)?;
        let lower = self.lower.leaf(&lower[..lo])?;
        let upper = self.upper.leaf(&upper[..hi])?;
        let width = self.width.leaf(&width[..n])?;
        let maximum_width = self.maximum_width.leaf(&maximum_width[..m])?;
        self.result.record(&[
            validate_canonical_structured_value(lower)?,
            validate_canonical_structured_value(maximum_width)?,
            validate_canonical_structured_value(source)?,
            validate_canonical_structured_value(upper)?,
            validate_canonical_structured_value(width)?,
        ])?;
        Ok(())
    }
}
