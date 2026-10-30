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
            &node_specs,
            &cord_specs,
            &routes,
        )?;
        // Evaluate constructors before transferring any affine input owner.
        let ready = ReadySet::new(active_nodes);
        let debug_control = DebugControlState::new();
        let quiescence_cord_end = quiescence::quiescence_cord_end(&cord_specs[..active_cords]);
        #[cfg(test)]
        let scan_counts = core::cell::Cell::new(scan_characterization::ScanCounts::default());
        let mut storage = alloc::boxed::Box::<Self>::new_uninit();
        let pointer = storage.as_mut_ptr();
        macro_rules! initialize {
            ($( $(#[$attribute:meta])* $field:ident : $value:expr ),* $(,)?) => {
                // This never-called exhaustive pattern makes adding a field
                // without initializing it a compile error. It reads no storage.
                let _check_fields = |value: &Self| {
                    let Self { $( $(#[$attribute])* $field: _, )* } = value;
                };
                $(
                    $(#[$attribute])*
                    // SAFETY: uniquely owned exact-layout Box storage. No
                    // reference to uninitialized Self is formed, and each
                    // field is written once without reading or dropping it.
                    unsafe { core::ptr::addr_of_mut!((*pointer).$field).write($value); }
                )*
            };
        }
        initialize! {
            node_specs: node_specs,
            terminal_transductions: [[None; PORTS]; NODES],
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
            last_host_request: [None; NODES],
            cancelled: false,
            debug_control: debug_control,
            #[cfg(test)]
            scan_counts: scan_counts,
        }
        // SAFETY: every field is initialized, including cfg-specific fields.
        Ok(unsafe { storage.assume_init() })
    }
}
