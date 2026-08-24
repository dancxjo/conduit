use crate::boundary::augment_boundary_cords;
use crate::{BoxedKernelOperation, KernelOperationRegistry};
use conduit_core::{PlanFragment, PortDirection, PortId as SemanticPortId, ValuePayload};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, OperationDriver, RemoteIngressOutcome,
    SchedulerStatus,
};
use conduit_kernel::{
    CordEndpoint, CordId, FixedHostOperationBindings, FixedRoutes, HostedSignLog, HostedValueStore,
    KernelEvent, NodeId, PortId, RemoteEndpointId, ValueStorage,
};
use conduit_runtime::lowering::{LoweredPlanFragment, MAXIMUM_KERNEL_PORTS_PER_NODE};
use std::collections::BTreeMap;

pub(crate) const MAX_NODES: usize = 16;
pub(crate) const MAX_CORDS: usize = 32;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const MAX_QUEUE_SLOTS: usize = 256;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = MAX_CORDS;
const HOST_OPERATIONS_PER_NODE: u16 = 8;
const HOST_BINDING_SLOTS: usize = MAX_NODES * HOST_OPERATIONS_PER_NODE as usize;
const PENDING_REQUESTS: usize = MAX_NODES;

type ChildScheduler = FixedScheduler<
    OperationDriver<BoxedKernelOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    MAX_QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundaryEndpoint {
    pub external_port_id: SemanticPortId,
    pub internal_port_id: SemanticPortId,
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub direction: PortDirection,
    pub value_kind: conduit_core::KindId,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

pub(crate) struct ChildKernel {
    scheduler: ChildScheduler,
    boundaries: BTreeMap<SemanticPortId, BoundaryEndpoint>,
    status: SchedulerStatus,
}

impl ChildKernel {
    pub(crate) fn prepare(
        fragment: &PlanFragment,
        mut lowered: LoweredPlanFragment,
        boundaries: Vec<BoundaryEndpoint>,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, String> {
        augment_boundary_cords(&mut lowered, &boundaries)?;
        let active_nodes = lowered.nodes.len();
        let active_cords = lowered.cords.len();
        if active_nodes == 0
            || active_nodes > MAX_NODES
            || active_cords == 0
            || active_cords > MAX_CORDS
            || usize::from(lowered.cord_value_slots) > MAX_QUEUE_SLOTS
            || lowered.host_operations.len() > HOST_BINDING_SLOTS
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
        for placement in &fragment.placements {
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
        if usize::from(host_requests) > PENDING_REQUESTS {
            return Err("child exceeds the admitted kernel host-request profile".into());
        }
        let mut values = HostedValueStore::new(
            value_items.max(1),
            maximum_value_bytes.max(1),
            value_bytes.max(1),
        )
        .map_err(debug)?;
        let mut operations = Vec::with_capacity(MAX_NODES);
        for placement in &fragment.placements {
            let factory = registry
                .get(&placement.implementation_id)
                .ok_or_else(|| "installed implementation disappeared".to_string())?;
            operations.push(BoxedKernelOperation::new(
                factory.prepare(placement, &mut values)?,
            ));
        }
        while operations.len() < MAX_NODES {
            operations.push(BoxedKernelOperation::inactive());
        }
        let drivers = operations
            .into_iter()
            .map(OperationDriver::new)
            .collect::<Result<Vec<_>, _>>()
            .map_err(debug)?
            .try_into()
            .map_err(|_| "kernel composite driver capacity changed".to_string())?;

        let inactive_node = conduit_kernel::scheduler::NodeSpec {
            input_cords: [None; PORTS],
            maximum_step_work: 1,
        };
        let mut nodes = [inactive_node; MAX_NODES];
        nodes[..active_nodes].copy_from_slice(&lowered.node_specs);
        let inactive_cord = CordSpec {
            cord: CordId(u16::MAX),
            source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
            sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
            slot_start: u16::MAX,
            item_capacity: 0,
            byte_capacity: 0,
        };
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
        let mut host_operations =
            FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(HOST_OPERATIONS_PER_NODE);
        for operation in &lowered.host_operations {
            host_operations
                .install(operation.node, operation.binding)
                .map_err(debug)?;
        }
        host_operations.seal().map_err(debug)?;
        let sign_bytes = u32::from(sign_items)
            .checked_mul(u32::try_from(core::mem::size_of::<KernelEvent>()).map_err(debug)?)
            .ok_or_else(|| "kernel composite Sign byte bound overflow".to_string())?;
        let signs = HostedSignLog::new_with_remote_storage(
            sign_items,
            sign_bytes,
            u16::try_from(active_cords * 8).map_err(debug)?.max(1),
            conduit_kernel::remote_sign_storage_bytes(
                u16::try_from(active_cords * 8).map_err(debug)?.max(1),
            )
            .ok_or_else(|| "kernel composite remote Sign byte bound overflow".to_string())?,
        )
        .map_err(debug)?;
        let scheduler = ChildScheduler::new_with_active_counts_and_host_operations(
            active_nodes,
            active_cords,
            nodes,
            cords,
            routes,
            host_operations,
            drivers,
            values,
            signs,
        )
        .map_err(debug)?;
        Ok(Self {
            scheduler,
            boundaries: boundaries
                .into_iter()
                .map(|boundary| (boundary.external_port_id.clone(), boundary))
                .collect(),
            status: SchedulerStatus::Idle,
        })
    }

    pub(crate) fn step(&mut self) -> Result<SchedulerStatus, String> {
        self.status = self.scheduler.step().map_err(debug)?;
        Ok(self.status)
    }

    pub(crate) fn status(&self) -> SchedulerStatus {
        self.status
    }

    pub(crate) fn next_host_request(&mut self) -> Option<HostOperationRequest> {
        self.scheduler.next_host_request()
    }

    pub(crate) fn complete_host_operation(
        &mut self,
        node: NodeId,
        request: conduit_kernel::RequestId,
        outcome: conduit_kernel::HostOperationOutcome,
    ) -> Result<(), String> {
        self.scheduler
            .complete_host_operation(node, request, outcome)
            .map_err(debug)
    }

    pub(crate) fn admit_boundary(
        &mut self,
        port_id: &SemanticPortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<RemoteIngressOutcome, String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Input)
            .ok_or_else(|| "unknown composite input face".to_string())?;
        if boundary.value_kind != value.value_kind {
            return Err("composite input value kind differs from its exact face".into());
        }
        self.scheduler
            .admit_remote_input(boundary.endpoint, boundary.cord, sequence, &value.encoded)
            .map_err(debug)
    }

    pub(crate) fn close_boundary(&mut self, port_id: &SemanticPortId) -> Result<(), String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Input)
            .ok_or_else(|| "unknown composite input face".to_string())?;
        self.scheduler
            .close_remote_input(boundary.endpoint, boundary.cord)
            .map_err(debug)
    }

    pub(crate) fn boundary_output(
        &mut self,
        port_id: &SemanticPortId,
    ) -> Result<Option<(u64, ValuePayload)>, String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Output)
            .ok_or_else(|| "unknown composite output face".to_string())?;
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(boundary.endpoint, boundary.cord)
            .map_err(debug)?
        else {
            return Ok(None);
        };
        let bytes = self
            .scheduler
            .values()
            .get(offer.value)
            .map_err(debug)?
            .to_vec();
        Ok(Some((
            offer.sequence,
            ValuePayload {
                value_kind: boundary.value_kind.clone(),
                encoded: bytes,
            },
        )))
    }

    pub(crate) fn deliver_boundary(
        &mut self,
        port_id: &SemanticPortId,
        sequence: u64,
    ) -> Result<(), String> {
        let boundary = self
            .boundaries
            .get(port_id)
            .filter(|boundary| boundary.direction == PortDirection::Output)
            .ok_or_else(|| "unknown composite output face".to_string())?;
        self.scheduler
            .remote_egress_accept(boundary.endpoint, boundary.cord, sequence)
            .and_then(|()| {
                self.scheduler
                    .remote_egress_delivered(boundary.endpoint, boundary.cord, sequence)
            })
            .map_err(debug)
    }

    pub(crate) fn cancel(&mut self) -> Result<(), String> {
        self.scheduler.cancel().map_err(debug)?;
        self.status = SchedulerStatus::Cancelled;
        Ok(())
    }

    pub(crate) fn signs(&self) -> Vec<KernelEvent> {
        self.scheduler.signs().events().collect()
    }

    pub(crate) fn remote_offer(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<(u64, Vec<u8>)>, String> {
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(endpoint, cord)
            .map_err(debug)?
        else {
            return Ok(None);
        };
        Ok(Some((
            offer.sequence,
            self.scheduler
                .values()
                .get(offer.value)
                .map_err(debug)?
                .to_vec(),
        )))
    }

    pub(crate) fn remote_delivered(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), String> {
        self.scheduler
            .remote_egress_accept(endpoint, cord, sequence)
            .and_then(|()| {
                self.scheduler
                    .remote_egress_delivered(endpoint, cord, sequence)
            })
            .map_err(debug)
    }

    pub(crate) fn remote_admit(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        self.scheduler
            .admit_remote_input(endpoint, cord, sequence, bytes)
            .map_err(debug)
    }

    pub(crate) fn remote_terminal(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<bool, String> {
        self.scheduler
            .remote_egress_terminal(endpoint, cord)
            .map_err(debug)
    }

    pub(crate) fn remote_close(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<(), String> {
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(debug)
    }
}

fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
