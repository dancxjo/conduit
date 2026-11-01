//! Lower a checked selector once; retain canonical Type bytes and its operation,
//! without retaining decoded Type trees or a second prepared validator tree.
use super::{StructuredSelector, StructuredSelectorBack, StructuredSelectorRefusal as R};
use crate::{validate_canonical_structured_value, PreparedStructuredValueValidator};
use alloc::{borrow::Cow, vec::Vec};

pub struct PreparedCanonicalStructuredSelector {
    input_type: Cow<'static, [u8]>,
    output_type: Cow<'static, [u8]>,
    operation: StructuredSelectorBack,
    maximum_input_bytes: usize,
    output: Vec<u8>,
}
impl PreparedCanonicalStructuredSelector {
    /// Construction admits the same checked Type/validator limits as ordinary
    /// preparation. Decoded schemas are discarded after lowering. Evaluation
    /// fully validates the canonical input and checks its exact semantic Type.
    pub fn prepare(
        selector: &StructuredSelector,
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, crate::StructuredInfoRefusal> {
        drop(PreparedStructuredValueValidator::new(
            selector.input_type(),
            maximum_input_bytes,
        )?);
        Ok(Self {
            input_type: Cow::Owned(selector.input_type().canonical_bytes()?),
            output_type: Cow::Owned(selector.output_type().canonical_bytes()?),
            operation: selector.operation.clone(),
            maximum_input_bytes,
            output: Vec::with_capacity(maximum_output_bytes),
        })
    }
    /// Borrow exact immutable canonical schemas; operation and output custody
    /// remain independent per selector. This grants no execution authority.
    pub fn with_static_schema(
        selector: &StructuredSelector,
        input_schema: &'static [u8],
        output_schema: &'static [u8],
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, crate::StructuredInfoRefusal> {
        drop(PreparedStructuredValueValidator::with_static_schema(
            selector.input_type(),
            input_schema,
            maximum_input_bytes,
        )?);
        if selector.output_type().canonical_bytes()? != output_schema {
            return Err(crate::StructuredInfoRefusal::WrongType);
        }
        Ok(Self {
            input_type: Cow::Borrowed(input_schema),
            output_type: Cow::Borrowed(output_schema),
            operation: selector.operation.clone(),
            maximum_input_bytes,
            output: Vec::with_capacity(maximum_output_bytes),
        })
    }
    pub fn select(&mut self, input: &[u8]) -> Result<Option<&[u8]>, R> {
        if input.len() > self.maximum_input_bytes {
            return Err(R::MalformedCheckedValue);
        }
        let original = validate_canonical_structured_value(input).map_err(malformed)?;
        if original.type_bytes() != self.input_type.as_ref() {
            return Err(R::MalformedCheckedValue);
        }
        let representation = original.representation().map_err(malformed)?;
        let selected = match &self.operation {
            StructuredSelectorBack::Field(name) => representation
                .record_field(name)
                .map_err(malformed)?
                .ok_or(R::MalformedCheckedValue)?,
            StructuredSelectorBack::Index(index) => representation
                .collection_index(*index)
                .map_err(malformed)?
                .ok_or(R::IndexOutOfRange)?,
            StructuredSelectorBack::Variant { tag, unmatched } => {
                match representation.variant_payload(tag).map_err(malformed)? {
                    Some(value) => value,
                    None => return unmatched_value(*unmatched),
                }
            }
            StructuredSelectorBack::RecordFieldValues {
                field,
                expected,
                match_when_present,
                unmatched,
            } => {
                let field = representation
                    .record_field(field)
                    .map_err(malformed)?
                    .ok_or(R::MalformedCheckedValue)?
                    .representation()
                    .map_err(malformed)?;
                let mut cursor = crate::structured_info::canonical::Cursor::new(field.value_node());
                if cursor.byte().map_err(malformed)? != 0 {
                    return Err(R::MalformedCheckedValue);
                }
                let actual = cursor.bytes().map_err(malformed)?;
                if !cursor.remaining.is_empty() {
                    return Err(R::MalformedCheckedValue);
                }
                if expected
                    .iter()
                    .any(|candidate| candidate.as_slice() == actual)
                    != *match_when_present
                {
                    return unmatched_value(*unmatched);
                }
                original
            }
        };
        if selected.type_bytes() != self.output_type.as_ref() {
            return Err(R::MalformedCheckedValue);
        }
        if self
            .output_type
            .len()
            .saturating_add(selected.value_node().len())
            > self.output.capacity()
        {
            return Err(R::CanonicalEncodingTooLarge);
        }
        self.output.clear();
        self.output.extend_from_slice(&self.output_type);
        self.output.extend_from_slice(selected.value_node());
        Ok(Some(&self.output))
    }
}
fn malformed(_: crate::StructuredInfoRefusal) -> R {
    R::MalformedCheckedValue
}
fn unmatched_value(
    disposition: crate::UnmatchedVariantDisposition,
) -> Result<Option<&'static [u8]>, R> {
    match disposition {
        crate::UnmatchedVariantDisposition::Drop => Ok(None),
        crate::UnmatchedVariantDisposition::Refuse => Err(R::UnmatchedVariant),
    }
}
