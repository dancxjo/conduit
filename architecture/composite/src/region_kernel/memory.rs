//! Managed backing capacities admitted against the exact worker binding.
use super::*;

/// Allocator bookkeeping and OS thread/guard/TLS overhead remain Host-profile
/// costs. The stack field is the Plan's reservation; no thread is started here.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct RegionMemoryReceipt {
    pub kernel_backing_bytes: usize,
    pub identity_backing_bytes: usize,
    pub scratch_bytes: usize,
    pub stack_reservation_bytes: usize,
    pub total_admitted_bytes: usize,
}
pub(super) fn admit(instance: &PreparedRegionKernel) -> Result<RegionMemoryReceipt, String> {
    let kernel_backing_bytes = instance
        .kernel
        .retained_worker_bytes()
        .ok_or("worker kernel lacks a complete managed memory receipt")?;
    let mut identity_backing_bytes = core::mem::size_of::<PreparedRegionKernel>()
        .checked_sub(core::mem::size_of::<ChildKernel>())
        .ok_or("worker metadata size underflow")?;
    let worker = &instance.worker_storage;
    for bytes in [
        instance.plan_id.0.capacity(),
        instance.fragment_id.0.capacity(),
        instance.region_id.0.capacity(),
        worker.binding.pool_id.0.capacity(),
        worker.binding.class_id.0.capacity(),
        instance
            .original_nodes
            .capacity()
            .checked_mul(core::mem::size_of::<NodeId>())
            .ok_or("worker node map overflow")?,
        instance
            .original_cords
            .capacity()
            .checked_mul(core::mem::size_of::<CordId>())
            .ok_or("worker Cord map overflow")?,
    ] {
        identity_backing_bytes = identity_backing_bytes
            .checked_add(bytes)
            .ok_or("worker metadata overflow")?;
    }
    let stack_reservation_bytes =
        usize::try_from(worker.stack_bytes).map_err(|_| "worker stack overflow")?;
    let scratch_bytes = instance.scratch.len();
    let total_admitted_bytes = kernel_backing_bytes
        .checked_add(identity_backing_bytes)
        .and_then(|bytes| bytes.checked_add(scratch_bytes))
        .and_then(|bytes| bytes.checked_add(stack_reservation_bytes))
        .ok_or("worker memory overflow")?;
    let capacity = usize::try_from(worker.binding.units).map_err(|_| "worker capacity overflow")?;
    if total_admitted_bytes > capacity {
        return Err("worker memory binding cannot hold its complete managed backing".into());
    }
    Ok(RegionMemoryReceipt {
        kernel_backing_bytes,
        identity_backing_bytes,
        scratch_bytes,
        stack_reservation_bytes,
        total_admitted_bytes,
    })
}
