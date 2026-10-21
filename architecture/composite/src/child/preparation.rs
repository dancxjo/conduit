//! Admit child storage, routes, Host Calls and installed Backs before execution.
use super::*;
mod tables;
use crate::{boundary::augment_boundary_cords, KernelOperationRegistry};
use conduit_core::PlanFragment;
use conduit_kernel::scheduler::CordSpec;
use conduit_kernel::{FixedHostCallBindings, FixedRoutes};
use conduit_plan_lowering::lowering::LoweredPlanFragment;
use tables::ChildPreparationTables;

impl ChildKernel {
    /// Fixed inline/boxed backing only. Values, Signs and owned Back heaps are
    /// additional; this is a preflight floor rather than a footprint receipt.
    pub(crate) const fn fixed_storage_floor_bytes() -> usize {
        core::mem::size_of::<Self>() + core::mem::size_of::<RawChildScheduler>()
    }

    pub(crate) fn prepare(
        fragment: &PlanFragment,
        mut lowered: LoweredPlanFragment,
        boundaries: Vec<BoundaryEndpoint>,
        registry: &KernelOperationRegistry,
        sign_storage: crate::KernelCompositeSignStorage,
        reusable: bool,
    ) -> Result<Self, String> {
        augment_boundary_cords(&mut lowered, &boundaries)?;
        let placements: Vec<_> = fragment.placements.iter().collect();
        Self::prepare_selected(
            &placements,
            lowered.into(),
            boundaries,
            registry,
            sign_storage,
            reusable,
            false,
        )
    }

    pub(crate) fn prepare_region(
        tables: &conduit_plan_lowering::execution_regions::PreparedRegionKernelTables<'_, '_>,
        placements: &[&conduit_core::PlannedGear],
        registry: &KernelOperationRegistry,
        sign_storage: crate::KernelCompositeSignStorage,
    ) -> Result<Self, String> {
        Self::prepare_selected(
            placements,
            ChildPreparationTables::from_region(tables)?,
            Vec::new(),
            registry,
            sign_storage,
            false,
            true,
        )
    }

