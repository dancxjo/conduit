//! Canonical finite named tensor values, shared by model and other tensor users.
use crate::{NamedTensor, NamedTensorBundle, TensorRefusal};
use alloc::vec::Vec;
use conduit_core::semantic_digest;
use conduit_plot::rust_binding::BoundedSequence;

pub const MAXIMUM_NAMED_TENSORS: usize = 32;
pub const NAMED_TENSOR_BUNDLE_INFO_ID: &str = "data/named-tensor-bundle@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamedTensorBundleRefusal {
    Count,
    InvalidName,
    DuplicateName,
    InvalidEncoding,
    Tensor {
        index: usize,
        refusal: TensorRefusal,
    },
    ByteOverflow,
}

impl NamedTensorBundle {
    /// Preparation constructor. Tensor bytes and resource descriptors retain
    /// their existing validation rules; this does not grant resource access.
    pub fn from_values(values: Vec<NamedTensor>) -> Result<Self, NamedTensorBundleRefusal> {
        let bounded =
            BoundedSequence::try_from_iter(values).map_err(|_| NamedTensorBundleRefusal::Count)?;
        let bundle = Self::new(bounded).map_err(|_| NamedTensorBundleRefusal::InvalidEncoding)?;
        bundle.validate()?;
        Ok(bundle)
    }

    pub fn validate(&self) -> Result<(), NamedTensorBundleRefusal> {
        let values = self.get().as_slice();
        for (index, named) in values.iter().enumerate() {
            if named.name.is_empty() || named.name.len() > 128 || named.name.as_bytes().contains(&0)
            {
                return Err(NamedTensorBundleRefusal::InvalidName);
            }
            if values[..index].iter().any(|prior| prior.name == named.name) {
                return Err(NamedTensorBundleRefusal::DuplicateName);
            }
            named
                .value
                .validate()
                .map_err(|refusal| NamedTensorBundleRefusal::Tensor { index, refusal })?;
        }
        Ok(())
    }

    pub fn tensor_bytes(&self) -> Result<u64, NamedTensorBundleRefusal> {
        self.validate()?;
        self.get()
            .iter()
            .enumerate()
            .try_fold(0_u64, |total, (index, named)| {
                let bytes = named
                    .value
                    .byte_count()
                    .map_err(|refusal| NamedTensorBundleRefusal::Tensor { index, refusal })?;
                total
                    .checked_add(bytes)
                    .ok_or(NamedTensorBundleRefusal::ByteOverflow)
            })
    }

    /// Canonical Native encoding retains names, order, full tensor descriptors
    /// and payload custody. It is distinct from any individual content digest.
    pub fn semantic_digest(&self) -> Result<[u8; 32], NamedTensorBundleRefusal> {
        use conduit_plot::rust_binding::NativeRustBinding;
        self.validate()?;
        let value = self
            .clone()
            .into_structured()
            .map_err(|_| NamedTensorBundleRefusal::InvalidEncoding)?;
        let bytes = value
            .canonical_bytes()
            .map_err(|_| NamedTensorBundleRefusal::InvalidEncoding)?;
        Ok(semantic_digest(NAMED_TENSOR_BUNDLE_INFO_ID, &bytes))
    }
}
