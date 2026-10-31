//! Historical canonical fragment fields following the placement sequence.
use super::*;
pub(super) fn push_tail(canonical: &mut impl CanonicalSink, tail: FragmentFingerprintTail<'_>) {
    push_u32(canonical, tail.connections.len() as u32);
    for connection in tail.connections {
        push_string(canonical, connection.connection_id.as_str());
        push_string(canonical, connection.source_placement_id.as_str());
        push_string(canonical, connection.source_port_id.as_str());
        push_string(canonical, connection.sink_placement_id.as_str());
        push_string(canonical, connection.sink_port_id.as_str());
        push_string(canonical, connection.value_kind.as_str());
        match &connection.resource {
            Some(resource) => {
                canonical.push(1);
                push_string(canonical, resource.contract.port_id.as_str());
                push_string(canonical, resource.contract.class_id.as_str());
                canonical.push(resource.contract.ownership as u8);
                canonical.push(resource.contract.lifecycle as u8);
                canonical.push(resource.contract.mobility as u8);
                push_string(canonical, resource.owner_placement_id.as_str());
                push_resource_binding(canonical, &resource.source_binding);
            }
            None => canonical.push(0),
        }
        match &connection.abnormal_kind {
            Some(kind) => {
                canonical.push(1);
                push_string(canonical, kind.as_str());
            }
            None => canonical.push(0),
        }
        canonical.push(match connection.track {
            crate::ConnectionTrack::Payload => 0,
            crate::ConnectionTrack::NormalClose => 1,
            crate::ConnectionTrack::AbnormalTerminal => 2,
            crate::ConnectionTrack::Quiescence => 3,
        });
        canonical.push(match connection.temporal {
            PortTemporal::Value => 0,
            PortTemporal::Flow { closes: false } => 1,
            PortTemporal::Flow { closes: true } => 2,
            PortTemporal::Current => 3,
        });
        match &connection.selected_line {
            Some(line) => {
                canonical.push(1);
                push_admitted_line(canonical, line);
            }
            None => canonical.push(0),
        }
        push_u32(canonical, connection.admitted_lines.len() as u32);
        for candidate in &connection.admitted_lines {
            push_admitted_line(canonical, candidate);
        }
        canonical.extend_from_slice(&connection.item_capacity.to_le_bytes());
        push_u32(canonical, connection.byte_capacity);
        canonical.push(connection.pressure_policy as u8);
    }
    push_u32(canonical, tail.fore_ports.len() as u32);
    for port in tail.fore_ports {
        push_string(canonical, port.front_port_id.as_str());
        canonical.push(port.direction as u8);
        push_string(canonical, port.placement_id.as_str());
        push_string(canonical, port.gear_port_id.as_str());
        push_string(canonical, port.value_kind.as_str());
        match &port.value_contract {
            Some(contract) => {
                canonical.push(1);
                push_value_contract(canonical, contract);
            }
            None => canonical.push(0),
        }
        push_optional_string(
            canonical,
            port.abnormal_kind.as_ref().map(|kind| kind.as_str()),
        );
        canonical.push(match port.track {
            crate::ConnectionTrack::Payload => 0,
            crate::ConnectionTrack::NormalClose => 1,
            crate::ConnectionTrack::AbnormalTerminal => 2,
            crate::ConnectionTrack::Quiescence => 3,
        });
        canonical.push(match port.temporal {
            PortTemporal::Value => 0,
            PortTemporal::Flow { closes: false } => 1,
            PortTemporal::Flow { closes: true } => 2,
            PortTemporal::Current => 3,
        });
        canonical.push(port.pressure_policy as u8);
        canonical.extend_from_slice(&port.item_capacity.to_le_bytes());
        push_u32(canonical, port.byte_capacity);
    }
    push_u32(canonical, tail.shared_pools.len() as u32);
    for pool in tail.shared_pools {
        push_string(canonical, pool.pool_id.as_str());
        push_string(canonical, pool.declaration_id.as_str());
        push_checked_front(canonical, &pool.member_front);
        canonical.extend_from_slice(&pool.maximum_members.to_le_bytes());
        canonical.extend_from_slice(&pool.member_limits.queue_item_capacity.to_le_bytes());
        push_u32(canonical, pool.member_limits.queue_byte_capacity);
        canonical.extend_from_slice(&pool.member_limits.sign_item_capacity.to_le_bytes());
        push_u32(canonical, pool.member_limits.sign_byte_capacity);
        canonical.push(u8::from(pool.member_sessions_required));
        canonical.push(pool.selection_policy.fingerprint_discriminant());
        push_u32(canonical, pool.realization_envelope.len() as u32);
        for realization in &pool.realization_envelope {
            push_string(canonical, realization.host_id.as_str());
            push_string(canonical, realization.boot_id.as_str());
            canonical.extend_from_slice(&realization.offer_generation.0.to_le_bytes());
            push_string(canonical, realization.capability_id.as_str());
            push_string(canonical, realization.implementation_id.as_str());
            push_string(canonical, realization.artifact_id.as_str());
            canonical.extend_from_slice(&realization.member_capacity.to_le_bytes());
            push_u32(canonical, realization.resources.len() as u32);
            for resource in &realization.resources {
                push_string(canonical, resource.pool_id.as_str());
                push_string(canonical, resource.class_id.as_str());
                push_u32(canonical, resource.units);
                match &resource.compute {
                    None => canonical.push(0),
                    Some(compute) => {
                        canonical.push(1);
                        push_u32(canonical, compute.selected_lanes);
                        canonical.push(compute.service_guarantee as u8);
                        push_string(canonical, compute.architecture_base_id.as_str());
                        canonical.push(compute.architecture_base_kind as u8);
                        push_optional_string(
                            canonical,
                            compute.topology_group_id.as_ref().map(|id| id.as_str()),
                        );
                        push_optional_string(
                            canonical,
                            compute.performance_class.as_ref().map(|id| id.as_str()),
                        );
                        match compute.nominal_clock_hz {
                            Some(value) => {
                                canonical.push(1);
                                canonical.extend_from_slice(&value.to_le_bytes());
                            }
                            None => canonical.push(0),
                        }
                    }
                }
            }
            push_u32(canonical, realization.admitted_lines.len() as u32);
            for line in &realization.admitted_lines {
                push_admitted_line(canonical, line);
            }
        }
        push_string(canonical, pool.admission_authority.as_str());
        push_u32(canonical, pool.consumers.len() as u32);
        for consumer in &pool.consumers {
            push_string(canonical, consumer.as_str());
        }
    }
    push_u32(canonical, tail.startup_dependencies.len() as u32);
    for dependency in tail.startup_dependencies {
        push_string(canonical, dependency.prerequisite_placement_id.as_str());
        push_string(canonical, dependency.dependent_placement_id.as_str());
    }
    push_u32(canonical, tail.startup_order.len() as u32);
    for placement_id in tail.startup_order {
        push_string(canonical, placement_id.as_str());
    }
    canonical.push(match tail.cancellation_policy {
        CancellationPolicy::CancelAllAndRejectLateCompletion => 0,
        CancellationPolicy::DrainBeforeCancel => 1,
    });
    canonical.push(match tail.terminal_policy {
        TerminalPolicy::RequireAllPlacementsAndConnections => 0,
        TerminalPolicy::RequirePlacementsOnly => 1,
    });
    push_u32(canonical, tail.expected_terminals.len() as u32);
    for terminal in tail.expected_terminals {
        match terminal {
            ExpectedTerminal::PlacementCompleted(placement_id) => {
                canonical.push(0);
                push_string(canonical, placement_id.as_str());
            }
            ExpectedTerminal::ConnectionCompleted(connection_id) => {
                canonical.push(1);
                push_string(canonical, connection_id.as_str());
            }
            ExpectedTerminal::PlanCompleted => canonical.push(2),
        }
    }
    push_u32(canonical, tail.expected_sign.len() as u32);
    for sign in tail.expected_sign {
        match sign {
            ExpectedSign::PlanFragmentReceived => canonical.push(0),
            ExpectedSign::PlacementPrepared(placement_id) => {
                canonical.push(1);
                push_string(canonical, placement_id.as_str());
            }
            ExpectedSign::PlacementTerminal(placement_id) => {
                canonical.push(2);
                push_string(canonical, placement_id.as_str());
            }
            ExpectedSign::ConnectionTerminal(connection_id) => {
                canonical.push(3);
                push_string(canonical, connection_id.as_str());
            }
            ExpectedSign::PlanTerminal => canonical.push(4),
        }
    }
    canonical.extend_from_slice(&tail.sign_storage_budget.item_capacity.to_le_bytes());
    push_u32(canonical, tail.sign_storage_budget.byte_capacity);
    crate::state_delay::push_canonical_state(canonical, tail.states);
    // Keep the historical fingerprint byte-for-byte when no external Fore
    // selects a Line. Bound Fore identities extend that canonical encoding.
    let bound = tail
        .fore_ports
        .iter()
        .enumerate()
        .filter_map(|(index, port)| port.selected_line.as_ref().map(|line| (index, line)));
    let bound_count = bound.clone().count();
    if bound_count != 0 {
        push_string(canonical, "conduit.plan/fore-lines@1");
        push_u32(canonical, bound_count as u32);
        for (index, line) in bound {
            push_u32(canonical, index as u32);
            push_admitted_line(canonical, line);
        }
    }
}
