//! Shared immutable tensor custody for hosted prepared numerical operators.
//! Adoption validates content and authority once; no dereferencing occurs in Step.
use crate::fixed_numeric_preparation::{admit_tensor_access, FixedPlannedRefusal};
use crate::fixed_tensor::FixedTensorRefusal;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use conduit_core::{AdmittedResourceAccess, ResourceReferenceBinding};
use conduit_data::{tensor_content_digest, TensorValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedResourceAdoptionRefusal {
    Tensor(FixedTensorRefusal),
    Authority(FixedPlannedRefusal),
}
#[cfg(target_has_atomic = "ptr")]
pub struct AdmittedFixedTensorResource {
    tensor: Arc<TensorValue>,
    bytes: Arc<[u8]>,
    access: AdmittedResourceAccess,
}
#[cfg(target_has_atomic = "ptr")]
impl AdmittedFixedTensorResource {
    pub fn adopt(
        tensor: Arc<TensorValue>,
        bytes: Arc<[u8]>,
        binding: &ResourceReferenceBinding,
    ) -> Result<Self, FixedResourceAdoptionRefusal> {
        tensor
            .validate()
            .map_err(|e| FixedResourceAdoptionRefusal::Tensor(FixedTensorRefusal::Tensor(e)))?;
        if tensor
            .byte_count()
            .map_err(|e| FixedResourceAdoptionRefusal::Tensor(FixedTensorRefusal::Tensor(e)))?
            != bytes.len() as u64
            || tensor.content_digest != tensor_content_digest(&bytes)
        {
            return Err(FixedResourceAdoptionRefusal::Tensor(
                FixedTensorRefusal::ContentIdentity,
            ));
        }
        let access = admit_tensor_access(&tensor, binding)
            .map_err(FixedResourceAdoptionRefusal::Authority)?;
        Ok(Self {
            tensor,
            bytes,
            access,
        })
    }
    pub fn tensor(&self) -> &TensorValue {
        &self.tensor
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn access(&self) -> &AdmittedResourceAccess {
        &self.access
    }
    /// Content plus inline descriptor storage, counted once per resource.
    /// Descriptor-owned sequences/strings and Arc headers are additional.
    pub fn content_and_inline_descriptor_bytes(&self) -> usize {
        self.bytes.len() + core::mem::size_of::<TensorValue>()
    }
}
pub(crate) enum FixedTensorView<'a> {
    Borrowed {
        tensor: &'a TensorValue,
        bytes: &'a [u8],
    },
    #[cfg(target_has_atomic = "ptr")]
    Owned(Arc<AdmittedFixedTensorResource>),
}
impl<'a> FixedTensorView<'a> {
    pub(crate) fn borrowed(tensor: &'a TensorValue, bytes: &'a [u8]) -> Self {
        Self::Borrowed { tensor, bytes }
    }
    pub(crate) fn tensor(&self) -> &TensorValue {
        match self {
            Self::Borrowed { tensor, .. } => tensor,
            #[cfg(target_has_atomic = "ptr")]
            Self::Owned(resource) => resource.tensor(),
        }
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Self::Borrowed { bytes, .. } => bytes,
            #[cfg(target_has_atomic = "ptr")]
            Self::Owned(resource) => resource.bytes(),
        }
    }
}
