//! Fixed-sign specialization of the original guarded allocation.
use super::*;
impl<
        D,
        S,
        const EVENTS: usize,
        const NODES: usize,
        const CORDS: usize,
        const PORTS: usize,
        const QUEUE_SLOTS: usize,
        const ROUTE_SLOTS: usize,
        const ROUTE_TARGETS: usize,
        const HOST_BINDING_SLOTS: usize,
        const PENDING_REQUESTS: usize,
        const TERMINAL_ROWS: usize,
        const PHASE_BYTES: usize,
    >
    FixedScheduler<
        D,
        S,
        crate::FixedSignLog<EVENTS>,
        NODES,
        CORDS,
        PORTS,
        QUEUE_SLOTS,
        ROUTE_SLOTS,
        ROUTE_TARGETS,
        HOST_BINDING_SLOTS,
        PENDING_REQUESTS,
        TERMINAL_ROWS,
        PHASE_BYTES,
    >
where
    D: StepBack<PORTS>,
    S: ValueStorage,
{
    /// Prepare the same funded scheduler with a fixed sign log initialized
    /// directly in its final field. Numeric budgets are checked before any
    /// allocation or table-writer callback. This grants no execution authority.
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    pub fn prepare_boxed_with_fixed_signs_and_table_writer<Q>(
        values: S,
        sign_byte_capacity: u32,
        remote_sign_item_capacity: u16,
        remote_sign_byte_capacity: u32,
        maximum_storage_bytes: usize,
        contracts: &[[Option<AssignedTerminalTransduction>; PORTS]; NODES],
        actual: &[[Option<AssignedTerminalTransduction>; PORTS]; NODES],
        map_error: impl Fn(KernelPreparationError) -> Q,
        write: impl FnOnce(
            &mut [NodeSpec<PORTS>; NODES],
            &mut [CordSpec; CORDS],
            &mut FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
            &mut FixedHostCallBindings<HOST_BINDING_SLOTS>,
        ) -> Result<(usize, usize), Q>,
    ) -> Result<PreparedKernelStorage<D, Self, NODES>, Q> {
        let budgets = crate::fixed_sign_preparation::FixedSignBudgets::checked::<EVENTS>(
            sign_byte_capacity,
            remote_sign_item_capacity,
            remote_sign_byte_capacity,
        )
        .map_err(|error| {
            map_error(KernelPreparationError::Topology(SchedulerError::Sign(
                error,
            )))
        })?;
        // SAFETY: checked budgets initialize the exact fixed log type directly;
        // the private field initializer cannot fail, unwind, or call user code.
        unsafe {
            Self::prepare_boxed_with_sign_initializer(
                values,
                |target| budgets.initialize::<EVENTS>(target),
                maximum_storage_bytes,
                contracts,
                actual,
                None,
                map_error,
                write,
            )
        }
    }
}
