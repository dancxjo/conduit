//! Bounded sequence framing from exact ready generated descriptor metadata.
//! This supplies representation only; complete Native admission remains required.
use crate::parser_canonical_schema::{select_field, shape, Shape};
use alloc::vec::Vec;
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SequenceRefusal {
    Descriptor,
    Type,
    Length,
    Pressure,
    Frame,
}
pub(crate) struct PreparedLexicalCanonicalSequence {
    encoded: Vec<u8>,
    value_type: &'static [u8],
    element_type: &'static [u8],
    minimum: usize,
    maximum: usize,
    maximum_encoded: usize,
}
impl PreparedLexicalCanonicalSequence {
    pub(crate) fn prepare<T: PreparedNativeRustBinding>(
        family: &PreparedNativeFamily,
        path: &[&str],
        maximum_encoded: usize,
        maximum_preparation: usize,
        maximum_retained: usize,
    ) -> Result<Self, SequenceRefusal> {
        use SequenceRefusal as R;
        if !family.contains_descriptor(T::PREPARED_DESCRIPTOR) {
            return Err(R::Descriptor);
        }
        let value_type =
            select_field(T::PREPARED_DESCRIPTOR.type_bytes, path).map_err(|_| R::Type)?;
        if !matches!(shape(value_type).map_err(|_| R::Type)?, Shape::Sequence)
            || value_type.len() < 6
        {
            return Err(R::Type);
        }
        // shape() already checked the complete bounded Type and its child extent.
        let minimum = usize::from(u16::from_le_bytes(
            value_type[1..3].try_into().map_err(|_| R::Type)?,
        ));
        let maximum = usize::from(u16::from_le_bytes(
            value_type[3..5].try_into().map_err(|_| R::Type)?,
        ));
        if minimum > maximum || maximum > 128 {
            return Err(R::Length);
        }
        let element_type = &value_type[5..];
        if maximum_encoded < value_type.len().checked_add(5).ok_or(R::Pressure)?
            || maximum_encoded > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || maximum_encoded > maximum_preparation
            || maximum_encoded > maximum_retained
        {
            return Err(R::Pressure);
        }
        Ok(Self {
            encoded: Vec::with_capacity(maximum_encoded),
            value_type,
            element_type,
            minimum,
            maximum,
            maximum_encoded,
        })
    }
    /// Two borrowed passes validate all child schemas and aggregate frame size
    /// before replacing the output. Runtime allocates no storage.
    pub(crate) fn compose<'a, I>(&mut self, values: I) -> Result<&[u8], SequenceRefusal>
    where
        I: Clone + ExactSizeIterator<Item = ValidatedCanonicalStructuredValue<'a>>,
    {
        use SequenceRefusal as R;
        let count = values.len();
        if count < self.minimum || count > self.maximum {
            return Err(R::Length);
        }
        let mut total = self.value_type.len().checked_add(5).ok_or(R::Pressure)?;
        for value in values.clone() {
            if value.type_bytes() != self.element_type {
                return Err(R::Type);
            }
            total = total
                .checked_add(value.value_node().len())
                .ok_or(R::Pressure)?;
            if total > self.maximum_encoded {
                return Err(R::Pressure);
            }
        }
        self.encoded.clear();
        self.encoded.extend_from_slice(self.value_type);
        self.encoded.push(1);
        self.encoded
            .extend_from_slice(&(count as u32).to_le_bytes());
        let mut written = 0usize;
        for value in values {
            written = written.checked_add(1).ok_or(R::Pressure)?;
            if written > count || value.type_bytes() != self.element_type {
                return Err(R::Type);
            }
            if self
                .encoded
                .len()
                .checked_add(value.value_node().len())
                .ok_or(R::Pressure)?
                > self.maximum_encoded
            {
                return Err(R::Pressure);
            }
            self.encoded.extend_from_slice(value.value_node());
        }
        if written != count {
            return Err(R::Length);
        }
        validate_canonical_structured_value(&self.encoded).map_err(|_| R::Frame)?;
        Ok(&self.encoded)
    }
}
