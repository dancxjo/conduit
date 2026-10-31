//! Allocating preparation entrance for the unchanged fixed execution kernel.
use super::*;

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
    /// Read-only form of the exact boxed-construction topology checks. This
    /// reserves no storage, transfers no owner and grants no execution scope.
    pub fn check_boxed_active_topology(
        active_nodes: usize,
        active_cords: usize,
        node_specs: &[NodeSpec<PORTS>; NODES],
        cord_specs: &[CordSpec; CORDS],
        routes: &FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: &FixedHostCallBindings<HOST_BINDING_SLOTS>,
    ) -> Result<(), SchedulerError> {
        validate_active_capacity(active_nodes, NODES, active_cords, CORDS)?;
        if PORTS == 0
            || QUEUE_SLOTS == 0
            || !routes.is_sealed()
            || PENDING_REQUESTS == 0
            || !host_bindings.is_sealed()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        routes.validate_active_prefix(active_nodes, active_cords)?;
        host_bindings.validate_active_nodes(active_nodes)?;
        validate_plan::<NODES, CORDS, PORTS, QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS>(
            active_nodes,
            active_cords,
            node_specs,
            cord_specs,
            routes,
        )?;
        Ok(())
    }

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
    ) -> Result<alloc::boxed::Box<Self>, SchedulerError> {
        Self::new_boxed_with_storage_and_active_counts_and_host_calls(
            None,
            active_nodes,
            active_cords,
            node_specs,
            cord_specs,
            routes,
            host_bindings,
            drivers,
            values,
            signs,
        )
    }

    /// Initialize exact preclaimed storage after the ordinary topology checks.
    /// Uninitialized storage supplies memory only, never execution authority.
    #[allow(clippy::too_many_arguments)]
    pub fn new_boxed_with_storage_and_active_counts_and_host_calls(
        storage: Option<alloc::boxed::Box<core::mem::MaybeUninit<Self>>>,
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostCallBindings<HOST_BINDING_SLOTS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<alloc::boxed::Box<Self>, SchedulerError> {
        Self::check_boxed_active_topology(
            active_nodes,
            active_cords,
            &node_specs,
            &cord_specs,
            &routes,
            &host_bindings,
        )?;
        validate_prepared_encoding_profile::<D, S, PORTS>(&drivers[..active_nodes], &values)?;
        // Evaluate constructors before moving any affine input into storage.
        // Then initialize each field directly at its final address rather than
        // materializing another complete scheduler in a return/temporary frame.
        let terminal_transductions = TerminalContracts::empty();
        let ready = ReadySet::new(active_nodes);
        let debug_control = DebugControlState::new();
        let quiescence_cord_end = quiescence::quiescence_cord_end(&cord_specs[..active_cords]);
        #[cfg(test)]
        let scan_counts = core::cell::Cell::new(scan_characterization::ScanCounts::default());
        let mut storage = storage.unwrap_or_else(alloc::boxed::Box::<Self>::new_uninit);
        let pointer = storage.as_mut_ptr();
        macro_rules! initialize {
            ($( $(#[$attribute:meta])* $field:ident : $value:expr ),* $(,)?) => {
                // An exhaustive pattern forces a compile error if Self gains a
                // field without updating this initializer. The closure is never
                // called and neither reads storage nor transfers an input owner.
                let _check_fields = |value: &Self| {
                    let Self { $( $(#[$attribute])* $field: _, )* } = value;
                };
                $(
                    $(#[$attribute])*
                    // SAFETY: pointer is uniquely owned exact-layout storage.
                    // addr_of_mut forms no reference to uninitialized Self;
                    // every field is written once without reading/dropping it.
                    unsafe { core::ptr::addr_of_mut!((*pointer).$field).write($value); }
                )*
            };
        }
        initialize! {
            node_specs: node_specs,
            terminal_transductions: terminal_transductions,
            terminal_inputs_consumed: [[false; PORTS]; NODES],
            terminal_phases: [None; NODES],
            terminal_cancellation_pending: [false; NODES],
            unresolved_abnormal: [None; NODES],
            projected_recovery_source: [None; NODES],
            recovery_for_cord: [None; CORDS],
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
            drivers: drivers,
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
            #[cfg(feature = "alloc")]
            compact_canonical_binding: None,
            last_host_request: [None; NODES],
            cancelled: false,
            debug_control: debug_control,
            #[cfg(test)]
            scan_counts: scan_counts,
        }
        // SAFETY: all fields (including each configuration-specific field) have
        // just been initialized. No initialized Self existed before this point.
        Ok(unsafe { storage.assume_init() })
    }
}
