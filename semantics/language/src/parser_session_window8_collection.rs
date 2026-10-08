//! Fixed collection framing from exact ready generated descriptor metadata.
//! This supplies representation only; complete Native admission remains required.
use crate::parser_canonical_schema::{Shape, select_field, shape};
use alloc::vec::Vec;
use conduit_core::{ValidatedCanonicalStructuredValue, validate_canonical_structured_value};
use conduit_plot::rust_binding::{
    NativeFamilyTypeDescriptor, PreparedNativeFamily, PreparedNativeRustBinding,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CollectionRefusal {
    Descriptor,
    Type,
    Length,
    Pressure,
    Frame,
}
pub(crate) struct PreparedWindow8CanonicalCollection {
    encoded: Vec<u8>,
    value_type: &'static [u8],
    element_type: &'static [u8],
    minimum: usize,
    maximum: usize,
    maximum_encoded: usize,
}
impl PreparedWindow8CanonicalCollection {
    pub(crate) fn prepare<T: PreparedNativeRustBinding>(
        family: &PreparedNativeFamily,
        path: &[&str],
        maximum_encoded: usize,
        maximum_preparation: usize,
        maximum_retained: usize,
    ) -> Result<Self, CollectionRefusal> {
        Self::prepare_descriptor(
            family,
            T::PREPARED_DESCRIPTOR,
            path,
            maximum_encoded,
            maximum_preparation,
            maximum_retained,
        )
    }
    pub(crate) fn prepare_descriptor(
        family: &PreparedNativeFamily,
        descriptor: &'static NativeFamilyTypeDescriptor,
        path: &[&str],
        maximum_encoded: usize,
        maximum_preparation: usize,
        maximum_retained: usize,
    ) -> Result<Self, CollectionRefusal> {
        use CollectionRefusal as R;
        if !family.contains_descriptor(descriptor) {
            return Err(R::Descriptor);
        }
        let value_type = select_field(descriptor.type_bytes, path).map_err(|_| R::Type)?;
        let (element_type, length) = match shape(value_type).map_err(|_| R::Type)? {
            Shape::Collection { element, length } => (element, usize::from(length)),
            _ => return Err(R::Type),
        };
        if length > 128 {
            return Err(R::Length);
        }
        let minimum = length;
        let maximum = length;
        if maximum_encoded < value_type.len().checked_add(5).ok_or(R::Pressure)?
            || maximum_encoded > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || maximum_encoded > maximum_preparation
            || maximum_encoded > maximum_retained
        {
            return Err(R::Pressure);
        }
        Ok(Self {
            encoded: {
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(maximum_encoded)
                    .map_err(|_| R::Pressure)?;
                if bytes.capacity() != maximum_encoded {
                    return Err(R::Pressure);
                }
                bytes
            },
            value_type,
            element_type,
            minimum,
            maximum,
            maximum_encoded,
        })
    }
    /// Two borrowed passes validate all child schemas and aggregate frame size
    /// before replacing the output. Runtime allocates no storage.
    pub(crate) fn compose<'a, I>(&mut self, values: I) -> Result<&[u8], CollectionRefusal>
    where
        I: Clone + ExactSizeIterator<Item = ValidatedCanonicalStructuredValue<'a>>,
    {
        use CollectionRefusal as R;
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
