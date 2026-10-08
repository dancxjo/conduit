//! Finite retained ownership for original numeric graph/model/resource material.
//! This is storage composition, not Source admission or model/tensor correlation.
//! Shared allocation headers, allocator bookkeeping and inline returned owner are
//! separate charges. Upstream acquisition/admission/preparation is never erased.
use crate::{AdmittedModelResource, fixed_tensor_resource::AdmittedFixedTensorResource};
use alloc::{sync::Arc, vec::Vec};
use conduit_core::{Plan, PlanStorageRefusal, plan_owned_heap_bytes};
use core::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericResourceStorageRefusal {
    Capacity,
    Plan(PlanStorageRefusal),
}
#[derive(Clone, Copy, Debug)]
pub struct NumericResourceStorageLimits {
    pub maximum_preparation_requested_bytes: usize,
    pub maximum_retained_payload_bytes: usize,
    pub maximum_shared_allocations: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NumericResourceStorageReceipt {
    preparation_requested_bytes_bound: usize,
    retained_payload_bytes: usize,
    shared_allocations: usize,
    unique_tensor_resources: usize,
    unique_tensor_descriptors: usize,
    unique_byte_backings: usize,
}
impl NumericResourceStorageReceipt {
    pub fn preparation_requested_bytes_bound(self) -> usize {
        self.preparation_requested_bytes_bound
    }
    pub fn retained_payload_bytes(self) -> usize {
        self.retained_payload_bytes
    }
    /// Each needs a separately admitted shared-allocation header charge.
    pub fn shared_allocations(self) -> usize {
        self.shared_allocations
    }
    pub fn unique_tensor_resources(self) -> usize {
        self.unique_tensor_resources
    }
    pub fn unique_tensor_descriptors(self) -> usize {
        self.unique_tensor_descriptors
    }
    pub fn unique_byte_backings(self) -> usize {
        self.unique_byte_backings
    }
}
/// Owns exact original Arcs. Duplicate handles remain in original order; only
/// physical payload accounting is deduplicated, by allocation identity.
pub struct PreparedNumericResourceOwners {
    model: Arc<AdmittedModelResource>,
    plan: Arc<Plan>,
    source: Arc<str>,
    tensors: Vec<Arc<AdmittedFixedTensorResource>>,
    buffers: Vec<Arc<[u8]>>,
    receipt: NumericResourceStorageReceipt,
}
fn add(total: &mut usize, bytes: usize) -> Result<(), NumericResourceStorageRefusal> {
    *total = total
        .checked_add(bytes)
        .ok_or(NumericResourceStorageRefusal::Capacity)?;
    Ok(())
}
impl PreparedNumericResourceOwners {
    /// Allocation-free reservation over complete already-owned originals.
    /// Unsupported Plan owner classes refuse rather than being charged as zero.
    pub fn storage_reservation(
        model: &Arc<AdmittedModelResource>,
        plan: &Arc<Plan>,
        source: &Arc<str>,
        tensors: &[Arc<AdmittedFixedTensorResource>],
        buffers: &[Arc<[u8]>],
    ) -> Result<NumericResourceStorageReceipt, NumericResourceStorageRefusal> {
        let array = tensors
            .len()
            .checked_mul(size_of::<Arc<AdmittedFixedTensorResource>>())
            .and_then(|n| {
                buffers
                    .len()
                    .checked_mul(size_of::<Arc<[u8]>>())
                    .and_then(|m| n.checked_add(m))
            })
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or(NumericResourceStorageRefusal::Capacity)?;
        let mut bytes = array;
        add(&mut bytes, size_of::<AdmittedModelResource>())?;
        add(&mut bytes, model.owned_payload_bytes())?;
        add(&mut bytes, size_of::<Plan>())?;
        add(
            &mut bytes,
            plan_owned_heap_bytes(plan).map_err(NumericResourceStorageRefusal::Plan)?,
        )?;
        add(&mut bytes, source.len())?;
        // Model root, model byte backing, Plan root and literal Source backing.
        let mut shared = 4usize;
        let mut resources = 0usize;
        let mut descriptors = 0usize;
        let mut backings = 1usize;
        let model_bytes = model.shared_storage();
        for (i, tensor) in tensors.iter().enumerate() {
            if tensors[..i].iter().any(|prior| Arc::ptr_eq(prior, tensor)) {
                continue;
            }
            add(&mut resources, 1)?;
            add(&mut shared, 1)?;
            add(&mut bytes, size_of::<AdmittedFixedTensorResource>())?;
            add(&mut bytes, tensor.access_owned_heap_bytes())?;
            if !tensors[..i]
                .iter()
                .any(|prior| tensor.shares_descriptor_with(prior))
            {
                add(&mut descriptors, 1)?;
                add(&mut shared, 1)?;
                add(&mut bytes, size_of::<conduit_data::TensorValue>())?;
                add(&mut bytes, tensor.descriptor_owned_heap_bytes())?;
            }
            if !tensor.shares_storage_arc(&model_bytes)
                && !tensors[..i]
                    .iter()
                    .any(|prior| tensor.shares_storage_with(prior))
            {
                add(&mut backings, 1)?;
                add(&mut shared, 1)?;
                add(&mut bytes, tensor.retained_storage_bytes())?;
            }
        }
        for (i, buffer) in buffers.iter().enumerate() {
            if model.shares_storage_with(buffer)
                || tensors
                    .iter()
                    .any(|tensor| tensor.shares_storage_arc(buffer))
                || buffers[..i].iter().any(|prior| Arc::ptr_eq(prior, buffer))
            {
                continue;
            }
            add(&mut backings, 1)?;
            add(&mut shared, 1)?;
            add(&mut bytes, buffer.len())?;
        }
        Ok(NumericResourceStorageReceipt {
            preparation_requested_bytes_bound: array,
            retained_payload_bytes: bytes,
            shared_allocations: shared,
            unique_tensor_resources: resources,
            unique_tensor_descriptors: descriptors,
            unique_byte_backings: backings,
        })
    }
    /// All capacity checks precede handle-array construction. No original owner
    /// is decoded, replaced, normalized, or dropped by this retention boundary.
    pub fn prepare(
        model: &Arc<AdmittedModelResource>,
        plan: &Arc<Plan>,
        source: &Arc<str>,
        tensors: &[Arc<AdmittedFixedTensorResource>],
        buffers: &[Arc<[u8]>],
        limits: NumericResourceStorageLimits,
    ) -> Result<Self, NumericResourceStorageRefusal> {
        let receipt = Self::storage_reservation(model, plan, source, tensors, buffers)?;
        if limits.maximum_preparation_requested_bytes < receipt.preparation_requested_bytes_bound
            || limits.maximum_retained_payload_bytes < receipt.retained_payload_bytes
            || limits.maximum_shared_allocations < receipt.shared_allocations
        {
            return Err(NumericResourceStorageRefusal::Capacity);
        }
        let mut owned_tensors = Vec::with_capacity(tensors.len());
        owned_tensors.extend(tensors.iter().map(Arc::clone));
        let mut owned_buffers = Vec::with_capacity(buffers.len());
        owned_buffers.extend(buffers.iter().map(Arc::clone));
        Ok(Self {
            model: Arc::clone(model),
            plan: Arc::clone(plan),
            source: Arc::clone(source),
            tensors: owned_tensors,
            buffers: owned_buffers,
            receipt,
        })
    }
    pub fn model(&self) -> &Arc<AdmittedModelResource> {
        &self.model
    }
    pub fn plan(&self) -> &Arc<Plan> {
        &self.plan
    }
    pub fn source(&self) -> &Arc<str> {
        &self.source
    }
    pub fn tensors(&self) -> &[Arc<AdmittedFixedTensorResource>] {
        &self.tensors
    }
    pub fn buffers(&self) -> &[Arc<[u8]>] {
        &self.buffers
    }
    pub fn receipt(&self) -> NumericResourceStorageReceipt {
        self.receipt
    }
}
