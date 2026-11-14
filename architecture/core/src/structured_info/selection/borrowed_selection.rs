//! Prepared selection over linked canonical material, without retained Type trees.
use super::*;
use crate::BorrowedStructuredValueValidator;

/// The complete existing selector is checked during preparation. Runtime first
/// validates every input leaf, including unselected fields, before borrowing a
/// projection. The caller owns the bounded output buffer; selection cannot grow it.
pub struct PreparedBorrowedStructuredSelector {
    input: BorrowedStructuredValueValidator<'static>,
    output_type: &'static [u8],
    operation: StructuredSelectorBack,
    maximum_output_bytes: usize,
}
enum Selected<'value> {
    Value(crate::ValidatedCanonicalStructuredValue<'value>),
    Unmatched(crate::UnmatchedVariantDisposition),
}
impl PreparedBorrowedStructuredSelector {
    pub fn new(
        material: &'static [u8],
        maximum_input_bytes: usize,
        maximum_output_bytes: usize,
    ) -> Result<Self, StructuredSelectorRefusal> {
        use StructuredSelectorRefusal as E;
        let checked = StructuredSelector::from_canonical_bytes(material)?;
        if checked.canonical_bytes()?.as_slice() != material {
            return Err(E::MalformedCanonicalEncoding);
        }
        let mut cursor = Cursor::new(&material[SELECTOR_DIGEST_DOMAIN.len()..]);
        let input_type = cursor.bytes().map_err(|_| E::MalformedCanonicalEncoding)?;
        let output = checked
            .output_type
            .canonical_bytes()
            .map_err(|_| E::CanonicalEncodingTooLarge)?;
        // Every selector output is either the original Type or an exact subtree.
        // Equal canonical occurrences are interchangeable immutable schemas.
        let start = input_type
            .windows(output.len())
            .position(|p| p == output)
            .ok_or(E::MalformedCanonicalEncoding)?;
        let output_type = &input_type[start..start + output.len()];
        if maximum_output_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || maximum_output_bytes <= output_type.len()
        {
            return Err(E::CanonicalEncodingTooLarge);
        }
        let input = BorrowedStructuredValueValidator::new(input_type, maximum_input_bytes)
            .map_err(|_| E::CanonicalEncodingTooLarge)?;
        Ok(Self {
            input,
            output_type,
            operation: checked.operation,
            maximum_output_bytes,
        })
    }

