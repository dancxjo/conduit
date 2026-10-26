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

#[cfg(any(feature = "target-memory-check", feature = "target-mask-memory-check"))]
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

/// Owned numeric execution material after exact Source/owner admission. This
/// carries no native possession or Body start, and no borrowed Plan metadata.
#[cfg(feature = "target-mask-memory-check")]
pub struct LocalKernelParts {
    nodes: [k::scheduler::NodeSpec<PORTS>; NODES],
    cords: [k::scheduler::CordSpec; CORDS],
    routes: k::FixedRoutes<NODES, CORDS>,
    calls: k::FixedHostCallBindings<NODES>,
    drivers: [ApplianceBack; NODES],
    selectors: [Option<PreparedCaptureSelector>; NODES],
    values: k::HostedValueStore,
    roles: ResidentRoles,
    pub sign_items: u16,
    pub sign_bytes: u32,
}
#[cfg(feature = "target-mask-memory-check")]
impl LocalKernelParts {
    pub fn from_checked(prepared: PreparedDirectLocalAppliance<'_, k::HostedValueStore>) -> Self {
        let tables = prepared.tables.into_parts();
        Self {
            sign_items: tables.sign_items,
            sign_bytes: tables.sign_bytes,
            nodes: tables.nodes,
            cords: tables.cords,
            routes: tables.routes,
            calls: tables.calls,
            drivers: prepared.drivers,
            selectors: prepared.selectors,
            values: prepared.values,
            roles: prepared.roles,
        }
    }
}
#[cfg(feature = "target-mask-memory-check")]
#[inline(never)]
pub fn assemble_local_direct(
    prepared: LocalKernelParts,
    signs: k::HostedSignLog,
) -> Result<PreparedKernel, &'static str> {
    let selectors = Box::new(prepared.selectors);
    let kernel = Kernel::new_boxed_with_active_counts_and_host_calls(
        NODES,
        CORDS,
        prepared.nodes,
        prepared.cords,
        prepared.routes,
        prepared.calls,
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
