//! Fixed-geometry canonical output storage admitted before model execution.
use crate::{
    tensor_content_digest, NamedTensorBundle, NamedTensorBundleView, PreparedNamedTensorBundleView,
    TensorAxis, TensorBacking, TensorBackingView, TensorElement, TensorValue,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::validate_canonical_structured_value;
use conduit_plot::rust_binding::NativeRustBinding;
use core::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedTensorOutputRefusal {
    Template,
    InvalidTensor,
    MetadataMismatch,
    UnsupportedBacking,
    UnknownPort,
    AlreadyWritten,
    PayloadLength,
    Incomplete,
    Encoding,
}
struct OutputSlot {
    name: String,
    element: TensorElement,
    dimensions: Box<[u64]>,
    axes: Box<[TensorAxis]>,
    payload: Range<usize>,
    digest: [usize; 32],
}

/// One exact set of names, dtypes, dimensions and axes. Preparation owns all
/// storage; writes can replace only admitted payload bytes and their digests.
/// Variable geometry requires a separately prepared specialization.
pub struct PreparedNamedTensorBundleOutput {
    encoded: Box<[u8]>,
    slots: Box<[OutputSlot]>,
    reader: PreparedNamedTensorBundleView,
    written: u32,
}
impl PreparedNamedTensorBundleOutput {
    pub fn prepare(template: &NamedTensorBundle) -> Result<Self, NamedTensorOutputRefusal> {
        use NamedTensorOutputRefusal as R;
        template.validate().map_err(|_| R::Template)?;
        let encoded = template.clone().encode().map_err(|_| R::Template)?;
        let reader = PreparedNamedTensorBundleView::prepare().map_err(|_| R::Template)?;
        let bundle = reader.read(&encoded).map_err(|_| R::Template)?;
        let canonical = validate_canonical_structured_value(&encoded).map_err(|_| R::Template)?;
        let offset = |bytes: &[u8]| -> Result<usize, NamedTensorOutputRefusal> {
            let start = (bytes.as_ptr() as usize)
                .checked_sub(encoded.as_ptr() as usize)
                .ok_or(R::Template)?;
            if start
                .checked_add(bytes.len())
                .is_none_or(|end| end > encoded.len())
            {
                return Err(R::Template);
            }
            Ok(start)
        };
        let mut slots = Vec::with_capacity(bundle.values().len());
        for (index, tensor) in bundle.values().enumerate() {
            let TensorBackingView::Inline(payload) = tensor.backing else {
                return Err(R::UnsupportedBacking);
            };
            let start = offset(payload)?;
            let value = canonical
                .collection_index(index as u16)
                .map_err(|_| R::Template)?
                .ok_or(R::Template)?
                .representation()
                .map_err(|_| R::Template)?
                .record_field("value")
                .map_err(|_| R::Template)?
                .ok_or(R::Template)?
                .representation()
                .map_err(|_| R::Template)?;
            let digest_view = value
                .record_field("content-digest")
                .map_err(|_| R::Template)?
                .ok_or(R::Template)?;
            let mut digest = [0; 32];
            for (index, slot) in digest.iter_mut().enumerate() {
                let octet = digest_view
                    .collection_index(index as u16)
                    .map_err(|_| R::Template)?
                    .ok_or(R::Template)?
                    .primitive_representation_bytes("value/u8")
                    .map_err(|_| R::Template)?;
                if octet.len() != 1 {
                    return Err(R::Template);
                }
                *slot = offset(octet)?;
            }
            slots.push(OutputSlot {
                name: tensor.name.into(),
                element: tensor.element,
                dimensions: tensor.dimensions().into(),
                axes: template.get()[index].value.axes.as_slice().into(),
                payload: start..start + payload.len(),
                digest,
            });
        }
        Ok(Self {
            encoded: encoded.into_boxed_slice(),
            slots: slots.into_boxed_slice(),
            reader,
            written: 0,
        })
    }
    /// Begin another invocation without exposing any prior invocation's output.
    pub fn reset(&mut self) {
        self.written = 0;
    }
    pub fn maximum_encoded_len(&self) -> usize {
        self.encoded.len()
    }
    pub fn write(&mut self, name: &str, payload: &[u8]) -> Result<(), NamedTensorOutputRefusal> {
        use NamedTensorOutputRefusal as R;
        let Some((index, slot)) = self
            .slots
            .iter()
            .enumerate()
            .find(|(_, slot)| slot.name == name)
        else {
            self.written = 0;
            return Err(R::UnknownPort);
        };
        let bit = 1_u32 << index;
        if self.written & bit != 0 {
            self.written = 0;
            return Err(R::AlreadyWritten);
        }
        if payload.len() != slot.payload.len() {
            self.written = 0;
            return Err(R::PayloadLength);
        }
        let digest = tensor_content_digest(payload);
        self.encoded[slot.payload.clone()].copy_from_slice(payload);
        for (offset, octet) in slot.digest.iter().zip(digest) {
            self.encoded[*offset] = octet;
        }
        self.written |= bit;
        Ok(())
    }
    /// Preserve a provider's complete admitted tensor descriptor. A changed
    /// dtype, dimension, axis identity, role or unit cannot be silently replaced
    /// by the prepared envelope's metadata.
    pub fn write_tensor(
        &mut self,
        name: &str,
        tensor: &TensorValue,
    ) -> Result<(), NamedTensorOutputRefusal> {
        use NamedTensorOutputRefusal as R;
        let refusal = if tensor.validate().is_err() {
            Some(R::InvalidTensor)
        } else if let Some(slot) = self.slots.iter().find(|slot| slot.name == name) {
            (slot.element != tensor.element
                || slot.dimensions.as_ref() != tensor.dimensions.as_slice()
                || slot.axes.as_ref() != tensor.axes.as_slice())
            .then_some(R::MetadataMismatch)
        } else {
            Some(R::UnknownPort)
        };
        if let Some(refusal) = refusal {
            self.written = 0;
            return Err(refusal);
        }
        let TensorBacking::Inline(bytes) = &tensor.backing else {
            self.written = 0;
            return Err(R::UnsupportedBacking);
        };
        self.write(name, bytes.as_slice())
    }

    /// Partial, reset or failed invocations cannot publish a mixed bundle.
    pub fn encoded(&self) -> Result<&[u8], NamedTensorOutputRefusal> {
        let required = if self.slots.len() == 32 {
            u32::MAX
        } else {
            (1_u32 << self.slots.len()) - 1
        };
        if self.written != required {
            return Err(NamedTensorOutputRefusal::Incomplete);
        }
        Ok(&self.encoded)
    }
    pub fn view(&self) -> Result<NamedTensorBundleView<'_>, NamedTensorOutputRefusal> {
        self.reader
            .read(self.encoded()?)
            .map_err(|_| NamedTensorOutputRefusal::Encoding)
    }
}
