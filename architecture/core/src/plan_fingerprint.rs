//! Canonical fingerprints for immutable Plan fragments and their commitment set.
//!
//! This module encodes admitted truth. It neither selects realizations nor
//! executes or mutates Plans.

use crate::{
    characteristic, execution_fusion, hash_bytes, plan_realization, push_resource_binding,
    push_string, push_u32, push_u64, AdmittedLine, BoundLink, CancellationPolicy, CheckedFace,
    ConfigurationValue, ExpectedSign, ExpectedTerminal, FormIdentity, FragmentCommitment,
    FragmentId, LinkAuthorityReference, LinkCredentialReference, PlanFragment, PlanId,
    PortDescriptor, PortDirection, PortTemporal, RealizationBack, TerminalPolicy,
};
use alloc::vec::Vec;

pub fn compute_fragment_id(fragment: &PlanFragment) -> FragmentId {
    let mut canonical = Vec::new();
    push_string(&mut canonical, fragment.source_document_id.as_str());
    push_string(&mut canonical, fragment.checked_form_id.as_str());
    push_string(&mut canonical, fragment.expanded_form_id.as_str());
    if !fragment.realization_backs.is_empty() {
        plan_realization::push_canonical(&mut canonical, &fragment.realization_backs);
    }
    push_string(&mut canonical, fragment.host_id.as_str());
    push_string(&mut canonical, fragment.boot_id.as_str());
    push_u64(&mut canonical, fragment.offer_generation.0);
    push_u32(&mut canonical, fragment.execution_regions.len() as u32);
    for region in &fragment.execution_regions {
        push_string(&mut canonical, region.region_id.as_str());
        push_u32(&mut canonical, region.admitted_placements.len() as u32);
        for placement in &region.admitted_placements {
            push_string(&mut canonical, placement.as_str());
        }
        push_string(&mut canonical, region.execution_profile_id.as_str());
        canonical.push(region.scheduling as u8);
        push_u32(&mut canonical, region.lane_count);
        push_resource_binding(&mut canonical, &region.lane_resource);
        push_string(&mut canonical, region.lane_base_id.as_str());
        push_u32(&mut canonical, region.requirements.runtime_memory_bytes);
        push_u32(&mut canonical, region.requirements.timer_slots);
        push_u32(&mut canonical, region.requirements.cord_item_capacity);
        push_u32(&mut canonical, region.requirements.cord_byte_capacity);
        push_u32(
            &mut canonical,
            u32::from(region.requirements.mandatory_sign_items),
        );
        push_u32(&mut canonical, region.requirements.mandatory_sign_bytes);
        canonical.push(u8::from(region.preemption_required));
        canonical.push(u8::from(region.isolation_required));
    }
    execution_fusion::push_canonical(&mut canonical, &fragment.execution_fusions);
    push_u32(&mut canonical, fragment.placements.len() as u32);
    for gear in &fragment.placements {
        push_string(&mut canonical, gear.placement_id.as_str());
        push_string(&mut canonical, gear.gear_id.as_str());
        push_string(&mut canonical, gear.kind_id.as_str());
        push_string(&mut canonical, gear.kind_contract_revision.as_str());
        push_string(&mut canonical, gear.execution_profile_id.as_str());
        push_u32(&mut canonical, gear.configuration.len() as u32);
        for entry in &gear.configuration {
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
                ConfigurationValue::I64(value) => {
                    canonical.push(3);
                    canonical.extend_from_slice(&value.to_le_bytes());
                }
                ConfigurationValue::Text(ref value) => {
                    canonical.push(2);
                    push_string(&mut canonical, value);
                }
                ConfigurationValue::Structured(ref value) => {
                    canonical.push(4);
                    push_string(&mut canonical, value.profile().as_str());
                    push_u32(&mut canonical, value.canonical_value().len() as u32);
                    canonical.extend_from_slice(value.canonical_value());
                }
            }
        }
        push_string(&mut canonical, gear.host_id.as_str());
        push_string(&mut canonical, gear.boot_id.as_str());
        push_u64(&mut canonical, gear.offer_generation.0);
        push_string(&mut canonical, gear.capability_id.as_str());
        push_string(&mut canonical, gear.implementation_id.as_str());
        push_string(&mut canonical, gear.artifact_id.as_str());
        push_u32(
            &mut canonical,
            gear.realization_characteristics.len() as u32,
        );
        for characteristic in &gear.realization_characteristics {
            characteristic::push_characteristic_canonical(&mut canonical, characteristic);
        }
        canonical.extend_from_slice(&gear.limits.max_active_instances.to_le_bytes());
        canonical.extend_from_slice(&gear.limits.max_queue_items.to_le_bytes());
        push_u32(&mut canonical, gear.limits.max_queue_bytes);
        push_ports(&mut canonical, &gear.inputs);
        push_ports(&mut canonical, &gear.outputs);
        push_u32(&mut canonical, gear.host_operations.len() as u32);
        for requirement in &gear.host_operations {
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
        push_u32(&mut canonical, gear.resources.len() as u32);
        for binding in &gear.resources {
            push_resource_binding(&mut canonical, binding);
        }
        push_u32(&mut canonical, gear.authority.len() as u32);
        for binding in &gear.authority {
            push_string(&mut canonical, binding.grant_id.as_str());
            push_string(&mut canonical, binding.contract_id.as_str());
            push_string(&mut canonical, binding.host_operation_contract_id.as_str());
            push_string(&mut canonical, binding.subject_kind.as_str());
            push_string(&mut canonical, binding.host_id.as_str());
            push_string(&mut canonical, binding.boot_id.as_str());
            push_string(&mut canonical, binding.capability_id.as_str());
        }
        push_u32(&mut canonical, gear.pool_references.len() as u32);
        for pool in &gear.pool_references {
            push_string(&mut canonical, pool.as_str());
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
        canonical.push(match connection.temporal {
            PortTemporal::Value => 0,
            PortTemporal::Flow { closes: false } => 1,
            PortTemporal::Flow { closes: true } => 2,
            PortTemporal::Current => 3,
        });
        match &connection.selected_line {
            Some(line) => {
                canonical.push(1);
                push_admitted_line(&mut canonical, line);
            }
            None => canonical.push(0),
        }
        push_u32(&mut canonical, connection.admitted_lines.len() as u32);
        for candidate in &connection.admitted_lines {
            push_admitted_line(&mut canonical, candidate);
        }
        canonical.extend_from_slice(&connection.item_capacity.to_le_bytes());
        push_u32(&mut canonical, connection.byte_capacity);
    }
    push_u32(&mut canonical, fragment.shared_pools.len() as u32);
    for pool in &fragment.shared_pools {
        push_string(&mut canonical, pool.pool_id.as_str());
        push_string(&mut canonical, pool.declaration_id.as_str());
        push_checked_face(&mut canonical, &pool.member_face);
        canonical.extend_from_slice(&pool.maximum_members.to_le_bytes());
        canonical.extend_from_slice(&pool.member_limits.queue_item_capacity.to_le_bytes());
        push_u32(&mut canonical, pool.member_limits.queue_byte_capacity);
        canonical.extend_from_slice(&pool.member_limits.sign_item_capacity.to_le_bytes());
        push_u32(&mut canonical, pool.member_limits.sign_byte_capacity);
        push_u32(&mut canonical, pool.realization_envelope.len() as u32);
        for realization in &pool.realization_envelope {
            push_string(&mut canonical, realization.host_id.as_str());
            push_string(&mut canonical, realization.boot_id.as_str());
            push_string(&mut canonical, realization.capability_id.as_str());
            canonical.extend_from_slice(&realization.member_capacity.to_le_bytes());
            push_u32(&mut canonical, realization.resources.len() as u32);
            for resource in &realization.resources {
                push_string(&mut canonical, resource.pool_id.as_str());
                push_string(&mut canonical, resource.class_id.as_str());
                push_u32(&mut canonical, resource.units);
            }
        }
        push_string(&mut canonical, pool.admission_authority.as_str());
        push_u32(&mut canonical, pool.consumers.len() as u32);
        for consumer in &pool.consumers {
            push_string(&mut canonical, consumer.as_str());
        }
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
    push_u32(&mut canonical, fragment.expected_sign.len() as u32);
    for sign in &fragment.expected_sign {
        match sign {
            ExpectedSign::PlanFragmentReceived => canonical.push(0),
            ExpectedSign::PlacementPrepared(placement_id) => {
                canonical.push(1);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedSign::PlacementTerminal(placement_id) => {
                canonical.push(2);
                push_string(&mut canonical, placement_id.as_str());
            }
            ExpectedSign::ConnectionTerminal(connection_id) => {
                canonical.push(3);
                push_string(&mut canonical, connection_id.as_str());
            }
            ExpectedSign::PlanTerminal => canonical.push(4),
        }
    }
    canonical.extend_from_slice(&fragment.sign_storage_budget.item_capacity.to_le_bytes());
    push_u32(&mut canonical, fragment.sign_storage_budget.byte_capacity);
    FragmentId::from(hash_bytes(&canonical))
}

fn push_checked_face(canonical: &mut Vec<u8>, face: &CheckedFace) {
    push_u32(canonical, face.startup_parameters().len() as u32);
    for parameter in face.startup_parameters() {
        push_string(canonical, &parameter.name);
        push_string(canonical, &parameter.value_type);
        canonical.push(u8::from(parameter.has_default));
    }
    push_ports(canonical, face.inputs());
    push_ports(canonical, face.outputs());
    match face.shorthand() {
        Some((input, output)) => {
            canonical.push(1);
            push_string(canonical, input.as_str());
            push_string(canonical, output.as_str());
        }
        None => canonical.push(0),
    }
}

fn push_bound_link(canonical: &mut Vec<u8>, binding: &BoundLink) {
    push_string(canonical, binding.binding_id.as_str());
    push_string(canonical, binding.source.host_id.as_str());
    push_string(canonical, binding.source.boot_id.as_str());
    push_string(canonical, binding.source.endpoint_id.as_str());
    push_string(canonical, binding.sink.host_id.as_str());
    push_string(canonical, binding.sink.boot_id.as_str());
    push_string(canonical, binding.sink.endpoint_id.as_str());
    push_string(canonical, binding.base.as_str());
    push_string(canonical, binding.base_instance_id.as_str());
    match &binding.credential {
        LinkCredentialReference::None => canonical.push(0),
        LinkCredentialReference::Opaque(reference) => {
            canonical.push(1);
            push_string(canonical, reference.as_str());
        }
    }
    match &binding.authority {
        LinkAuthorityReference::ProcessOwned => canonical.push(0),
        LinkAuthorityReference::Grant(grant_id) => {
            canonical.push(1);
            push_string(canonical, grant_id.as_str());
        }
    }
    canonical.extend_from_slice(&binding.limits.maximum_in_flight_items.to_le_bytes());
    push_u32(canonical, binding.limits.maximum_payload_bytes);
    push_u32(canonical, binding.limits.maximum_buffered_bytes);
    push_u32(canonical, binding.limits.maximum_frame_bytes);
}

fn push_admitted_line(canonical: &mut Vec<u8>, line: &AdmittedLine) {
    push_string(canonical, line.line_id.as_str());
    push_bound_link(canonical, &line.binding);
    canonical.push(line.contract.scope as u8);
    canonical.push(line.contract.traffic_shape as u8);
    canonical.push(line.contract.duplex as u8);
    canonical.push(line.contract.ordering as u8);
    canonical.push(line.contract.reliability as u8);
    canonical.push(line.contract.continuation as u8);
    canonical.push(line.contract.security as u8);
}

pub(crate) fn compute_plan_id(
    form_identity: &FormIdentity,
    realization_backs: &[RealizationBack],
    commitments: &[FragmentCommitment],
) -> PlanId {
    let mut canonical = Vec::new();
    push_string(&mut canonical, form_identity.source_document_id.as_str());
    push_string(&mut canonical, form_identity.checked_form_id.as_str());
    push_string(&mut canonical, form_identity.expanded_form_id.as_str());
    if !realization_backs.is_empty() {
        plan_realization::push_canonical(&mut canonical, realization_backs);
    }
    push_u32(&mut canonical, commitments.len() as u32);
    for commitment in commitments {
        push_string(&mut canonical, commitment.host_id.as_str());
        push_string(&mut canonical, commitment.fragment_id.as_str());
    }
    PlanId::from(hash_bytes(&canonical))
}

fn push_ports(canonical: &mut Vec<u8>, ports: &[PortDescriptor]) {
    push_u32(canonical, ports.len() as u32);
    for port in ports {
        push_string(canonical, port.port_id.as_str());
        push_string(canonical, port.value_kind.as_str());
        canonical.push(match port.direction {
            PortDirection::Input => 0,
            PortDirection::Output => 1,
        });
        canonical.push(match port.temporal {
            PortTemporal::Value => 0,
            PortTemporal::Flow { closes: false } => 1,
            PortTemporal::Flow { closes: true } => 2,
            PortTemporal::Current => 3,
        });
    }
}
