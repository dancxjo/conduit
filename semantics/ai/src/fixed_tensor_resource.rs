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
    /// Allocation identity only; equal content in another backing stays distinct.
    pub fn shares_storage_arc(&self, other: &Arc<[u8]>) -> bool {
        Arc::ptr_eq(&self.bytes, other)
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

/// Requested payload capacities owned by a full tensor descriptor. Inline root,
/// shared Arc allocation headers and allocator bookkeeping are separate charges.
/// Spare sequence slots are included; child strings are counted for live entries.
pub fn tensor_descriptor_owned_heap_bytes(tensor: &TensorValue) -> usize {
    use conduit_data::{TensorAxis, TensorAxisRole, TensorBacking};
    let mut bytes = tensor
        .axes
        .allocated_capacity()
        .saturating_mul(core::mem::size_of::<TensorAxis>())
        .saturating_add(
            tensor
                .dimensions
                .allocated_capacity()
                .saturating_mul(core::mem::size_of::<u64>()),
        );
    for axis in &tensor.axes {
        bytes = bytes.saturating_add(axis.identity.as_ref().map_or(0, |s| s.capacity()));
        if let TensorAxisRole::Other(role) = &axis.role {
            bytes = bytes.saturating_add(role.identity().capacity());
        }
    }
    match &tensor.backing {
        TensorBacking::Inline(content) => bytes.saturating_add(content.as_slice().len()),
        TensorBacking::Resource(reference) => bytes
            .saturating_add(reference.content_profile.owned_heap_bytes())
            .saturating_add(reference.access_class.owned_heap_bytes())
            .saturating_add(
                reference
                    .lifetime
                    .expires_at
                    .as_ref()
                    .map_or(0, |time| time.clock_basis.capacity()),
            ),
    }
}
#[cfg(target_has_atomic = "ptr")]
impl AdmittedFixedTensorResource {
    pub fn descriptor_owned_heap_bytes(&self) -> usize {
        tensor_descriptor_owned_heap_bytes(&self.tensor)
    }
    pub fn access_owned_heap_bytes(&self) -> usize {
        self.access
            .handle
            .owned_heap_bytes()
            .saturating_add(self.access.authority_grant.owned_heap_bytes())
    }
    pub fn shares_descriptor_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.tensor, &other.tensor)
    }
}
