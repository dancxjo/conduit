//! Shared immutable tensor custody for hosted prepared numerical operators.
//! Adoption validates content and authority once; no dereferencing occurs in Step.
use crate::fixed_numeric_preparation::{FixedPlannedRefusal, admit_tensor_access};
use crate::fixed_tensor::FixedTensorRefusal;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use conduit_core::{AdmittedResourceAccess, ResourceReferenceBinding};
use conduit_data::{TensorValue, tensor_content_digest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedResourceAdoptionRefusal {
    Tensor(FixedTensorRefusal),
    Authority(FixedPlannedRefusal),
    ContentRange,
}
#[cfg(target_has_atomic = "ptr")]
pub struct AdmittedFixedTensorResource {
    tensor: Arc<TensorValue>,
    bytes: Arc<[u8]>,
    range: core::ops::Range<usize>,
    access: AdmittedResourceAccess,
}
#[cfg(target_has_atomic = "ptr")]
impl AdmittedFixedTensorResource {
    pub fn adopt(
        tensor: Arc<TensorValue>,
        bytes: Arc<[u8]>,
        binding: &ResourceReferenceBinding,
    ) -> Result<Self, FixedResourceAdoptionRefusal> {
        let length = bytes.len();
        Self::adopt_shared_slice(tensor, bytes, 0..length, binding)
    }
    /// Retain a bounded immutable slice of shared storage, without copying it.
    /// The slice must have its own exact tensor descriptor and read grant.
    /// Parent-model admission and slice-to-model correlation belong to the caller.
    pub fn adopt_shared_slice(
        tensor: Arc<TensorValue>,
        bytes: Arc<[u8]>,
        range: core::ops::Range<usize>,
        binding: &ResourceReferenceBinding,
    ) -> Result<Self, FixedResourceAdoptionRefusal> {
        let content = bytes
            .get(range.clone())
            .ok_or(FixedResourceAdoptionRefusal::ContentRange)?;
        tensor
            .validate()
            .map_err(|e| FixedResourceAdoptionRefusal::Tensor(FixedTensorRefusal::Tensor(e)))?;
        if tensor
            .byte_count()
            .map_err(|e| FixedResourceAdoptionRefusal::Tensor(FixedTensorRefusal::Tensor(e)))?
            != content.len() as u64
            || tensor.content_digest != tensor_content_digest(content)
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
            range,
            access,
        })
    }
    pub fn tensor(&self) -> &TensorValue {
        &self.tensor
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[self.range.clone()]
    }
    /// Actual retained backing allocation, counted once across shared slices.
    /// Descriptor metadata and Arc headers are additional.
    pub fn retained_storage_bytes(&self) -> usize {
        self.bytes.len()
    }
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.bytes, &other.bytes)
    }

    pub fn access(&self) -> &AdmittedResourceAccess {
        &self.access
    }
    /// Logical slice content plus inline descriptor, counted once per resource.
    /// Physical retained storage can be shared or larger; measure it separately.
    /// Descriptor-owned sequences/strings and Arc headers are additional.
    pub fn content_and_inline_descriptor_bytes(&self) -> usize {
        self.bytes().len() + core::mem::size_of::<TensorValue>()
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
