//! Constructor-owned fixed terms for the closed startup recipe. This excludes
//! lowering, ledger staging, metadata/provider clones, caller captures and
//! errors, so it is not a complete preparation receipt or execution authority.
use crate::installed_std::{
    kernel_preparation::KernelTables, InstalledBack, InstalledScheduler, MAX_NODES,
};
use conduit_kernel::{HostedSignLog, HostedValueStore};
use core::mem::size_of;

pub(super) struct FixedWorkspace {
    pub(super) values: usize,
    pub(super) signs: usize,
    pub(super) retained_kernel: usize,
    pub(super) constructor_requests: usize,
}
pub(super) fn derive() -> Option<FixedWorkspace> {
    if !cfg!(target_pointer_width = "64") {
        return None;
    }
    // Remote preparation starts maximum_value_bytes at TICK_ENCODED_LEN;
    // source budget then contributes exactly one Bool slot, sink no slots.
    let maximum = crate::installed_std::TICK_ENCODED_LEN.max(conduit_core::BOOL_ENCODED_LEN as u32);
    let values = HostedValueStore::canonical_storage_bytes(core::iter::once(maximum)).ok()?;
    // prepare_with_retention default32 + body/wake8 + startup-chime16;
    // no remote endpoints and default retention contribute zero remote rows.
    let signs = HostedSignLog::canonical_storage_bytes(32 + 8 + 16, 0)?;
    let retained_kernel = size_of::<InstalledScheduler>()
        .checked_add(values)?
        .checked_add(signs)?
        .checked_add(crate::hosted_audio::startup_chime::PCM_BYTES)?
        // Same qualified Arc<AtomicU8> layout as back_capacity.rs.
        .checked_add(size_of::<[usize; 3]>())?;
    // preparation.rs first collects the MAX_NODES operation Vec, then consumes
    // it through another collect before array conversion. Count both exact-size
    // buffers even when the pinned library reuses the original allocation.
    // KernelTables and the new boxed scheduler overlap during install.
    let drivers = MAX_NODES.checked_mul(size_of::<InstalledBack>())?;
    let constructor_requests = retained_kernel
        .checked_add(size_of::<KernelTables>())?
        .checked_add(drivers)?
        .checked_add(drivers)?;
    Some(FixedWorkspace {
        values,
        signs,
        retained_kernel,
        constructor_requests,
    })
}
