//! Exact pre-activation lowering from string-identified plan facts into the
//! numeric tables consumed by `conduit-kernel`.

use conduit_core::{
    mandatory_evidence_storage_requirement, verify_plan_fragment, ConnectionId, ConnectionProvider,
    ExpectedEvidence, FragmentId, HostOperationContractId, KindId, PlacementId, PlanFragment,
    PlanId, PortDescriptor, PortDirection, PortId as PlanPortId,
    ResourceBinding as PlanResourceBinding,
};
use conduit_kernel::{
    scheduler::{CordSpec, NodeSpec},
    CordId, EvidenceExpectationId, EvidenceExpectationTarget, HostOperationBinding,
    HostOperationId, NodeId, PortId, ResourceBinding as KernelResourceBinding, ResourceId,
    RouteRange, RouteTarget,
};
use std::collections::{BTreeMap, BTreeSet};

/// The first takeover checkpoint deliberately admits only the scheduler's
/// fixed per-node port width. Wider plans must be rejected before activation.
pub const MAXIMUM_KERNEL_PORTS_PER_NODE: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    InvalidFragment,
    EmptyFragment,
    CapacityOverflow,
    DuplicatePlacement(PlacementId),
    DuplicateConnection(ConnectionId),
    DuplicatePort {
        placement_id: PlacementId,
        port_id: PlanPortId,
    },
    UnknownConnectionEndpoint(ConnectionId),
    UnknownConnectionPort(ConnectionId),
    ConnectionValueKindMismatch(ConnectionId),
    InvalidConnectionBudget(ConnectionId),
    UnsupportedRemoteConnection(ConnectionId),
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
    EvidenceBudgetInvalid,
    EvidenceReferenceMissing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPort {
    pub node: NodeId,
    pub port: PortId,
    pub port_id: PlanPortId,
    pub value_kind: KindId,
    pub direction: PortDirection,
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
pub struct LoweredEvidence {
    pub expectation: EvidenceExpectationId,
    pub expected: ExpectedEvidence,
    pub target: EvidenceExpectationTarget,
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
    pub host_operations: Vec<(NodeId, HostOperationId, HostOperationContractId)>,
    pub resources: Vec<(NodeId, ResourceId, PlanResourceBinding)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPlanFragment {
    pub identity: KernelIdentityMap,
    pub nodes: Vec<LoweredNode>,
    pub node_specs: Vec<NodeSpec<MAXIMUM_KERNEL_PORTS_PER_NODE>>,
    pub cords: Vec<LoweredCord>,
    pub routes: Vec<LoweredRoute>,
    pub host_operations: Vec<LoweredHostOperation>,
    pub resources: Vec<LoweredResource>,
    pub evidence: Vec<LoweredEvidence>,
    pub value_slots: u16,
    pub value_bytes: u32,
    pub evidence_items: u16,
    pub evidence_bytes: u32,
}

pub fn lower_plan_fragment(fragment: &PlanFragment) -> Result<LoweredPlanFragment, LoweringError> {
    if !verify_plan_fragment(fragment) {
        return Err(LoweringError::InvalidFragment);
    }
    if fragment.placements.is_empty() {
        return Err(LoweringError::EmptyFragment);
    }
    if mandatory_evidence_storage_requirement(&fragment.expected_evidence)
        != Some(fragment.evidence_storage_budget)
    {
        return Err(LoweringError::EvidenceBudgetInvalid);
    }
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
        let input_cords = [None; MAXIMUM_KERNEL_PORTS_PER_NODE];
        if inputs.len() > input_cords.len() || outputs.len() > MAXIMUM_KERNEL_PORTS_PER_NODE {
            return Err(LoweringError::CapacityOverflow);
        }
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
    let mut value_slots = 0u16;
    let mut value_bytes = 0u32;
    for (cord_index, connection) in fragment.connections.iter().enumerate() {
        if !connection_ids.insert(connection.connection_id.clone()) {
            return Err(LoweringError::DuplicateConnection(
                connection.connection_id.clone(),
            ));
        }
        if connection.provider != ConnectionProvider::Local {
            return Err(LoweringError::UnsupportedRemoteConnection(
                connection.connection_id.clone(),
            ));
        }
        if connection.item_capacity == 0 || connection.byte_capacity == 0 {
            return Err(LoweringError::InvalidConnectionBudget(
                connection.connection_id.clone(),
            ));
        }
        let cord = CordId(as_u16(cord_index)?);
        let source_node = *placement_nodes
            .get(&connection.source_placement_id)
            .ok_or_else(|| {
                LoweringError::UnknownConnectionEndpoint(connection.connection_id.clone())
            })?;
        let sink_node = *placement_nodes
            .get(&connection.sink_placement_id)
            .ok_or_else(|| {
                LoweringError::UnknownConnectionEndpoint(connection.connection_id.clone())
            })?;
        let source_port = find_port(
            &nodes[usize::from(source_node.0)].outputs,
            &connection.source_port_id,
        )
        .ok_or_else(|| LoweringError::UnknownConnectionPort(connection.connection_id.clone()))?;
        let sink_port = find_port(
            &nodes[usize::from(sink_node.0)].inputs,
            &connection.sink_port_id,
        )
        .ok_or_else(|| LoweringError::UnknownConnectionPort(connection.connection_id.clone()))?;
        if nodes[usize::from(source_node.0)].outputs[usize::from(source_port.0)].value_kind
            != connection.value_kind
            || nodes[usize::from(sink_node.0)].inputs[usize::from(sink_port.0)].value_kind
                != connection.value_kind
        {
            return Err(LoweringError::ConnectionValueKindMismatch(
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
        let sink_slot =
            &mut node_specs[usize::from(sink_node.0)].input_cords[usize::from(sink_port.0)];
        if sink_slot.is_some() {
            return Err(LoweringError::MultipleConnectionsToInput {
                placement_id: connection.sink_placement_id.clone(),
                port_id: connection.sink_port_id.clone(),
            });
        }
        *sink_slot = Some(cord);
        cords.push(LoweredCord {
            connection_id: connection.connection_id.clone(),
            spec: CordSpec {
                cord,
                source_node,
                source_port,
                sink_node,
                sink_port,
                slot_start,
                item_capacity: connection.item_capacity,
                byte_capacity: connection.byte_capacity,
            },
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

    let mut evidence = Vec::with_capacity(fragment.expected_evidence.len());
    for (index, expected) in fragment.expected_evidence.iter().enumerate() {
        let target = match expected {
            ExpectedEvidence::PlanFragmentReceived | ExpectedEvidence::PlanTerminal => {
                EvidenceExpectationTarget::Fragment
            }
            ExpectedEvidence::PlacementPrepared(id) | ExpectedEvidence::PlacementTerminal(id) => {
                EvidenceExpectationTarget::Node(
                    *placement_nodes
                        .get(id)
                        .ok_or(LoweringError::EvidenceReferenceMissing)?,
                )
            }
            ExpectedEvidence::ConnectionTerminal(id) => EvidenceExpectationTarget::Cord(
                cords
                    .iter()
                    .find(|cord| &cord.connection_id == id)
                    .map(|cord| cord.spec.cord)
                    .ok_or(LoweringError::EvidenceReferenceMissing)?,
            ),
        };
        evidence.push(LoweredEvidence {
            expectation: EvidenceExpectationId(as_u16(index)?),
            expected: expected.clone(),
            target,
        });
    }

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
        routes,
        host_operations,
        resources,
        evidence,
        value_slots,
        value_bytes,
        evidence_items: fragment.evidence_storage_budget.item_capacity,
        evidence_bytes: fragment.evidence_storage_budget.byte_capacity,
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
            })
        })
        .collect()
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
        grouped
            .entry((cord.spec.source_node, cord.spec.source_port))
            .or_default()
            .push(RouteTarget {
                cord: cord.spec.cord,
                sink_node: cord.spec.sink_node,
                sink_port: cord.spec.sink_port,
            });
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
