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
pub type KernelBacking = Box<core::mem::MaybeUninit<Kernel>>;
/// Fallible reservation before cold metadata fragments the Root heap. Nothing
/// is initialized or callable until checked numeric material is transferred.
pub fn preclaim_local_backing() -> Result<KernelBacking, &'static str> {
    let layout = core::alloc::Layout::new::<Kernel>();
    // SAFETY: the global allocator receives the exact nonzero Kernel layout;
    // Box subsequently owns that allocation, without reading its uninitialized data.
    let pointer = unsafe { alloc::alloc::alloc(layout) }.cast::<core::mem::MaybeUninit<Kernel>>();
    if pointer.is_null() {
        return Err("local-kernel-backing-capacity");
    }
    // SAFETY: the non-null pointer is uniquely owned and has Box's exact layout.
    Ok(unsafe { Box::from_raw(pointer) })
}
/// Settled original scheduler storage, not freshly initialized software. No
/// constructor can claim native possession or authorize another invocation.
pub struct RetiredKernelMaterial(PreparedKernel);
pub struct RetiredKernelParts {
    pub execution:
        k::scheduler::RetiredExecution<ApplianceBack, k::HostedValueStore, k::HostedSignLog, NODES>,
    pub selectors: Box<[Option<PreparedCaptureSelector>; NODES]>,
    pub roles: ResidentRoles,
}
impl RetiredKernelParts {
    /// Drop the actual terminal software and selectors, then empty its original
    /// value backing for separately prepared software. Slot generations and
    /// permanently retired slots survive `clear`; no finite Back is restarted.
    ///
    /// Evidence is returned intact so Root can finish the old Play before
    /// admitting a successor. This carries no native provider or start grant.
    pub fn into_reusable_backing(self) -> (k::HostedValueStore, k::HostedSignLog) {
        use k::ValueStorage;
        let Self {
            execution,
            selectors,
            roles: _,
        } = self;
        let k::scheduler::RetiredExecution {
            drivers,
            mut values,
            signs,
            ..
        } = execution;
        // No software reference may outlive clearing its store. These are the
        // original Backs from the terminal, drained scheduler, not new drivers.
        drop(drivers);
        drop(selectors);
        values.clear();
        (values, signs)
    }
}
impl RetiredKernelMaterial {
    pub fn kernel(&self) -> &Kernel {
        &self.0.kernel
    }
    pub fn selectors(&self) -> &[Option<PreparedCaptureSelector>; NODES] {
        &self.0.selectors
    }
    pub fn roles(&self) -> ResidentRoles {
        self.0.roles
    }
    /// Reclaim only backing after actual terminal material was accepted by
    /// Root. Unlike into_parts this never returns a full driver array by value.
    /// This does not accept terminal Signs or admit a successor on its own.
    pub fn into_reusable_backing(self) -> (k::HostedValueStore, k::HostedSignLog) {
        use k::ValueStorage;
        let PreparedKernel {
            kernel,
            selectors,
            roles: _,
        } = self.0;
        let (mut values, signs) = match kernel.try_retire_backing() {
            Ok(backing) => backing,
            Err(_) => unreachable!("exclusive preflighted retired kernel"),
        };
        drop(selectors);
        values.clear();
        (values, signs)
    }

    /// Consuming inspection/return only. Old drivers retain their terminal state;
    /// a new run still needs independently verified Source/configuration and claims.
    pub fn into_parts(self) -> RetiredKernelParts {
        let PreparedKernel {
            kernel,
            selectors,
            roles,
        } = self.0;
        let execution = match (*kernel).try_retire() {
            Ok(execution) => execution,
            Err(_) => unreachable!("exclusive preflighted retired kernel"),
        };
        RetiredKernelParts {
            execution,
            selectors,
            roles,
        }
    }
}
impl PreparedKernel {
    pub fn retirement_ready(&self) -> bool {
        self.kernel.can_retire()
    }
    pub fn try_retire(self) -> Result<RetiredKernelMaterial, Self> {
        if !self.retirement_ready() {
            return Err(self);
        }
        Ok(RetiredKernelMaterial(self))
    }
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
    compact_observation_edge: Option<VerifiedCompactObservationEdge>,
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
    pub fn validate_topology(&self) -> Result<(), &'static str> {
        Kernel::check_boxed_active_topology(
            NODES,
            CORDS,
            &self.nodes,
            &self.cords,
            &self.routes,
            &self.calls,
        )
        .map_err(|_| "local-kernel-tables")
    }
    pub fn from_checked(prepared: PreparedDirectLocalAppliance<'_, k::HostedValueStore>) -> Self {
        let tables = prepared.tables.into_parts();
        Self {
            compact_observation_edge: prepared.compact_observation_edge,
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
    assemble_local_direct_with_backing(prepared, signs, None)
}
#[cfg(feature = "target-mask-memory-check")]
#[inline(never)]
pub fn assemble_local_direct_with_backing(
    prepared: LocalKernelParts,
    signs: k::HostedSignLog,
    mut backing: Option<KernelBacking>,
) -> Result<PreparedKernel, &'static str> {
    assemble_local_direct_with_backing_retained(&mut Some(prepared), &mut Some(signs), &mut backing)
}
/// Refusal leaves actual software, values, evidence and backing in place.
/// Fresh allocation supplies memory only; it cannot restart an old execution.
#[cfg(feature = "target-mask-memory-check")]
// Keep affine preparation in its outlined caller so large numeric material
// shares stack slots instead of accumulating across nested transfer frames.
#[inline(always)]
pub fn assemble_local_direct_with_backing_retained(
    prepared: &mut Option<LocalKernelParts>,
    signs: &mut Option<k::HostedSignLog>,
    backing: &mut Option<KernelBacking>,
) -> Result<PreparedKernel, &'static str> {
    prepared
        .as_ref()
        .ok_or("local-kernel-parts-missing")?
        .validate_topology()?;
    if signs.is_none() {
        return Err("local-kernel-signs-missing");
    }
    if backing.is_none() {
        *backing = Some(preclaim_local_backing()?);
    }
    type Selectors = [Option<PreparedCaptureSelector>; NODES];
    let layout = core::alloc::Layout::new::<Selectors>();
    // SAFETY: this nonzero exact array layout is the subsequent Box's layout.
    // No initialized input is moved until both reservations have succeeded.
    let pointer =
        unsafe { alloc::alloc::alloc(layout) }.cast::<core::mem::MaybeUninit<Selectors>>();
    if pointer.is_null() {
        return Err("local-selector-backing-capacity");
    }
    let prepared = prepared
        .take()
        .expect("exclusively preflighted local material");
    let signs = signs
        .take()
        .expect("exclusively preflighted local evidence");
    // SAFETY: the allocation is non-null, aligned, uniquely owned and uninitialized.
    let selectors = Box::write(unsafe { Box::from_raw(pointer) }, prepared.selectors);
    let mut kernel = Kernel::new_boxed_with_storage_and_active_counts_and_host_calls(
        backing.take(),
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
    .unwrap_or_else(|_| unreachable!("immutable topology preflight and exact preclaimed backing"));
    if let Some(edge) = prepared.compact_observation_edge {
        // SAFETY: these exact immutable checked tables, actual initialized
        // drivers and affine values were moved from the witness's preparation.
        unsafe { edge.bind(&mut kernel) }.unwrap_or_else(|_| {
            unreachable!("checked compact Source edge and unchanged physical storage")
        });
    }
    Ok(PreparedKernel {
        kernel,
        selectors,
        roles: prepared.roles,
    })
}
