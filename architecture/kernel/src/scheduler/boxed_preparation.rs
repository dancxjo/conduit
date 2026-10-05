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
        Ok(alloc::boxed::Box::new(Self {
            node_specs,
            terminal_transductions: [[None; PORTS]; NODES],
            terminal_inputs_consumed: [[false; PORTS]; NODES],
            terminal_phases: [None; NODES],
            terminal_cancellation_pending: [false; NODES],
            unresolved_abnormal: [None; NODES],
            projected_recovery_source: [None; NODES],
            recovery_for_cord: [None; CORDS],
            cord_specs,
            active_nodes,
            active_cords,
            routes,
            host_bindings: Some(host_bindings),
            pending_host_calls: [None; PENDING_REQUESTS],
            drivers,
            values,
            signs,
            cords: [CordState::EMPTY; CORDS],
            queue_slots: [None; QUEUE_SLOTS],
            ready: core::array::from_fn(|node| node < active_nodes),
            completed: [false; NODES],
            cursor: 0,
            host_request_cursor: 0,
            decisions: 0,
            last_host_request: [None; NODES],
            cancelled: false,
            debug_control: DebugControlState::new(),
        }))
    }
}
