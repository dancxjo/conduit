use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOperation {
    pub placement_id: PlacementId,
    pub operation_id: OperationId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub execution_profile_id: ExecutionProfileId,
    pub configuration: Vec<ConfigurationEntry>,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub host_operations: Vec<HostOperationRequirement>,
    pub resources: Vec<ResourceBinding>,
    pub authority: Vec<AuthorityBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedTerminal {
    PlacementCompleted(PlacementId),
    ConnectionCompleted(ConnectionId),
    PlanCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedEvidence {
    PlanFragmentReceived,
    PlacementPrepared(PlacementId),
    PlacementTerminal(PlacementId),
    ConnectionTerminal(ConnectionId),
    PlanTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StartupDependency {
    pub prerequisite_placement_id: PlacementId,
    pub dependent_placement_id: PlacementId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationPolicy {
    CancelAllAndRejectLateCompletion,
    DrainBeforeCancel,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalPolicy {
    RequireAllPlacementsAndConnections,
    RequirePlacementsOnly,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceStorageBudget {
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MandatoryEvidenceReport {
    pub plan_id: PlanId,
    pub expected: Vec<ExpectedEvidence>,
    pub recorded: Vec<ExpectedEvidence>,
    pub storage_budget: EvidenceStorageBudget,
    pub allocated_item_slots: u32,
    pub used_bytes: u32,
    pub overflowed: bool,
}

pub fn mandatory_evidence_storage_requirement(
    evidence: &[ExpectedEvidence],
) -> Option<EvidenceStorageBudget> {
    let item_capacity = u16::try_from(evidence.len()).ok()?;
    let mut byte_capacity = 0u32;
    for item in evidence {
        let identity = match item {
            ExpectedEvidence::PlanFragmentReceived | ExpectedEvidence::PlanTerminal => None,
            ExpectedEvidence::PlacementPrepared(placement_id)
            | ExpectedEvidence::PlacementTerminal(placement_id) => Some(placement_id.as_str()),
            ExpectedEvidence::ConnectionTerminal(connection_id) => Some(connection_id.as_str()),
        };
        let identity_bytes = match identity {
            Some(value) => u32::try_from(value.len()).ok()?,
            None => 0,
        };
        byte_capacity = byte_capacity.checked_add(1)?.checked_add(identity_bytes)?;
    }
    Some(EvidenceStorageBudget {
        item_capacity,
        byte_capacity,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FragmentCommitment {
    pub host_id: HostId,
    pub fragment_id: FragmentId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedConnection {
    pub connection_id: ConnectionId,
    pub source_placement_id: PlacementId,
    pub source_port_id: PortId,
    pub sink_placement_id: PlacementId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
    pub provider: ConnectionProvider,
    pub link_binding: Option<LinkBinding>,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanFragment {
    pub plan_id: PlanId,
    pub fragment_id: FragmentId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub placements: Vec<PlannedOperation>,
    pub connections: Vec<PlannedConnection>,
    pub startup_dependencies: Vec<StartupDependency>,
    pub startup_order: Vec<PlacementId>,
    pub cancellation_policy: CancellationPolicy,
    pub terminal_policy: TerminalPolicy,
    pub expected_terminals: Vec<ExpectedTerminal>,
    pub expected_evidence: Vec<ExpectedEvidence>,
    pub evidence_storage_budget: EvidenceStorageBudget,
    pub plan_fragments: Vec<FragmentCommitment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub plan_id: PlanId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub fragments: Vec<PlanFragment>,
}

pub fn seal_plan(form_identity: FormIdentity, mut fragments: Vec<PlanFragment>) -> Plan {
    for fragment in &mut fragments {
        fragment.plan_id = PlanId::from("");
        fragment.source_document_id = form_identity.source_document_id.clone();
        fragment.checked_form_id = form_identity.checked_form_id.clone();
        fragment.expanded_form_id = form_identity.expanded_form_id.clone();
        fragment.fragment_id = compute_fragment_id(fragment);
        fragment.plan_fragments.clear();
    }
    let mut commitments = fragments
        .iter()
        .map(|fragment| FragmentCommitment {
            host_id: fragment.host_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
        })
        .collect::<Vec<_>>();
    commitments.sort();
    let plan_id = compute_plan_id(&form_identity, &commitments);
    for fragment in &mut fragments {
        fragment.plan_id = plan_id.clone();
        fragment.plan_fragments = commitments.clone();
    }
    Plan {
        plan_id,
        source_document_id: form_identity.source_document_id,
        checked_form_id: form_identity.checked_form_id,
        expanded_form_id: form_identity.expanded_form_id,
        fragments,
    }
}

pub fn verify_plan(plan: &Plan) -> bool {
    plan.fragments.iter().all(verify_plan_fragment)
        && plan.fragments.iter().all(|fragment| {
            fragment.plan_id == plan.plan_id
                && fragment.source_document_id == plan.source_document_id
                && fragment.checked_form_id == plan.checked_form_id
                && fragment.expanded_form_id == plan.expanded_form_id
        })
        && plan
            .fragments
            .first()
            .is_none_or(|first| first.plan_fragments.len() == plan.fragments.len())
        && verify_plan_connections(plan)
}

fn verify_plan_connections(plan: &Plan) -> bool {
    let connections = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .collect::<Vec<_>>();
    for (index, connection) in connections.iter().enumerate() {
        if connections[..index]
            .iter()
            .any(|prior| prior.connection_id == connection.connection_id)
        {
            continue;
        }
        let occurrences = connections
            .iter()
            .filter(|candidate| candidate.connection_id == connection.connection_id)
            .collect::<Vec<_>>();
        if occurrences.iter().any(|candidate| *candidate != connection) {
            return false;
        }
        let source = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|placement| placement.placement_id == connection.source_placement_id)
            .collect::<Vec<_>>();
        let sink = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .filter(|placement| placement.placement_id == connection.sink_placement_id)
            .collect::<Vec<_>>();
        if source.len() != 1 || sink.len() != 1 {
            return false;
        }
        let source = source[0];
        let sink = sink[0];
        if source.host_id == sink.host_id {
            if occurrences.len() != 1
                || connection.provider != ConnectionProvider::Local
                || connection.link_binding.is_some()
            {
                return false;
            }
        } else {
            let Some(binding) = &connection.link_binding else {
                return false;
            };
            if occurrences.len() != 2
                || connection.provider == ConnectionProvider::Local
                || binding.binding_id.as_str().is_empty()
                || binding.provider != connection.provider
                || binding.provider_instance_id.as_str().is_empty()
                || binding.availability != LinkAvailability::Ready
                || binding.source.host_id != source.host_id
                || binding.source.boot_id != source.boot_id
                || binding.source.endpoint_id.as_str().is_empty()
                || binding.sink.host_id != sink.host_id
                || binding.sink.boot_id != sink.boot_id
                || binding.sink.endpoint_id.as_str().is_empty()
                || binding.source.endpoint_id == binding.sink.endpoint_id
                || binding.limits.maximum_in_flight_items < connection.item_capacity
                || binding.limits.maximum_payload_bytes < connection.byte_capacity
                || binding.limits.maximum_buffered_bytes < connection.byte_capacity
                || binding.limits.maximum_frame_bytes < binding.limits.maximum_payload_bytes
                || matches!(
                    &binding.credential,
                    LinkCredentialReference::Opaque(reference) if reference.as_str().is_empty()
                )
                || matches!(
                    &binding.authority,
                    LinkAuthorityReference::Grant(grant_id) if grant_id.as_str().is_empty()
                )
            {
                return false;
            }
        }
    }
    true
}

pub fn verify_plan_fragment(fragment: &PlanFragment) -> bool {
    if compute_fragment_id(fragment) != fragment.fragment_id {
        return false;
    }
    let mut commitments = fragment.plan_fragments.clone();
    commitments.sort();
    if commitments != fragment.plan_fragments
        || commitments
            .windows(2)
            .any(|pair| pair[0].host_id == pair[1].host_id)
    {
        return false;
    }
    let own_matches = commitments
        .iter()
        .filter(|item| item.host_id == fragment.host_id && item.fragment_id == fragment.fragment_id)
        .count();
    own_matches == 1
        && compute_plan_id(
            &FormIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_form_id: fragment.checked_form_id.clone(),
                expanded_form_id: fragment.expanded_form_id.clone(),
            },
            &commitments,
        ) == fragment.plan_id
}

pub fn compute_fragment_id(fragment: &PlanFragment) -> FragmentId {
    let mut canonical = Vec::new();
    push_string(&mut canonical, fragment.source_document_id.as_str());
    push_string(&mut canonical, fragment.checked_form_id.as_str());
    push_string(&mut canonical, fragment.expanded_form_id.as_str());
    push_string(&mut canonical, fragment.host_id.as_str());
    push_string(&mut canonical, fragment.boot_id.as_str());
    push_u64(&mut canonical, fragment.offer_generation.0);
    push_u32(&mut canonical, fragment.placements.len() as u32);
    for operation in &fragment.placements {
        push_string(&mut canonical, operation.placement_id.as_str());
        push_string(&mut canonical, operation.operation_id.as_str());
        push_string(&mut canonical, operation.kind_id.as_str());
        push_string(&mut canonical, operation.kind_contract_revision.as_str());
        push_string(&mut canonical, operation.execution_profile_id.as_str());
        push_u32(&mut canonical, operation.configuration.len() as u32);
        for entry in &operation.configuration {
            push_string(&mut canonical, &entry.key);
            match entry.value {
                ConfigurationValue::Bool(value) => {
                    canonical.push(0);
                    canonical.push(u8::from(value));
                }
                ConfigurationValue::U64(value) => {
                    canonical.push(1);
                    push_u64(&mut canonical, value);
                }
            }
        }
        push_string(&mut canonical, operation.host_id.as_str());
        push_string(&mut canonical, operation.boot_id.as_str());
        push_u64(&mut canonical, operation.offer_generation.0);
        push_string(&mut canonical, operation.capability_id.as_str());
        push_string(&mut canonical, operation.implementation_id.as_str());
        push_string(&mut canonical, operation.artifact_id.as_str());
        push_ports(&mut canonical, &operation.inputs);
        push_ports(&mut canonical, &operation.outputs);
        push_u32(&mut canonical, operation.host_operations.len() as u32);
        for requirement in &operation.host_operations {
            push_string(&mut canonical, requirement.contract_id.as_str());
            match &requirement.target_kind {
                Some(target_kind) => {
                    canonical.push(1);
                    push_string(&mut canonical, target_kind.as_str());
                }
                None => canonical.push(0),
            }
            canonical.extend_from_slice(&requirement.maximum_in_flight.to_le_bytes());
            push_u32(&mut canonical, requirement.maximum_input_bytes);
            push_u32(&mut canonical, requirement.maximum_output_bytes);
        }
        push_u32(&mut canonical, operation.resources.len() as u32);
        for binding in &operation.resources {
            push_string(&mut canonical, binding.pool_id.as_str());
            push_string(&mut canonical, binding.class_id.as_str());
            push_u32(&mut canonical, binding.units);
        }
        push_u32(&mut canonical, operation.authority.len() as u32);
        for binding in &operation.authority {
            push_string(&mut canonical, binding.grant_id.as_str());
            push_string(&mut canonical, binding.contract_id.as_str());
            push_string(&mut canonical, binding.host_operation_contract_id.as_str());
            push_string(&mut canonical, binding.subject_kind.as_str());
            push_string(&mut canonical, binding.host_id.as_str());
            push_string(&mut canonical, binding.boot_id.as_str());
            push_string(&mut canonical, binding.capability_id.as_str());
        }
    }
    push_u32(&mut canonical, fragment.connections.len() as u32);
    for connection in &fragment.connections {
        push_string(&mut canonical, connection.connection_id.as_str());
        push_string(&mut canonical, connection.source_placement_id.as_str());
        push_string(&mut canonical, connection.source_port_id.as_str());
        push_string(&mut canonical, connection.sink_placement_id.as_str());
        push_string(&mut canonical, connection.sink_port_id.as_str());
        push_string(&mut canonical, connection.value_kind.as_str());
        canonical.push(match connection.provider {
            ConnectionProvider::Local => 0,
            ConnectionProvider::InMemory => 1,
            ConnectionProvider::FixtureFrame => 2,
            ConnectionProvider::FixtureDatagram => 3,
            ConnectionProvider::WebSocket => 4,
        });
        match &connection.link_binding {
            Some(binding) => {
                canonical.push(1);
                push_string(&mut canonical, binding.binding_id.as_str());
                push_string(&mut canonical, binding.source.host_id.as_str());
                push_string(&mut canonical, binding.source.boot_id.as_str());
                push_string(&mut canonical, binding.source.endpoint_id.as_str());
                push_string(&mut canonical, binding.sink.host_id.as_str());
                push_string(&mut canonical, binding.sink.boot_id.as_str());
                push_string(&mut canonical, binding.sink.endpoint_id.as_str());
                canonical.push(match binding.provider {
                    ConnectionProvider::Local => 0,
                    ConnectionProvider::InMemory => 1,
                    ConnectionProvider::FixtureFrame => 2,
                    ConnectionProvider::FixtureDatagram => 3,
                    ConnectionProvider::WebSocket => 4,
                });
                push_string(&mut canonical, binding.provider_instance_id.as_str());
                canonical.push(match binding.availability {
                    LinkAvailability::Ready => 0,
                    LinkAvailability::Unavailable => 1,
                });
                match &binding.credential {
                    LinkCredentialReference::None => canonical.push(0),
                    LinkCredentialReference::Opaque(reference) => {
                        canonical.push(1);
                        push_string(&mut canonical, reference.as_str());
                    }
                }
                match &binding.authority {
                    LinkAuthorityReference::ProcessOwned => canonical.push(0),
                    LinkAuthorityReference::Grant(grant_id) => {
                        canonical.push(1);
                        push_string(&mut canonical, grant_id.as_str());
                    }
                }
                canonical.extend_from_slice(&binding.limits.maximum_in_flight_items.to_le_bytes());
                push_u32(&mut canonical, binding.limits.maximum_payload_bytes);
                push_u32(&mut canonical, binding.limits.maximum_buffered_bytes);
                push_u32(&mut canonical, binding.limits.maximum_frame_bytes);
            }
            None => canonical.push(0),
        }
        canonical.extend_from_slice(&connection.item_capacity.to_le_bytes());
        push_u32(&mut canonical, connection.byte_capacity);
    }
    push_u32(&mut canonical, fragment.startup_dependencies.len() as u32);
    for dependency in &fragment.startup_dependencies {
        push_string(
            &mut canonical,
            dependency.prerequisite_placement_id.as_str(),
        );
        push_string(&mut canonical, dependency.dependent_placement_id.as_str());
    }
    push_u32(&mut canonical, fragment.startup_order.len() as u32);
    for placement_id in &fragment.startup_order {
        push_string(&mut canonical, placement_id.as_str());
    }
    canonical.push(match fragment.cancellation_policy {
        CancellationPolicy::CancelAllAndRejectLateCompletion => 0,
        CancellationPolicy::DrainBeforeCancel => 1,
    });
    canonical.push(match fragment.terminal_policy {
        TerminalPolicy::RequireAllPlacementsAndConnections => 0,
        TerminalPolicy::RequirePlacementsOnly => 1,
    });
    push_u32(&mut canonical, fragment.expected_terminals.len() as u32);
    for terminal in &fragment.expected_terminals {
        match terminal {
            ExpectedTerminal::PlacementCompleted(placement_id) => {
                canonical.push(0);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedTerminal::ConnectionCompleted(connection_id) => {
                canonical.push(1);
                push_string(&mut canonical, connection_id.as_str());
            }
            ExpectedTerminal::PlanCompleted => canonical.push(2),
        }
    }
    push_u32(&mut canonical, fragment.expected_evidence.len() as u32);
    for evidence in &fragment.expected_evidence {
        match evidence {
            ExpectedEvidence::PlanFragmentReceived => canonical.push(0),
            ExpectedEvidence::PlacementPrepared(placement_id) => {
                canonical.push(1);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedEvidence::PlacementTerminal(placement_id) => {
                canonical.push(2);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedEvidence::ConnectionTerminal(connection_id) => {
                canonical.push(3);
                push_string(&mut canonical, connection_id.as_str());
            }
            ExpectedEvidence::PlanTerminal => canonical.push(4),
        }
    }
    canonical.extend_from_slice(&fragment.evidence_storage_budget.item_capacity.to_le_bytes());
    push_u32(
        &mut canonical,
        fragment.evidence_storage_budget.byte_capacity,
    );
    FragmentId::from(hash_bytes(&canonical))
}
