//! Allocating preparation entrance for the unchanged fixed execution kernel.
use super::*;
mod fixed_signs;
mod storage;
mod table_writer;
pub use storage::{KernelPreparationError, PreparedKernelStorage};

impl<
        D,
        S,
        E,
        const NODES: usize,
        const CORDS: usize,
        const PORTS: usize,
        const QUEUE_SLOTS: usize,
        const ROUTE_SLOTS: usize,
        const ROUTE_TARGETS: usize,
        const HOST_BINDING_SLOTS: usize,
        const PENDING_REQUESTS: usize,
    >
    FixedScheduler<
        D,
        S,
        E,
        NODES,
        CORDS,
        PORTS,
        QUEUE_SLOTS,
        ROUTE_SLOTS,
        ROUTE_TARGETS,
        HOST_BINDING_SLOTS,
        PENDING_REQUESTS,
    >
where
    D: StepBack<PORTS>,
    S: ValueStorage,
    E: SignSink,
{
    /// Prepare the same fixed scheduler in allocated storage without carrying
    /// a large scheduler value through intermediate Result return frames.
    /// All allocation and topology validation happen before execution.
    #[allow(clippy::too_many_arguments)]
    pub fn new_boxed_with_active_counts_and_host_calls(
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostCallBindings<HOST_BINDING_SLOTS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<alloc::boxed::Box<Self>, KernelPreparationError> {
        Self::prepare_boxed(
            active_nodes,
            active_cords,
            node_specs,
            cord_specs,
            routes,
            host_bindings,
            values,
            signs,
            usize::MAX,
        )
        .map(|prepared| prepared.bind(drivers))
    }

    /// Validate topology and acquire physical storage before any Back transfer.
    /// The ceiling covers Self's inline payload; external backing and complete
    /// workset resources remain independently admitted by the caller.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_boxed(
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostCallBindings<HOST_BINDING_SLOTS>,
        values: S,
        signs: E,
        maximum_storage_bytes: usize,
    ) -> Result<PreparedKernelStorage<D, Self, NODES>, KernelPreparationError> {
        Self::prepare_boxed_with_terminal_transductions(
            active_nodes,
            active_cords,
            node_specs,
            cord_specs,
            routes,
            host_bindings,
            values,
            signs,
            maximum_storage_bytes,
            [[None; PORTS]; NODES],
            [[None; PORTS]; NODES],
        )
    }

    /// Compare independently lowered laws with the actual borrowed Back reports
    /// before transfer. The matrix and projected recovery map occupy already
    /// funded inline scheduler storage; binding performs no semantic checks.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_boxed_with_terminal_transductions(
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostCallBindings<HOST_BINDING_SLOTS>,
        values: S,
        signs: E,
        maximum_storage_bytes: usize,
        contracts: [[Option<AssignedTerminalTransduction>; PORTS]; NODES],
        actual: [[Option<AssignedTerminalTransduction>; PORTS]; NODES],
    ) -> Result<PreparedKernelStorage<D, Self, NODES>, KernelPreparationError> {
        Self::prepare_boxed_with_terminal_transductions_ref(
            active_nodes,
            active_cords,
            node_specs,
            cord_specs,
            routes,
            host_bindings,
            values,
            signs,
            maximum_storage_bytes,
            &contracts,
            &actual,
        )
    }

    /// Borrow immutable lowering/report matrices during the same original
    /// validation and allocation path. The expected matrix is copied directly
    /// into the already funded scheduler field; no duplicate matrix is retained.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_boxed_with_terminal_transductions_ref(
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostCallBindings<HOST_BINDING_SLOTS>,
        values: S,
        signs: E,
        maximum_storage_bytes: usize,
        contracts: &[[Option<AssignedTerminalTransduction>; PORTS]; NODES],
        actual: &[[Option<AssignedTerminalTransduction>; PORTS]; NODES],
    ) -> Result<PreparedKernelStorage<D, Self, NODES>, KernelPreparationError> {
        validate_active_capacity(active_nodes, NODES, active_cords, CORDS)?;
        if PORTS == 0
            || QUEUE_SLOTS == 0
            || !routes.is_sealed()
            || PENDING_REQUESTS == 0
            || !host_bindings.is_sealed()
        {
            return Err(KernelPreparationError::Topology(
                SchedulerError::InvalidPlan,
            ));
        }
        routes.validate_active_prefix(active_nodes, active_cords)?;
        host_bindings.validate_active_nodes(active_nodes)?;
        validate_plan::<NODES, CORDS, PORTS, QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS>(
            active_nodes,
            active_cords,
            &node_specs,
            &cord_specs,
            &routes,
        )?;
        for node in 0..NODES {
            if actual[node] != contracts[node]
                || (node >= active_nodes && contracts[node].iter().any(Option::is_some))
            {
                return Err(SchedulerError::InvalidPlan.into());
            }
        }
        let recovery_for_cord = projected_recovery_map(active_cords, &cord_specs, contracts)?;
        // Evaluate constructors before transferring any affine input owner.
        let ready = ReadySet::new(active_nodes);
        let debug_control = DebugControlState::new();
        let quiescence_cord_end = quiescence::quiescence_cord_end(&cord_specs[..active_cords]);
        #[cfg(test)]
        let scan_counts = core::cell::Cell::new(scan_characterization::ScanCounts::default());
        let mut storage = storage::allocate::<Self>(maximum_storage_bytes)?;
        let pointer = storage.as_mut_ptr();
        macro_rules! initialize {
            ($( $(#[$attribute:meta])* $field:ident : $value:expr ),* $(,)?) => {{
                // This never-called exhaustive pattern makes adding a field
                // without initializing it a compile error. It reads no storage.
                let _check_fields = |value: &Self| {
                    let Self { drivers: _, $( $(#[$attribute])* $field: _, )* } = value;
                };
                $(
                    $(#[$attribute])*
                    // SAFETY: uniquely owned exact-layout Box storage. No
                    // reference to uninitialized Self is formed, and each
                    // field is written once without reading or dropping it.
                    unsafe { core::ptr::addr_of_mut!((*pointer).$field).write($value); }
                )*
                // The same exhaustive field list defines cleanup of every
                // initialized owner. Drivers remain uninitialized until bind.
                let drop_initialized: unsafe fn(*mut Self) = |pointer| {
                    $(
                        $(#[$attribute])*
                        // SAFETY: this closure is installed only after all
                        // listed fields have been initialized exactly once.
                        unsafe { core::ptr::drop_in_place(core::ptr::addr_of_mut!((*pointer).$field)); }
                    )*
                };
                drop_initialized
            }};
        }
        let drop_initialized = initialize! {
            node_specs: node_specs,
            terminal_transductions: *contracts,
            terminal_inputs_consumed: [[false; PORTS]; NODES],
            terminal_phases: [None; NODES],
            terminal_cancellation_pending: [false; NODES],
            unresolved_abnormal: [None; NODES],
            projected_recovery_source: [None; NODES],
            recovery_for_cord: recovery_for_cord,
            quiescence_cord_end: quiescence_cord_end,
            armed_quiescence_count: 0,
            cord_specs: cord_specs,
            active_nodes: active_nodes,
            active_cords: active_cords,
            routes: routes,
            host_bindings: Some(host_bindings),
            pending_host_calls: [None; PENDING_REQUESTS],
            pending_host_count: 0,
            host_slot_search_start: 0,
            cancellation_search_start: 0,
            undispatched_host_count: 0,
            undispatched_cancellation_count: 0,
            pending_host_slots: [None; NODES],
            values: values,
            signs: signs,
            cords: [CordState::EMPTY; CORDS],
            queue_slots: [None; QUEUE_SLOTS],
            ready: ready,
            completed: [false; NODES],
            cursor: 0,
            host_request_cursor: 0,
            decisions: 0,
            logical_invocations: 0,
            region_successors: [None; NODES],
            native_input_ports: [None; NODES],
            native_output_ports: [None; NODES],
            compact_f32_bindings: None,
            #[cfg(feature = "execution-workspace")]
            execution_workspace: None,
            #[cfg(feature = "execution-workspace")]
            unmet_execution_workspace: false,
            last_host_request: [None; NODES],
            cancelled: false,
            debug_control: debug_control,
            #[cfg(test)]
            scan_counts: scan_counts,
        };
        let bind: unsafe fn(*mut Self, [D; NODES]) = |pointer, drivers| {
            // SAFETY: drivers is the one remaining uninitialized field in
            // uniquely owned exact-layout storage. No Self reference is formed.
            #[cfg(feature = "execution-workspace")]
            // SAFETY: active_nodes is an initialized Copy field in this exact
            // owner; no reference to incomplete Self is formed.
            let active = unsafe { core::ptr::addr_of!((*pointer).active_nodes).read() };
            #[cfg(feature = "execution-workspace")]
            let requires_workspace = drivers[..active]
                .iter()
                .filter_map(|back| back.workspace_requirements())
                .any(|requirements| {
                    // SAFETY: this Option is initialized; no reference to the
                    // incomplete scheduler or its driver field is formed.
                    let workspace =
                        unsafe { &*core::ptr::addr_of!((*pointer).execution_workspace) };
                    workspace
                        .as_ref()
                        .is_none_or(|workspace| !workspace.supports(requirements))
                });
            unsafe {
                core::ptr::addr_of_mut!((*pointer).drivers).write(drivers);
                #[cfg(feature = "execution-workspace")]
                core::ptr::addr_of_mut!((*pointer).unmet_execution_workspace)
                    .write(requires_workspace);
            }
        };
        Ok(PreparedKernelStorage::new(storage, drop_initialized, bind))
    }
}