    pub fn input_type_bytes(&self) -> &'static [u8] {
        self.input.type_bytes()
    }
    pub fn output_type_bytes(&self) -> &'static [u8] {
        self.output_type
    }

    /// Only owned operation strings/predicate bytes; linked schemas and caller
    /// output storage are separate. No Type graph remains after preparation.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        match &self.operation {
            StructuredSelectorBack::Field(name) => Some(name.capacity()),
            StructuredSelectorBack::Index(_) => Some(0),
            StructuredSelectorBack::Variant { tag, .. } => Some(tag.capacity()),
            StructuredSelectorBack::RecordFieldValues {
                field, expected, ..
            } => {
                let bytes = expected
                    .capacity()
                    .checked_mul(core::mem::size_of::<Vec<u8>>())?
                    .checked_add(field.capacity())?;
                expected
                    .iter()
                    .try_fold(bytes, |n, item| n.checked_add(item.capacity()))
            }
        }
    }

    pub fn select_canonical_into(
        &self,
        encoded: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<StructuredCanonicalSelection, StructuredSelectorRefusal> {
        use StructuredSelectorRefusal as E;
        let value = self.input.validate(encoded).map_err(|error| match error {
            StructuredInfoRefusal::WrongType => E::WrongInputType,
            StructuredInfoRefusal::CanonicalEncodingTooLarge => E::CanonicalEncodingTooLarge,
            _ => E::MalformedCheckedValue,
        })?;
        self.select_validated_into(value, output)
    }

    /// Select from the exact linked schema and a separately retained node.
    /// The complete input is validated before selection, including leaves that
    /// the projection does not choose. This creates no admission authority.
    pub fn select_parts_into(
        &self,
        schema: &[u8],
        node: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<StructuredCanonicalSelection, StructuredSelectorRefusal> {
        use StructuredSelectorRefusal as E;
        if schema != self.input.type_bytes() {
            return Err(E::WrongInputType);
        }
        let value = self
            .input
            .validate_node(node)
            .map_err(|error| match error {
                StructuredInfoRefusal::WrongType => E::WrongInputType,
                StructuredInfoRefusal::CanonicalEncodingTooLarge => E::CanonicalEncodingTooLarge,
                _ => E::MalformedCheckedValue,
            })?;
        self.select_validated_into(value, output)
    }

    fn select_validated_into(
        &self,
        value: crate::ValidatedCanonicalStructuredValue<'_>,
        output: &mut Vec<u8>,
    ) -> Result<StructuredCanonicalSelection, StructuredSelectorRefusal> {
        let selected = match self.selected(value)? {
            Selected::Value(selected) => selected,
            Selected::Unmatched(disposition) => {
                return Ok(StructuredCanonicalSelection::Unmatched(disposition))
            }
        };
        self.checked_size(selected, output.capacity())?;
        output.clear();
        output.extend_from_slice(self.output_type);
        output.extend_from_slice(selected.value_node());
        Ok(StructuredCanonicalSelection::Matched)
    }
    fn selected<'value>(
        &self,
        value: crate::ValidatedCanonicalStructuredValue<'value>,
    ) -> Result<Selected<'value>, StructuredSelectorRefusal> {
        use StructuredSelectorRefusal as E;
        let malformed = |_| E::MalformedCheckedValue;
        let representation = value.representation().map_err(malformed)?;
        let selected = match &self.operation {
            StructuredSelectorBack::Field(field) => representation
                .record_field(field)
                .map_err(malformed)?
                .ok_or(E::MalformedCheckedValue)?,
            StructuredSelectorBack::Index(index) => representation
                .collection_index(*index)
                .map_err(malformed)?
                .ok_or(E::IndexOutOfRange)?,
            StructuredSelectorBack::Variant { tag, unmatched } => {
                let Some(payload) = representation.variant_payload(tag).map_err(malformed)? else {
                    return Ok(Selected::Unmatched(*unmatched));
                };
                payload
            }
            StructuredSelectorBack::RecordFieldValues {
                field,
                expected,
                match_when_present,
                unmatched,
            } => {
                let leaf = representation
                    .record_field(field)
                    .map_err(malformed)?
                    .ok_or(E::MalformedCheckedValue)?
                    .representation()
                    .map_err(malformed)?;
                let mut kind = Cursor::new(leaf.type_bytes());
                if kind.byte().map_err(malformed)? != 0 {
                    return Err(E::MalformedCheckedValue);
                }
                let identity = kind.text_ref().map_err(malformed)?;
                let actual = leaf.primitive_bytes(identity).map_err(malformed)?;
                if expected.iter().any(|v| v.as_slice() == actual) != *match_when_present {
                    return Ok(Selected::Unmatched(*unmatched));
                }
                value
            }
        };
        Ok(Selected::Value(selected))
    }
    fn checked_size(
        &self,
        selected: crate::ValidatedCanonicalStructuredValue<'_>,
        capacity: usize,
    ) -> Result<usize, StructuredSelectorRefusal> {
        use StructuredSelectorRefusal as E;
        let bytes = self
            .output_type
            .len()
            .checked_add(selected.value_node().len())
            .ok_or(E::CanonicalEncodingTooLarge)?;
        if bytes > self.maximum_output_bytes || bytes > capacity {
            return Err(E::CanonicalEncodingTooLarge);
        }
        if selected.type_bytes() != self.output_type {
            return Err(E::MalformedCheckedValue);
        }
        Ok(bytes)
    }
    /// Complete validation and all size/type checks precede any destination write.
    /// Unmatched dispositions preserve the destination and report zero bytes.
    pub fn select_canonical_slice_into(
        &self,
        encoded: &[u8],
        output: &mut [u8],
    ) -> Result<(StructuredCanonicalSelection, usize), StructuredSelectorRefusal> {
        let value = self
            .input
            .validate(encoded)
            .map_err(selection_input_error)?;
        self.select_validated_slice_into(value, output)
    }
    pub fn select_parts_slice_into(
        &self,
        schema: &[u8],
        node: &[u8],
        output: &mut [u8],
    ) -> Result<(StructuredCanonicalSelection, usize), StructuredSelectorRefusal> {
        if schema != self.input.type_bytes() {
            return Err(StructuredSelectorRefusal::WrongInputType);
        }
        let value = self
            .input
            .validate_node(node)
            .map_err(selection_input_error)?;
        self.select_validated_slice_into(value, output)
    }
    fn select_validated_slice_into(
        &self,
        value: crate::ValidatedCanonicalStructuredValue<'_>,
        output: &mut [u8],
    ) -> Result<(StructuredCanonicalSelection, usize), StructuredSelectorRefusal> {
        match self.selected(value)? {
            Selected::Unmatched(disposition) => {
                Ok((StructuredCanonicalSelection::Unmatched(disposition), 0))
            }
            Selected::Value(selected) => {
                let length = self.checked_size(selected, output.len())?;
                let (schema, node) = output[..length].split_at_mut(self.output_type.len());
                schema.copy_from_slice(self.output_type);
                node.copy_from_slice(selected.value_node());
                Ok((StructuredCanonicalSelection::Matched, length))
            }
        }
    }
}

fn selection_input_error(error: StructuredInfoRefusal) -> StructuredSelectorRefusal {
    match error {
        StructuredInfoRefusal::WrongType => StructuredSelectorRefusal::WrongInputType,
        StructuredInfoRefusal::CanonicalEncodingTooLarge => {
            StructuredSelectorRefusal::CanonicalEncodingTooLarge
        }
        _ => StructuredSelectorRefusal::MalformedCheckedValue,
    }
}
