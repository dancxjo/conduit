//! Actual ordinary-kernel storage assembly, shared with CPU conformance.
//! Assembly transfers ownership only; it dispatches no Step or native call.
use alloc::boxed::Box;
use conduit_environment_runtime::{
    appliance::{assigned::*, ApplianceBack},
    PreparedCaptureSelector,
};
use conduit_kernel::{self as k, scheduler::FixedScheduler};
pub type Kernel = FixedScheduler<
    ApplianceBack,
    k::HostedValueStore,
    k::HostedSignLog,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    { NODES * PORTS },
    CORDS,
    NODES,
    NODES,
>;
pub struct PreparedKernel {
    pub(crate) kernel: Box<Kernel>,
    pub(crate) selectors: Box<[Option<PreparedCaptureSelector>; NODES]>,
    pub(crate) roles: ResidentRoles,
}
#[inline(never)]
pub fn assemble(
    prepared: Box<PreparedAssignedAppliance<k::HostedValueStore>>,
    signs: k::HostedSignLog,
) -> Result<PreparedKernel, &'static str> {
    let prepared = {
        let boxed = prepared;
        *boxed
    }; // Retire the moved-out box at this block boundary, before scheduler allocation.
    let selectors = Box::new(prepared.selectors);
    let kernel = Kernel::new_boxed_with_active_counts_and_host_calls(
        NODES,
        CORDS,
        prepared.tables.nodes,
        prepared.tables.cords,
        prepared.tables.routes,
        prepared.tables.calls,
        prepared.drivers,
        prepared.values,
        signs,
    )
    .map_err(|_| "kernel-tables")?;
    Ok(PreparedKernel {
        kernel,
        selectors,
        roles: prepared.roles,
    })
}

#[cfg(feature = "target-memory-check")]
#[inline(never)]
pub fn assemble_local(
    prepared: PreparedLocalAppliance<'_, k::HostedValueStore>,
    signs: k::HostedSignLog,
) -> Result<PreparedKernel, &'static str> {
    let selectors = Box::new(prepared.selectors);
    let kernel = Kernel::new_boxed_with_active_counts_and_host_calls(
        NODES,
        CORDS,
        prepared.tables.nodes,
        prepared.tables.cords,
        prepared.tables.routes,
        prepared.tables.calls,
        prepared.drivers,
        prepared.values,
        signs,
    )
    .map_err(|_| "local-kernel-tables")?;
    Ok(PreparedKernel {
        kernel,
        selectors,
        roles: prepared.roles,
    })
}
