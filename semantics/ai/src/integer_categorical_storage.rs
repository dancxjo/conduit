//! Complete requested storage for categorical model custody. No canonical
//! digest or semantic sequence maximum stands in for the retained originals.
use super::*;
use core::mem::{align_of, size_of, size_of_val};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CategoricalStorageRefusal {
    Overflow,
    Layout,
    Pressure,
    Preparation,
}
#[derive(Clone, Copy, Debug)]
pub struct CategoricalStepStorageLimits {
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct CategoricalStepStorageReservation {
    pub original_resource_retained_bytes: usize,
    pub retained_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct CategoricalStepStorageReceipt {
    pub original_resource_retained_bytes: usize,
    pub weights_retained_bytes: usize,
    pub type_retained_bytes: usize,
    pub execution_metadata_retained_bytes: usize,
    /// Includes complete original resource bytes and metadata and conservative
    /// Arc headers/alignment. Allocator bookkeeping is not requested storage.
    pub retained_heap_bytes_bound: usize,
}
fn add(a: usize, b: usize) -> Result<usize, CategoricalStorageRefusal> {
    a.checked_add(b).ok_or(CategoricalStorageRefusal::Overflow)
}
fn mul(a: usize, b: usize) -> Result<usize, CategoricalStorageRefusal> {
    a.checked_mul(b).ok_or(CategoricalStorageRefusal::Overflow)
}
fn arc_storage<T>(payload: usize) -> Result<usize, CategoricalStorageRefusal> {
    add(add(payload, 2 * size_of::<usize>())?, 4 * align_of::<T>())
}
fn sequence<T, const N: usize>(
    v: &conduit_plot::rust_binding::BoundedSequence<T, N>,
) -> Result<usize, CategoricalStorageRefusal> {
    mul(v.allocated_capacity(), size_of::<T>())
}
fn tensor(v: &crate::ModelTensorConstraint) -> Result<usize, CategoricalStorageRefusal> {
    let mut bytes = add(sequence(v.elements().get())?, sequence(v.axes().get())?)?;
    for axis in v.axes().get() {
        match axis.role() {
            conduit_data::TensorAxisRole::Other(v) => {
                // These checked generated variants currently store their payload
                // inline. A future boxed layout must receive a new storage proof.
                if size_of_val(v) != size_of::<conduit_data::TensorAxisRoleOther>() {
                    return Err(CategoricalStorageRefusal::Layout);
                }
                bytes = add(bytes, v.identity().capacity())?;
            }
            conduit_data::TensorAxisRole::Batch
            | conduit_data::TensorAxisRole::Time
            | conduit_data::TensorAxisRole::Feature
            | conduit_data::TensorAxisRole::Sensor
            | conduit_data::TensorAxisRole::SpatialCoordinate
            | conduit_data::TensorAxisRole::Frequency
            | conduit_data::TensorAxisRole::Channel => {}
        }
        match axis.dimension() {
            crate::ModelDimensionConstraint::Fixed(v) => {
                if size_of_val(v) != size_of::<crate::ModelDimensionConstraintFixed>() {
                    return Err(CategoricalStorageRefusal::Layout);
                }
            }
            crate::ModelDimensionConstraint::Bounded(v) => {
                if size_of_val(v) != size_of::<crate::ModelDimensionConstraintBounded>() {
                    return Err(CategoricalStorageRefusal::Layout);
                }
            }
        }
    }
    Ok(bytes)
}
fn signature(v: &crate::ModelSignature) -> Result<usize, CategoricalStorageRefusal> {
    let mut bytes = add(v.identity().capacity(), sequence(v.operations().get())?)?;
    bytes = add(bytes, sequence(v.inputs().get())?)?;
    bytes = add(bytes, sequence(v.outputs().get())?)?;
    for port in v.inputs().get().iter().chain(v.outputs().get()) {
        bytes = add(
            add(bytes, port.identity().get().capacity())?,
            port.semantic_kind().get().capacity(),
        )?;
        macro_rules! payload {
            ($v:expr, $ty:ty) => {{
                if size_of_val($v) != size_of::<$ty>() {
                    return Err(CategoricalStorageRefusal::Layout);
                }
                tensor($v.constraint())?
            }};
        }
        let children = match port.value() {
            crate::ModelValueConstraint::Tensor(v) => {
                payload!(v, crate::ModelValueConstraintTensor)
            }
            crate::ModelValueConstraint::SampledSignal(v) => {
                payload!(v, crate::ModelValueConstraintSampledSignal)
            }
            crate::ModelValueConstraint::ProbabilisticTensor(v) => {
                payload!(v, crate::ModelValueConstraintProbabilisticTensor)
            }
            crate::ModelValueConstraint::ProbabilisticSignal(v) => {
                payload!(v, crate::ModelValueConstraintProbabilisticSignal)
            }
        };
        bytes = add(bytes, children)?;
    }
    Ok(bytes)
}
fn reference(v: &BoundedResourceRef) -> Result<usize, CategoricalStorageRefusal> {
    add(
        add(v.content_profile.0.capacity(), v.access_class.0.capacity())?,
        v.lifetime
            .expires_at
            .as_ref()
            .map_or(0, |v| v.clock_basis.capacity()),
    )
}
fn content(v: &ResourceContentOffer) -> Result<usize, CategoricalStorageRefusal> {
    [
        v.contract.content_profile.0.capacity(),
        v.owner_host.0.capacity(),
        v.owner_boot.0.capacity(),
        v.base_id.0.capacity(),
        v.residence_profile.0.capacity(),
    ]
    .into_iter()
    .try_fold(0, add)
}
fn offer(v: &ResourceOffer) -> Result<usize, CategoricalStorageRefusal> {
    let mut bytes = add(v.pool_id.0.capacity(), v.class_id.0.capacity())?;
    if let Some(v) = &v.content {
        bytes = add(bytes, content(v)?)?;
    }
    if let Some(v) = &v.compute {
        bytes = add(bytes, v.architecture_base_id.0.capacity())?;
        bytes = add(
            bytes,
            mul(
                v.topology_groups.capacity(),
                size_of::<ComputeTopologyGroup>(),
            )?,
        )?;
        for group in &v.topology_groups {
            bytes = add(bytes, group.group_id.0.capacity())?;
            for n in [
                group.numa_domain.as_ref().map_or(0, |v| v.0.capacity()),
                group.cache_domain.as_ref().map_or(0, |v| v.0.capacity()),
                group
                    .performance_class
                    .as_ref()
                    .map_or(0, |v| v.0.capacity()),
            ] {
                bytes = add(bytes, n)?;
            }
        }
    }
    Ok(bytes)
}
/// Allocation-free storage inspection; shared allocations are charged in full.
/// The result includes every original signature field, vector spare capacity,
/// artifact/reference metadata, adopted access grant, and immutable model bytes.
pub fn categorical_resource_retained_bytes(
    v: &AdmittedModelResource,
) -> Result<usize, CategoricalStorageRefusal> {
    let a = v.artifact();
    let mut bytes = arc_storage::<AdmittedModelResource>(size_of::<AdmittedModelResource>())?;
    for n in [
        a.architecture_profile.capacity(),
        a.format_profile.capacity(),
        a.precision_profile.capacity(),
        reference(&a.content)?,
        signature(v.signature())?,
        v.access().handle.0.capacity(),
        v.access().authority_grant.0.capacity(),
        arc_storage::<usize>(v.bytes().len())?,
    ] {
        bytes = add(bytes, n)?;
    }
    Ok(bytes)
}
impl PreparedCategoricalStep {
    /// Reserves the original owners plus all construction work before any new
    /// allocation. The primitive codecs contribute an 8 MiB finite allowance;
    /// 64 times incoming storage covers geometric canonical/digest writers,
    /// duplicate signature collectors, residence/contract clones, exact weights,
    /// both Type owners, identity writers and their transient copies. This is a
    /// conservative requested-storage bound, not a small-memory claim.
    pub fn storage_reservation(
        resource: &AdmittedModelResource,
        pool: &ResourcePoolId,
        residence: &ResourceContentOffer,
    ) -> Result<CategoricalStepStorageReservation, CategoricalStorageRefusal> {
        if size_of::<usize>() > 8
            || size_of::<StructuredInfoValue>() > 256
            || size_of::<StructuredInfoType>() > 128
        {
            return Err(CategoricalStorageRefusal::Layout);
        }
        let original = categorical_resource_retained_bytes(resource)?;
        let incoming = add(add(original, pool.0.capacity())?, content(residence)?)?;
        let future = add(8 * 1024 * 1024, mul(incoming, 64)?)?;
        let total = add(
            add(incoming, future)?,
            arc_storage::<Self>(size_of::<Self>())?,
        )?;
        Ok(CategoricalStepStorageReservation {
            original_resource_retained_bytes: original,
            retained_heap_bytes_bound: total,
            preparation_peak_heap_bytes_bound: total,
        })
    }
    pub fn prepare_bounded(
        resource: Arc<AdmittedModelResource>,
        pool: ResourcePoolId,
        residence: ResourceContentOffer,
        limits: CategoricalStepStorageLimits,
    ) -> Result<Self, CategoricalStorageRefusal> {
        let reservation = Self::storage_reservation(&resource, &pool, &residence)?;
        if reservation.retained_heap_bytes_bound > limits.maximum_retained_bytes
            || reservation.preparation_peak_heap_bytes_bound > limits.maximum_preparation_peak_bytes
        {
            return Err(CategoricalStorageRefusal::Pressure);
        }
        let owner = Self::prepare(resource, pool, residence)
            .map_err(|_| CategoricalStorageRefusal::Preparation)?;
        if owner.storage_receipt()?.retained_heap_bytes_bound
            > reservation.retained_heap_bytes_bound
        {
            return Err(CategoricalStorageRefusal::Pressure);
        }
        Ok(owner)
    }
    /// Counts the actual capacities of every complete original retained owner.
    /// No allocation, serialization, cached identifier or schema rebuild occurs.
    pub fn storage_receipt(
        &self,
    ) -> Result<CategoricalStepStorageReceipt, CategoricalStorageRefusal> {
        let original = categorical_resource_retained_bytes(&self.resource)?;
        let weights = self
            .model
            .retained_weight_bytes()
            .ok_or(CategoricalStorageRefusal::Overflow)?;
        let types = add(
            self.indices.owned_heap_bytes(),
            self.scores.owned_heap_bytes(),
        )?;
        let mut metadata = add(self.identity.capacity(), self.adoption_identity.capacity())?;
        metadata = add(metadata, self.pool.0.capacity())?;
        metadata = add(metadata, offer(&self.resource_offer)?)?;
        metadata = add(metadata, content(&self.bound_content)?)?;
        metadata = add(metadata, arc_storage::<Self>(size_of::<Self>())?)?;
        let retained = add(add(add(original, weights)?, types)?, metadata)?;
        Ok(CategoricalStepStorageReceipt {
            original_resource_retained_bytes: original,
            weights_retained_bytes: weights,
            type_retained_bytes: types,
            execution_metadata_retained_bytes: metadata,
            retained_heap_bytes_bound: retained,
        })
    }
}
