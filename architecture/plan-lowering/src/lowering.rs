//! Exact pre-Play-start lowering from string-identified plan facts into the
//! numeric tables consumed by `conduit-kernel`.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use conduit_core::{
    ActivePlayId, ActivePlayIdentity, AdmittedLine, BootId, ConnectionId, ExpectedSign, FragmentId,
    HostId, HostOperationContractId, KindId, LinkEndpoint, PlacementId, PlanFragment, PlanId,
    PortDescriptor, PortDirection, PortId as PlanPortId, PresentationId, PresentationIdentity,
    ResourceBinding as PlanResourceBinding, SharedPoolId, SignId, SignIdentity,
};
use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, NodeSpec},
    CordId, HostOperationBinding, HostOperationId, NodeId, PortId, RemoteEndpointId,
    ResourceBinding as KernelResourceBinding, ResourceId, RouteRange, RouteTarget,
    SignExpectationId, SignExpectationTarget,
};

mod admission;
mod fusion;
mod profile;
mod remote;
mod shared_pool;
use fusion::lower_fusions;
pub use fusion::LoweredFusion;
pub use profile::{
    KernelStorageProfile, KernelStorageProfileError, FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
    FIXED_KERNEL_STORAGE_PROFILE,
};
use remote::lower_remote_endpoints;
use shared_pool::lower_shared_pools;
pub use shared_pool::{LoweredPoolRealization, LoweredSharedPool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    InvalidFragment,
    UnsupportedState(conduit_core::StateId),
    EmptyFragment,
    CapacityOverflow,
    ProfileCapacityExceeded {
        placement_id: PlacementId,
        direction: PortDirection,
        required: usize,
        available: usize,
    },
    DuplicatePlacement(PlacementId),
    DuplicateConnection(ConnectionId),
    DuplicatePort {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    UnknownConnectionEndpoint(ConnectionId),
    UnknownConnectionPort(ConnectionId),
    ConnectionContractMismatch(ConnectionId),
    InvalidConnectionBudget(ConnectionId),
    InvalidRemoteConnection(ConnectionId),
    MultipleConnectionsToInput {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    PortDirectionMismatch {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    UnsupportedHostOperationConcurrency(PlacementId),
    ResourceBindingInvalid(PlacementId),
    SignBudgetInvalid,
    SignReferenceMissing,
    SharedPoolInvalid(SharedPoolId),
    SharedPoolConsumerMissing(SharedPoolId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPort {
    pub node: NodeId,
    pub port: PortId,
    pub port_id: PlanPortId,
    pub value_kind: KindId,
    pub direction: PortDirection,
    pub temporal: conduit_core::PortTemporal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredNode {
    pub node: NodeId,
    pub placement_id: PlacementId,
    pub maximum_step_work: u16,
    pub inputs: Vec<LoweredPort>,
    pub outputs: Vec<LoweredPort>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredCord {
    pub connection_id: ConnectionId,
    pub spec: CordSpec,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RemoteCordDirection {
    Egress,
    Ingress,
}

/// Exact identity binding retained outside the allocation-independent kernel.
/// The host must bind this numeric endpoint to this admitted Line before
/// trigger; the Base adapter is not allowed to choose or rewrite any fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredRemoteEndpoint {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub connection_id: ConnectionId,
    pub source_fragment_id: FragmentId,
    pub sink_fragment_id: FragmentId,
    pub direction: RemoteCordDirection,
    pub local: LinkEndpoint,
    pub peer: LinkEndpoint,
    pub value_kind: KindId,
    pub temporal: conduit_core::PortTemporal,
    pub line: AdmittedLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredRoute {
    pub source_node: NodeId,
    pub source_port: PortId,
    pub range: RouteRange,
    pub targets: Vec<RouteTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredHostOperation {
    pub node: NodeId,
    pub operation: HostOperationId,
    pub contract_id: HostOperationContractId,
    pub target_kind: Option<KindId>,
    pub maximum_in_flight: u16,
    pub binding: HostOperationBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredResource {
    pub node: NodeId,
    pub binding: KernelResourceBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSign {
    pub expectation: SignExpectationId,
    pub expected: ExpectedSign,
    pub target: SignExpectationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelPortIdentity {
    pub node: NodeId,
    pub direction: PortDirection,
    pub port: PortId,
    pub port_id: PlanPortId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelIdentityMap {
    pub plan_id: PlanId,
    pub fragment_id: FragmentId,
    pub placements: Vec<(NodeId, PlacementId)>,
    pub ports: Vec<KernelPortIdentity>,
    pub connections: Vec<(CordId, ConnectionId)>,
    pub remote_endpoints: Vec<(RemoteEndpointId, ConnectionId)>,
    pub host_operations: Vec<(NodeId, HostOperationId, HostOperationContractId)>,
    pub resources: Vec<(NodeId, ResourceId, PlanResourceBinding)>,
}

impl KernelIdentityMap {
    pub fn placement_for_node(&self, node: NodeId) -> Option<&PlacementId> {
        self.placements
            .iter()
            .find(|(candidate, _)| *candidate == node)
            .map(|(_, placement)| placement)
    }

    pub fn node_for_placement(&self, placement: &PlacementId) -> Option<NodeId> {
        self.placements
            .iter()
            .find(|(_, candidate)| candidate == placement)
            .map(|(node, _)| *node)
    }

    pub fn connection_for_cord(&self, cord: CordId) -> Option<&ConnectionId> {
        self.connections
            .iter()
            .find(|(candidate, _)| *candidate == cord)
            .map(|(_, connection)| connection)
    }

    pub fn cord_for_connection(&self, connection: &ConnectionId) -> Option<CordId> {
        self.connections
            .iter()
            .find(|(_, candidate)| candidate == connection)
            .map(|(cord, _)| *cord)
    }

    pub fn connection_for_remote_endpoint(
        &self,
        endpoint: RemoteEndpointId,
    ) -> Option<&ConnectionId> {
        self.remote_endpoints
            .iter()
            .find(|(candidate, _)| *candidate == endpoint)
            .map(|(_, connection)| connection)
    }

    pub fn remote_endpoint_for_connection(
        &self,
        connection: &ConnectionId,
    ) -> Option<RemoteEndpointId> {
        self.remote_endpoints
            .iter()
            .find(|(_, candidate)| candidate == connection)
            .map(|(endpoint, _)| *endpoint)
    }

    pub fn port_identity(
        &self,
        node: NodeId,
        direction: PortDirection,
        port: PortId,
    ) -> Option<&KernelPortIdentity> {
        self.ports.iter().find(|identity| {
            identity.node == node && identity.direction == direction && identity.port == port
        })
    }

    pub fn port_for_identity(
        &self,
        node: NodeId,
        direction: PortDirection,
        port_id: &PlanPortId,
    ) -> Option<PortId> {
        self.ports
            .iter()
            .find(|identity| {
                identity.node == node
                    && identity.direction == direction
                    && &identity.port_id == port_id
            })
            .map(|identity| identity.port)
    }

    pub fn host_operation_contract(
        &self,
        node: NodeId,
        operation: HostOperationId,
    ) -> Option<&HostOperationContractId> {
        self.host_operations
            .iter()
            .find(|(candidate_node, candidate_operation, _)| {
                *candidate_node == node && *candidate_operation == operation
            })
            .map(|(_, _, contract)| contract)
    }

    pub fn host_operation_for_contract(
        &self,
        node: NodeId,
        contract: &HostOperationContractId,
    ) -> Option<HostOperationId> {
        self.host_operations
            .iter()
            .find(|(candidate_node, _, candidate_contract)| {
                *candidate_node == node && candidate_contract == contract
            })
            .map(|(_, operation, _)| *operation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionIdentityError {
    WrongPlan,
    WrongActivePlay,
    WrongHost,
    UnknownNode,
    UnknownHostOperation,
    UnknownRequest,
    UnknownPresentation,
    DuplicateIdentity,
    CapacityExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelHostRequestIdentity {
    pub node: NodeId,
    pub request: conduit_kernel::RequestId,
    pub operation: HostOperationId,
    pub contract_id: HostOperationContractId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelPresentationIdentity {
    pub node: NodeId,
    pub request: conduit_kernel::RequestId,
    pub presentation_id: PresentationId,
    pub placement_id: PlacementId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelSignIdentity {
    pub sign_id: SignId,
    pub node: Option<NodeId>,
    pub request: Option<conduit_kernel::RequestId>,
    pub presentation_id: Option<PresentationId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelExecutionIdentityMap {
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    host_id: HostId,
    boot_id: BootId,
    requests: Vec<KernelHostRequestIdentity>,
    presentations: Vec<KernelPresentationIdentity>,
    signs: Vec<KernelSignIdentity>,
}

impl KernelExecutionIdentityMap {
    pub fn new(
        lowered: &KernelIdentityMap,
        active_play: &ActivePlayIdentity,
        request_capacity: usize,
        presentation_capacity: usize,
        sign_capacity: usize,
    ) -> Result<Self, ExecutionIdentityError> {
        if active_play.plan_id != lowered.plan_id {
            return Err(ExecutionIdentityError::WrongPlan);
        }
        Ok(Self {
            plan_id: lowered.plan_id.clone(),
            active_play_id: active_play.active_play_id.clone(),
            host_id: active_play.host_id.clone(),
            boot_id: active_play.boot_id.clone(),
            requests: Vec::with_capacity(request_capacity),
            presentations: Vec::with_capacity(presentation_capacity),
            signs: Vec::with_capacity(sign_capacity),
        })
    }

    pub fn bind_request(
        &mut self,
        lowered: &KernelIdentityMap,
        node: NodeId,
        request: conduit_kernel::RequestId,
        operation: HostOperationId,
    ) -> Result<(), ExecutionIdentityError> {
        let contract_id = lowered
            .host_operation_contract(node, operation)
            .ok_or(ExecutionIdentityError::UnknownHostOperation)?;
        if self.requests.len() >= self.requests.capacity() {
            return Err(ExecutionIdentityError::CapacityExceeded);
        }
        if self
            .requests
            .iter()
            .any(|identity| identity.node == node && identity.request == request)
        {
            return Err(ExecutionIdentityError::DuplicateIdentity);
        }
        self.requests.push(KernelHostRequestIdentity {
            node,
            request,
            operation,
            contract_id: contract_id.clone(),
        });
        Ok(())
    }

    pub fn bind_presentation(
        &mut self,
        lowered: &KernelIdentityMap,
        node: NodeId,
        request: conduit_kernel::RequestId,
        presentation: &PresentationIdentity,
    ) -> Result<(), ExecutionIdentityError> {
        if presentation.active_play_id != self.active_play_id {
            return Err(ExecutionIdentityError::WrongActivePlay);
        }
        if lowered.node_for_placement(&presentation.placement_id) != Some(node) {
            return Err(ExecutionIdentityError::UnknownNode);
        }
        if self.request(node, request).is_none() {
            return Err(ExecutionIdentityError::UnknownRequest);
        }
        if self.presentations.len() >= self.presentations.capacity() {
            return Err(ExecutionIdentityError::CapacityExceeded);
        }
        if self.presentations.iter().any(|identity| {
            identity.presentation_id == presentation.presentation_id
                || (identity.node == node && identity.request == request)
        }) {
            return Err(ExecutionIdentityError::DuplicateIdentity);
        }
        self.presentations.push(KernelPresentationIdentity {
            node,
            request,
            presentation_id: presentation.presentation_id.clone(),
            placement_id: presentation.placement_id.clone(),
        });
        Ok(())
    }

    pub fn bind_sign(
        &mut self,
        sign: &SignIdentity,
        node: Option<NodeId>,
        request: Option<conduit_kernel::RequestId>,
        presentation_id: Option<&PresentationId>,
    ) -> Result<(), ExecutionIdentityError> {
        if sign.active_play_id.as_ref() != Some(&self.active_play_id) {
            return Err(ExecutionIdentityError::WrongActivePlay);
        }
        if sign.host_id != self.host_id || sign.boot_id != self.boot_id {
            return Err(ExecutionIdentityError::WrongHost);
        }
        if node.is_some() != request.is_some() {
            return Err(ExecutionIdentityError::UnknownRequest);
        }
        if let Some((node, request)) = node.zip(request) {
            if self.request(node, request).is_none() {
                return Err(ExecutionIdentityError::UnknownRequest);
            }
        }
        if let Some(presentation_id) = presentation_id {
            let presentation = self
                .presentation(presentation_id)
                .ok_or(ExecutionIdentityError::UnknownPresentation)?;
            if Some(presentation.node) != node || Some(presentation.request) != request {
                return Err(ExecutionIdentityError::UnknownPresentation);
            }
        }
        if self.signs.len() >= self.signs.capacity() {
            return Err(ExecutionIdentityError::CapacityExceeded);
        }
        if self
            .signs
            .iter()
            .any(|identity| identity.sign_id == sign.sign_id)
        {
            return Err(ExecutionIdentityError::DuplicateIdentity);
        }
        self.signs.push(KernelSignIdentity {
            sign_id: sign.sign_id.clone(),
            node,
            request,
            presentation_id: presentation_id.cloned(),
        });
        Ok(())
    }

    pub fn request(
        &self,
        node: NodeId,
        request: conduit_kernel::RequestId,
    ) -> Option<&KernelHostRequestIdentity> {
        self.requests
            .iter()
            .find(|identity| identity.node == node && identity.request == request)
    }

    pub fn request_for_contract<'a>(
        &'a self,
        node: NodeId,
        contract: &'a HostOperationContractId,
    ) -> impl Iterator<Item = &'a KernelHostRequestIdentity> + 'a {
        self.requests
            .iter()
            .filter(move |identity| identity.node == node && &identity.contract_id == contract)
    }

    pub fn presentation(
        &self,
        presentation: &PresentationId,
    ) -> Option<&KernelPresentationIdentity> {
        self.presentations
            .iter()
            .find(|identity| &identity.presentation_id == presentation)
    }

    pub fn presentation_for_request(
        &self,
        node: NodeId,
        request: conduit_kernel::RequestId,
    ) -> Option<&KernelPresentationIdentity> {
        self.presentations
            .iter()
            .find(|identity| identity.node == node && identity.request == request)
    }

    pub fn sign_identity(&self, sign: &SignId) -> Option<&KernelSignIdentity> {
        self.signs.iter().find(|identity| &identity.sign_id == sign)
    }

    pub fn sign_for_presentation(
        &self,
        presentation: &PresentationId,
    ) -> Option<&KernelSignIdentity> {
        self.signs
            .iter()
            .find(|identity| identity.presentation_id.as_ref() == Some(presentation))
    }

    pub fn allocation_capacities(&self) -> (usize, usize, usize) {
        (
            self.requests.capacity(),
            self.presentations.capacity(),
            self.signs.capacity(),
        )
    }

    pub fn lengths(&self) -> (usize, usize, usize) {
        (
            self.requests.len(),
            self.presentations.len(),
            self.signs.len(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPlanFragment {
    pub identity: KernelIdentityMap,
    pub nodes: Vec<LoweredNode>,
    pub node_specs: Vec<NodeSpec<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>>,
    pub cords: Vec<LoweredCord>,
    pub fusions: Vec<LoweredFusion>,
    pub remote_endpoints: Vec<LoweredRemoteEndpoint>,
    pub routes: Vec<LoweredRoute>,
    pub host_operations: Vec<LoweredHostOperation>,
    pub resources: Vec<LoweredResource>,
    pub signs: Vec<LoweredSign>,
    pub shared_pools: Vec<LoweredSharedPool>,
    pub cord_value_slots: u16,
    pub cord_value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
}

pub fn lower_plan_fragment(fragment: &PlanFragment) -> Result<LoweredPlanFragment, LoweringError> {
    lower_plan_fragment_for_profile(fragment, FIXED_KERNEL_STORAGE_PROFILE)
}

pub fn lower_plan_fragment_for_profile(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<LoweredPlanFragment, LoweringError> {
    admission::validate_fragment(fragment)?;
    let mut placement_nodes = BTreeMap::new();
    let mut nodes = Vec::with_capacity(fragment.placements.len());
    let mut node_specs = Vec::with_capacity(fragment.placements.len());
    let mut identity_ports = Vec::new();
    for (node_index, placement) in fragment.placements.iter().enumerate() {
        let node = NodeId(as_u16(node_index)?);
        if placement_nodes
            .insert(placement.placement_id.clone(), node)
            .is_some()
        {
            return Err(LoweringError::DuplicatePlacement(
                placement.placement_id.clone(),
            ));
        }
        let inputs = lower_ports(
            node,
            &placement.placement_id,
            &placement.inputs,
            PortDirection::Input,
        )?;
        let outputs = lower_ports(
            node,
            &placement.placement_id,
            &placement.outputs,
            PortDirection::Output,
        )?;
        if inputs.len() > profile.maximum_ports_per_node() {
            return Err(LoweringError::ProfileCapacityExceeded {
                placement_id: placement.placement_id.clone(),
                direction: PortDirection::Input,
                required: inputs.len(),
                available: profile.maximum_ports_per_node(),
            });
        }
        if outputs.len() > profile.maximum_ports_per_node() {
            return Err(LoweringError::ProfileCapacityExceeded {
                placement_id: placement.placement_id.clone(),
                direction: PortDirection::Output,
                required: outputs.len(),
                available: profile.maximum_ports_per_node(),
            });
        }
        let input_cords = [None; FIXED_KERNEL_STORAGE_PORTS_PER_NODE];
        let maximum_step_work = 1usize
            .checked_add(inputs.len())
            .and_then(|value| value.checked_add(outputs.len()))
            .and_then(|value| value.checked_add(placement.host_operations.len()))
            .ok_or(LoweringError::CapacityOverflow)
            .and_then(as_u16)?;
        identity_ports.extend(
            inputs
                .iter()
                .chain(outputs.iter())
                .map(|port| KernelPortIdentity {
                    node,
                    direction: port.direction,
                    port: port.port,
                    port_id: port.port_id.clone(),
                }),
        );
        nodes.push(LoweredNode {
            node,
            placement_id: placement.placement_id.clone(),
            maximum_step_work,
            inputs,
            outputs,
        });
        node_specs.push(NodeSpec {
            input_cords,
            maximum_step_work,
        });
    }

    let mut connection_ids = BTreeSet::new();
    let mut cords = Vec::with_capacity(fragment.connections.len());
    let mut remote_endpoints = Vec::new();
    let mut value_slots = 0u16;
    let mut value_bytes = 0u32;
    for (cord_index, connection) in fragment.connections.iter().enumerate() {
        if !connection_ids.insert(connection.connection_id.clone()) {
            return Err(LoweringError::DuplicateConnection(
                connection.connection_id.clone(),
            ));
        }
        if connection.item_capacity == 0 || connection.byte_capacity == 0 {
            return Err(LoweringError::InvalidConnectionBudget(
                connection.connection_id.clone(),
            ));
        }
        let cord = CordId(as_u16(cord_index)?);
        let source_node = placement_nodes
            .get(&connection.source_placement_id)
            .copied();
        let sink_node = placement_nodes.get(&connection.sink_placement_id).copied();
        let source_port = source_node
            .map(|node| {
                find_port(
                    &nodes[usize::from(node.0)].outputs,
                    &connection.source_port_id,
                )
                .ok_or_else(|| {
                    LoweringError::UnknownConnectionPort(connection.connection_id.clone())
                })
            })
            .transpose()?;
        let sink_port = sink_node
            .map(|node| {
                find_port(&nodes[usize::from(node.0)].inputs, &connection.sink_port_id).ok_or_else(
                    || LoweringError::UnknownConnectionPort(connection.connection_id.clone()),
                )
            })
            .transpose()?;
        if source_node.zip(source_port).is_some_and(|(node, port)| {
            let descriptor = &nodes[usize::from(node.0)].outputs[usize::from(port.0)];
            descriptor.value_kind != connection.value_kind
                || descriptor.temporal != connection.temporal
        }) || sink_node.zip(sink_port).is_some_and(|(node, port)| {
            let descriptor = &nodes[usize::from(node.0)].inputs[usize::from(port.0)];
            descriptor.value_kind != connection.value_kind
                || descriptor.temporal != connection.temporal
        }) {
            return Err(LoweringError::ConnectionContractMismatch(
                connection.connection_id.clone(),
            ));
        }
        let slot_start = value_slots;
        value_slots = value_slots
            .checked_add(connection.item_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        value_bytes = value_bytes
            .checked_add(connection.byte_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        if let Some((sink_node, sink_port)) = sink_node.zip(sink_port) {
            let sink_slot =
                &mut node_specs[usize::from(sink_node.0)].input_cords[usize::from(sink_port.0)];
            if sink_slot.is_some() {
                return Err(LoweringError::MultipleConnectionsToInput {
                    placement_id: connection.sink_placement_id.clone(),
                    port_id: connection.sink_port_id.clone(),
                });
            }
            *sink_slot = Some(cord);
        }
        let spec = match (source_node.zip(source_port), sink_node.zip(sink_port)) {
            (Some((source_node, source_port)), Some((sink_node, sink_port))) => {
                if connection.selected_line.is_some() || !connection.admitted_lines.is_empty() {
                    return Err(LoweringError::InvalidRemoteConnection(
                        connection.connection_id.clone(),
                    ));
                }
                CordSpec::local(
                    cord,
                    (source_node, source_port),
                    (sink_node, sink_port),
                    CordCapacity {
                        slot_start,
                        item_capacity: connection.item_capacity,
                        byte_capacity: connection.byte_capacity,
                    },
                )
            }
            (Some((source_node, source_port)), None) => {
                let endpoint = lower_remote_endpoints(
                    fragment,
                    connection,
                    cord,
                    RemoteCordDirection::Egress,
                    &mut remote_endpoints,
                )?;
                CordSpec::remote_egress(
                    cord,
                    (source_node, source_port),
                    endpoint,
                    CordCapacity {
                        slot_start,
                        item_capacity: connection.item_capacity,
                        byte_capacity: connection.byte_capacity,
                    },
                )
            }
            (None, Some((sink_node, sink_port))) => {
                let endpoint = lower_remote_endpoints(
                    fragment,
                    connection,
                    cord,
                    RemoteCordDirection::Ingress,
                    &mut remote_endpoints,
                )?;
                CordSpec::remote_ingress(
                    cord,
                    endpoint,
                    (sink_node, sink_port),
                    CordCapacity {
                        slot_start,
                        item_capacity: connection.item_capacity,
                        byte_capacity: connection.byte_capacity,
                    },
                )
            }
            (None, None) => {
                return Err(LoweringError::UnknownConnectionEndpoint(
                    connection.connection_id.clone(),
                ));
            }
        };
        cords.push(LoweredCord {
            connection_id: connection.connection_id.clone(),
            spec,
        });
    }

    let routes = lower_routes(&cords)?;
    let mut host_operations = Vec::new();
    let mut resources = Vec::new();
    for (placement, node) in fragment.placements.iter().zip(nodes.iter()) {
        for (index, requirement) in placement.host_operations.iter().enumerate() {
            if requirement.maximum_in_flight != 1 {
                return Err(LoweringError::UnsupportedHostOperationConcurrency(
                    placement.placement_id.clone(),
                ));
            }
            let operation = HostOperationId(as_u16(index)?);
            host_operations.push(LoweredHostOperation {
                node: node.node,
                operation,
                contract_id: requirement.contract_id.clone(),
                target_kind: requirement.target_kind.clone(),
                maximum_in_flight: requirement.maximum_in_flight,
                binding: HostOperationBinding {
                    operation,
                    maximum_input_bytes: requirement.maximum_input_bytes,
                    maximum_output_bytes: requirement.maximum_output_bytes,
                },
            });
        }
        for (index, binding) in placement.resources.iter().enumerate() {
            if binding.units == 0 {
                return Err(LoweringError::ResourceBindingInvalid(
                    placement.placement_id.clone(),
                ));
            }
            resources.push(LoweredResource {
                node: node.node,
                binding: KernelResourceBinding {
                    resource: ResourceId(as_u16(index)?),
                    units: binding.units,
                },
            });
        }
    }

    let mut signs = Vec::with_capacity(fragment.expected_sign.len());
    for (index, expected) in fragment.expected_sign.iter().enumerate() {
        let target = match expected {
            ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => {
                SignExpectationTarget::Fragment
            }
            ExpectedSign::PlacementPrepared(id) | ExpectedSign::PlacementTerminal(id) => {
                SignExpectationTarget::Node(
                    *placement_nodes
                        .get(id)
                        .ok_or(LoweringError::SignReferenceMissing)?,
                )
            }
            ExpectedSign::ConnectionTerminal(id) => SignExpectationTarget::Cord(
                cords
                    .iter()
                    .find(|cord| &cord.connection_id == id)
                    .map(|cord| cord.spec.cord)
                    .ok_or(LoweringError::SignReferenceMissing)?,
            ),
        };
        signs.push(LoweredSign {
            expectation: SignExpectationId(as_u16(index)?),
            expected: expected.clone(),
            target,
        });
    }

    let shared_pools = lower_shared_pools(fragment, &placement_nodes)?;
    let fusions = lower_fusions(fragment, &placement_nodes, &cords)?;

    Ok(LoweredPlanFragment {
        identity: KernelIdentityMap {
            plan_id: fragment.plan_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
            placements: nodes
                .iter()
                .map(|node| (node.node, node.placement_id.clone()))
                .collect(),
            ports: identity_ports,
            connections: cords
                .iter()
                .map(|cord| (cord.spec.cord, cord.connection_id.clone()))
                .collect(),
            remote_endpoints: remote_endpoints
                .iter()
                .map(|item| (item.endpoint, item.connection_id.clone()))
                .collect(),
            host_operations: host_operations
                .iter()
                .map(|item| (item.node, item.operation, item.contract_id.clone()))
                .collect(),
            resources: resources
                .iter()
                .zip(
                    fragment
                        .placements
                        .iter()
                        .flat_map(|placement| &placement.resources),
                )
                .map(|(item, binding)| (item.node, item.binding.resource, binding.clone()))
                .collect(),
        },
        nodes,
        node_specs,
        cords,
        fusions,
        remote_endpoints,
        routes,
        host_operations,
        resources,
        signs,
        shared_pools,
        cord_value_slots: value_slots,
        cord_value_bytes: value_bytes,
        sign_items: fragment.sign_storage_budget.item_capacity,
        sign_bytes: fragment.sign_storage_budget.byte_capacity,
    })
}

fn lower_ports(
    node: NodeId,
    placement_id: &PlacementId,
    ports: &[PortDescriptor],
    expected_direction: PortDirection,
) -> Result<Vec<LoweredPort>, LoweringError> {
    let mut ids = BTreeSet::new();
    ports
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            if descriptor.direction != expected_direction {
                return Err(LoweringError::PortDirectionMismatch {
                    placement_id: placement_id.clone(),
                    port_id: descriptor.port_id.clone(),
                });
            }
            if !ids.insert(descriptor.port_id.clone()) {
                return Err(LoweringError::DuplicatePort {
                    placement_id: placement_id.clone(),
                    port_id: descriptor.port_id.clone(),
                });
            }
            Ok(LoweredPort {
                node,
                port: PortId(as_u16(index)?),
                port_id: descriptor.port_id.clone(),
                value_kind: descriptor.value_kind.clone(),
                direction: descriptor.direction,
                temporal: descriptor.temporal,
            })
        })
        .collect()
}

fn fragment_id_for_host(
    fragment: &PlanFragment,
    host_id: &HostId,
) -> Result<FragmentId, LoweringError> {
    fragment
        .plan_fragments
        .iter()
        .find(|commitment| &commitment.host_id == host_id)
        .map(|commitment| commitment.fragment_id.clone())
        .ok_or(LoweringError::InvalidFragment)
}

fn find_port(ports: &[LoweredPort], id: &PlanPortId) -> Option<PortId> {
    ports
        .iter()
        .find(|port| &port.port_id == id)
        .map(|port| port.port)
}

fn lower_routes(cords: &[LoweredCord]) -> Result<Vec<LoweredRoute>, LoweringError> {
    let mut grouped = BTreeMap::<(NodeId, PortId), Vec<RouteTarget>>::new();
    for cord in cords {
        if let Some((source_node, source_port)) = cord.spec.source_local() {
            grouped
                .entry((source_node, source_port))
                .or_default()
                .push(RouteTarget {
                    cord: cord.spec.cord,
                    sink: cord.spec.sink,
                });
        }
    }
    let mut next_target = 0u16;
    grouped
        .into_iter()
        .map(|((source_node, source_port), targets)| {
            let len = as_u16(targets.len())?;
            let range = RouteRange {
                start: next_target,
                len,
            };
            next_target = next_target
                .checked_add(len)
                .ok_or(LoweringError::CapacityOverflow)?;
            Ok(LoweredRoute {
                source_node,
                source_port,
                range,
                targets,
            })
        })
        .collect()
}

fn as_u16(value: usize) -> Result<u16, LoweringError> {
    u16::try_from(value).map_err(|_| LoweringError::CapacityOverflow)
}
