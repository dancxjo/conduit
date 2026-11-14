//! Single-allocation topology construction, with guarded inert fields.
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
    /// Fill initialized inert tables in the original funded allocation. The
    /// writer supplies no admission or execution authority. Every topology and
    /// terminal check runs before storage escapes; refusal drops all stores.
    /// No reference to an uninitialized scheduler or public raw pointer escapes.
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    pub fn prepare_boxed_with_table_writer<Q>(
        values: S,
        signs: E,
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
        // SAFETY: this private initializer moves the exact owned sign sink into
        // its final destination once, without calling user code or failing.
        unsafe {
            Self::prepare_boxed_with_sign_initializer(
                values,
                |target| target.write(signs),
                maximum_storage_bytes,
                contracts,
                actual,
                map_error,
                write,
            )
        }
    }
    // SAFETY: initializer must initialize exactly one E without unwinding or
    // failure. It is private to this preparation module; no public pointer API.
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    pub(super) unsafe fn prepare_boxed_with_sign_initializer<Q>(
        values: S,
        initialize_signs: impl FnOnce(*mut E),
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
        // Evaluate constructors before transferring any affine input owner.
        let ready = ReadySet::new(0);
        let debug_control = DebugControlState::new();
        let quiescence_cord_end = 0;
        #[cfg(test)]
        let scan_counts = core::cell::Cell::new(scan_characterization::ScanCounts::default());
        let mut storage = storage::allocate::<Self>(maximum_storage_bytes).map_err(&map_error)?;
        let pointer = storage.as_mut_ptr();
        macro_rules! initialize {
            ($( $(#[$attribute:meta])* $field:ident : $value:expr ),* $(,)?) => {{
                // This never-called exhaustive pattern makes adding a field
                // without initializing it a compile error. It reads no storage.
                let _check_fields = |value: &Self| {
                    let Self { drivers: _, signs: _, $( $(#[$attribute])* $field: _, )* } = value;
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
                    // SAFETY: the private infallible initializer wrote E once.
                    // Preserve owned value-store cleanup before the sign sink.
                    unsafe { core::ptr::drop_in_place(core::ptr::addr_of_mut!((*pointer).signs)); }
                };
                drop_initialized
            }};
        }
        // SAFETY: private caller guarantees exact infallible initialization.
        unsafe {
            initialize_signs(core::ptr::addr_of_mut!((*pointer).signs));
        }
        let drop_initialized = initialize! {
            node_specs: [NodeSpec { input_cords: [None; PORTS], maximum_step_fuel: 0 }; NODES],
            terminal_transductions: *contracts,
            terminal_inputs_consumed: [[false; PORTS]; NODES],
            terminal_phases: [None; NODES],
            terminal_cancellation_pending: [false; NODES],
            unresolved_abnormal: [None; NODES],
            projected_recovery_source: [None; NODES],
            recovery_for_cord: [None; CORDS],
            quiescence_cord_end: quiescence_cord_end,
            armed_quiescence_count: 0,
            cord_specs: [CordSpec::inactive(); CORDS],
            active_nodes: 0,
            active_cords: 0,
            routes: FixedRoutes::new(PORTS as u16),
            host_bindings: Some(FixedHostCallBindings::new(1)),
            pending_host_calls: [None; PENDING_REQUESTS],
            pending_host_count: 0,
            host_slot_search_start: 0,
            cancellation_search_start: 0,
            undispatched_host_count: 0,
            undispatched_cancellation_count: 0,
            pending_host_slots: [None; NODES],
            values: values,
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
            // SAFETY: active_nodes is initialized and Copy. Do not form an
            // incomplete Self reference or charge inert capacity-tail Backs.
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
        let prepared = PreparedKernelStorage::new(storage, drop_initialized, bind);
        // SAFETY: every field except drivers is initialized and exclusively
        // owned by prepared. Never form a reference to the incomplete Self.
        // The guard drops all initialized fields on writer/validation refusal.
        unsafe {
            let (active_nodes, active_cords) = write(
                &mut *core::ptr::addr_of_mut!((*pointer).node_specs),
                &mut *core::ptr::addr_of_mut!((*pointer).cord_specs),
                &mut *core::ptr::addr_of_mut!((*pointer).routes),
                (*core::ptr::addr_of_mut!((*pointer).host_bindings))
                    .as_mut()
                    .expect("initialized bindings"),
            )?;
            let nodes = &*core::ptr::addr_of!((*pointer).node_specs);
            let cords = &*core::ptr::addr_of!((*pointer).cord_specs);
            let routes = &*core::ptr::addr_of!((*pointer).routes);
            let calls = (*core::ptr::addr_of!((*pointer).host_bindings))
                .as_ref()
                .expect("initialized bindings");
            let validate = || -> Result<_, KernelPreparationError> {
                validate_active_capacity(active_nodes, NODES, active_cords, CORDS)?;
                if PORTS == 0
                    || QUEUE_SLOTS == 0
                    || PENDING_REQUESTS == 0
                    || !routes.is_sealed()
                    || !calls.is_sealed()
                {
                    return Err(SchedulerError::InvalidPlan.into());
                }
                routes.validate_active_prefix(active_nodes, active_cords)?;
                calls.validate_active_nodes(active_nodes)?;
                validate_plan::<NODES, CORDS, PORTS, QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS>(
                    active_nodes,
                    active_cords,
                    nodes,
                    cords,
                    routes,
                )?;
                for node in 0..NODES {
                    if actual[node] != contracts[node]
                        || (node >= active_nodes && contracts[node].iter().any(Option::is_some))
                    {
                        return Err(SchedulerError::InvalidPlan.into());
                    }
                }
                projected_recovery_map(active_cords, cords, contracts).map_err(Into::into)
            };
            let recovery = validate().map_err(&map_error)?;
            core::ptr::addr_of_mut!((*pointer).recovery_for_cord).write(recovery);
            core::ptr::addr_of_mut!((*pointer).active_nodes).write(active_nodes);
            core::ptr::addr_of_mut!((*pointer).active_cords).write(active_cords);
            core::ptr::addr_of_mut!((*pointer).ready).write(ReadySet::new(active_nodes));
            core::ptr::addr_of_mut!((*pointer).quiescence_cord_end)
                .write(quiescence::quiescence_cord_end(&cords[..active_cords]));
        }
        Ok(prepared)
    }
}