    fn prepare_selected(
        placements: &[&conduit_core::PlannedGear],
        lowered: ChildPreparationTables,
        boundaries: Vec<BoundaryEndpoint>,
        registry: &KernelOperationRegistry,
        sign_storage: crate::KernelCompositeSignStorage,
        reusable: bool,
        worker: bool,
    ) -> Result<Self, String> {
        if reusable
            && (!lowered.host_calls.is_empty()
                || placements.iter().any(|p| {
                    !p.host_calls.is_empty()
                        || p.base.is_some()
                        || !p.resources.is_empty()
                        || !p.authority.is_empty()
                })
                || lowered.cords.iter().any(|c| {
                    !matches!(
                        (c.spec.source, c.spec.sink),
                        (
                            conduit_kernel::CordEndpoint::Local { .. },
                            conduit_kernel::CordEndpoint::Remote(_)
                        ) | (
                            conduit_kernel::CordEndpoint::Remote(_),
                            conduit_kernel::CordEndpoint::Local { .. }
                        )
                    )
                }))
        {
            return Err(
                "reusable child requires pure local Value boundaries without effects or Lines"
                    .into(),
            );
        }
        let mut input_targets = BTreeMap::new();
        for boundary in boundaries
            .iter()
            .filter(|boundary| boundary.direction == PortDirection::Input)
        {
            let targets = if boundary.already_lowered {
                lowered
                    .fore_ports
                    .iter()
                    .filter(|fore| {
                        fore.front_port_id == boundary.external_port_id
                            && fore.direction == PortDirection::Input
                    })
                    .map(|fore| (fore.endpoint, fore.cord))
                    .collect()
            } else {
                vec![(boundary.endpoint, boundary.cord)]
            };
            input_targets.insert(boundary.external_port_id.clone(), targets);
        }
        let active_nodes = lowered.nodes.len();
        let active_cords = lowered.cords.len();
        if active_nodes == 0
            || active_nodes > MAX_NODES
            || active_cords == 0
            || active_cords > MAX_CORDS
            || usize::from(lowered.cord_value_slots) > MAX_QUEUE_SLOTS
            || lowered.host_calls.len() > HOST_BINDING_SLOTS
        {
            return Err("child exceeds the admitted kernel composite profile".into());
        }

        let mut value_items = lowered.cord_value_slots;
        let mut value_bytes = lowered.cord_value_bytes;
        let mut maximum_value_bytes = lowered
            .cords
            .iter()
            .map(|cord| cord.spec.byte_capacity)
            .max()
            .unwrap_or(1);
        let mut host_requests = 0u16;
        let mut sign_items = lowered
            .sign_items
            .checked_add(u16::try_from(active_nodes * 8 + active_cords * 8).map_err(debug)?)
            .ok_or_else(|| "kernel composite Sign bound overflow".to_string())?;
        for placement in placements {
            let factory = registry.get(&placement.implementation_id).ok_or_else(|| {
                format!(
                    "implementation '{}' is not installed",
                    placement.implementation_id.as_str()
                )
            })?;
            let budget = factory.budget(placement)?;
            value_items = value_items
                .checked_add(budget.value_items)
                .ok_or_else(|| "kernel composite value item bound overflow".to_string())?;
            value_bytes = value_bytes
                .checked_add(budget.value_bytes)
                .ok_or_else(|| "kernel composite value byte bound overflow".to_string())?;
            maximum_value_bytes = maximum_value_bytes.max(budget.maximum_value_bytes);
            host_requests = host_requests
                .checked_add(budget.host_requests)
                .ok_or_else(|| "kernel composite host-request bound overflow".to_string())?;
            sign_items = sign_items
                .checked_add(budget.sign_items)
                .ok_or_else(|| "kernel composite Sign bound overflow".to_string())?;
        }
        sign_items = sign_items
            .checked_add(sign_storage.additional_local_items)
            .ok_or_else(|| "kernel composite Sign bound overflow".to_string())?;
        let remote_sign_items = u16::try_from(active_cords * 8)
            .map_err(debug)?
            .max(1)
            .checked_add(sign_storage.additional_remote_items)
            .ok_or_else(|| "kernel composite remote Sign bound overflow".to_string())?;
        if reusable && host_requests != 0 {
            return Err("reusable child cannot admit Host requests".into());
        }
        if usize::from(host_requests) > PENDING_REQUESTS {
            return Err("child exceeds the admitted kernel host-request profile".into());
        }
        let mut values = HostedValueStore::new(
            value_items.max(1),
            maximum_value_bytes.max(1),
            value_bytes.max(1),
        )
        .map_err(debug)?;
        let mut backs = Vec::with_capacity(MAX_NODES);
        let mut worker_back_bytes = 0usize;
        for placement in placements {
            let factory = registry
                .get(&placement.implementation_id)
                .ok_or_else(|| "installed implementation disappeared".to_string())?;
            backs.push(if worker {
                let (back, bytes) = factory.prepare_worker(placement, &mut values)?.into_parts();
                worker_back_bytes = worker_back_bytes
                    .checked_add(bytes)
                    .ok_or("worker Back memory overflow")?;
                BoxedKernelBack::new(back)
            } else if reusable {
                BoxedKernelBack::reusable(factory.prepare_reusable_value(placement, &mut values)?)
            } else {
                BoxedKernelBack::new(factory.prepare(placement, &mut values)?)
            });
        }
        while backs.len() < MAX_NODES {
            backs.push(BoxedKernelBack::inactive());
        }
        let backs = backs
            .try_into()
            .map_err(|_| "kernel composite Back capacity changed".to_string())?;

        let inactive_node = conduit_kernel::scheduler::NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_fuel: 1,
        };
        let mut nodes = [inactive_node; MAX_NODES];
        nodes[..active_nodes].copy_from_slice(&lowered.node_specs);
        let inactive_cord = CordSpec::inactive();
        let mut cords = [inactive_cord; MAX_CORDS];
        for (destination, source) in cords.iter_mut().zip(&lowered.cords) {
            *destination = source.spec;
        }
        let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(debug)?;
        }
        routes.seal().map_err(debug)?;
        let mut host_calls = FixedHostCallBindings::<HOST_BINDING_SLOTS>::new(HOST_CALLS_PER_NODE);
        for operation in &lowered.host_calls {
            host_calls
                .install(operation.node, operation.binding)
                .map_err(debug)?;
        }
        host_calls.seal().map_err(debug)?;
        let sign_bytes = u32::from(sign_items)
            .checked_mul(u32::try_from(core::mem::size_of::<KernelEvent>()).map_err(debug)?)
            .ok_or_else(|| "kernel composite Sign byte bound overflow".to_string())?;
        let signs = HostedSignLog::new_with_remote_storage(
            sign_items,
            sign_bytes,
            remote_sign_items,
            conduit_kernel::remote_sign_storage_bytes(remote_sign_items)
                .ok_or_else(|| "kernel composite remote Sign byte bound overflow".to_string())?,
        )
        .map_err(debug)?;
        let mut scheduler = RawChildScheduler::new_boxed_with_active_counts_and_host_calls(
            active_nodes,
            active_cords,
            nodes,
            cords,
            routes,
            host_calls,
            backs,
            values,
            signs,
        )
        .map_err(debug)?;
        let mut terminal_contracts = [[None; PORTS]; MAX_NODES];
        for node in &lowered.nodes {
            let contracts = terminal_contracts
                .get_mut(usize::from(node.node.0))
                .ok_or_else(|| "terminal contract node exceeds the admitted profile".to_string())?;
            for contract in &node.terminal_transductions {
                let assigned = contract.assigned();
                let slot = contracts
                    .get_mut(usize::from(assigned.input.0))
                    .ok_or_else(|| {
                        "terminal contract input exceeds the admitted profile".to_string()
                    })?;
                if slot.replace(assigned).is_some() {
                    return Err("duplicate lowered terminal contract input".to_string());
                }
            }
        }
        scheduler
            .bind_terminal_transductions(terminal_contracts)
            .map_err(debug)?;
        let scheduler = ChildScheduler::new(scheduler, &boundaries, remote_sign_items, reusable)?;
        Ok(Self {
            scheduler,
            input_targets,
            boundaries: boundaries
                .into_iter()
                .map(|boundary| (boundary.external_port_id.clone(), boundary))
                .collect(),
            status: SchedulerStatus::Idle,
            worker_back_bytes: worker.then_some(worker_back_bytes),
        })
    }
}

fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
