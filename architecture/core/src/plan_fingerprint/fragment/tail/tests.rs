//! Independent historical byte oracle for all borrowed-tail forwarding.
use super::*;
use crate::canonical_bytes::CanonicalHash;
use crate::*;
use alloc::vec::Vec;
mod api {
    pub use crate::*;
}
#[path = "../../../../tests/common/fingerprint_refs.rs"]
mod fixture;
// Historical test-only byte oracle.

fn legacy_tail(canonical: &mut impl CanonicalSink, tail: FragmentFingerprintTail<'_>) {
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

fn legacy_header(canonical: &mut impl CanonicalSink, header: FragmentFingerprintHeader<'_>) {
    push_string(canonical, header.source_document_id.as_str());
    push_string(canonical, header.checked_plot_id.as_str());
    push_string(canonical, header.expanded_plot_id.as_str());
    canonical.push(header.completion_policy as u8);
    if !header.realization_backs.is_empty() {
        plan_realization::push_canonical(canonical, header.realization_backs);
    }
    push_string(canonical, header.host_id.as_str());
    push_string(canonical, header.boot_id.as_str());
    push_u64(canonical, header.offer_generation.0);
    execution_regions::push_canonical(canonical, header.execution_regions);
    execution_fusion::push_canonical(canonical, header.execution_fusions);
}

#[test]
fn borrowed_header_tail_bytes_match_historical_full_records() {
    for mask in 0..512 {
        for track in [
            ConnectionTrack::Payload,
            ConnectionTrack::NormalClose,
            ConnectionTrack::AbnormalTerminal,
            ConnectionTrack::Quiescence,
        ] {
            for temporal in [
                PortTemporal::Value,
                PortTemporal::Current,
                PortTemporal::Flow { closes: false },
                PortTemporal::Flow { closes: true },
            ] {
                let mut f = fixture::populated_fragment();
                if mask & 1 == 0 {
                    f.connections[0].resource = None;
                }
                if mask & 2 == 0 {
                    f.connections[0].selected_line = None;
                }
                if mask & 4 == 0 {
                    f.connections[0].admitted_lines.clear();
                }
                if mask & 8 == 0 {
                    f.connections[0].abnormal_kind = None;
                }
                if mask & 16 == 0 {
                    f.fore_ports[0].selected_line = None;
                }
                if mask & 32 == 0 {
                    f.shared_pools[0].realization_envelope[0].resources[0].compute = None;
                }
                if mask & 64 == 0 {
                    f.realization_backs.clear();
                    f.execution_regions.clear();
                    f.execution_fusions.clear();
                }
                if mask & 128 == 0 {
                    f.cancellation_policy = CancellationPolicy::DrainBeforeCancel;
                    f.terminal_policy = TerminalPolicy::RequirePlacementsOnly;
                    f.completion_policy = PlanCompletionPolicy::SemanticCompletion;
                }
                f.connections[0].track = track;
                f.connections[0].temporal = temporal;
                f.connections[0].pressure_policy = if mask & 1 == 0 {
                    DeliveryPressurePolicy::PreserveOrder
                } else {
                    DeliveryPressurePolicy::CoalesceLatest
                };
                if mask & 256 == 0 {
                    f.connections.clear();
                    f.fore_ports.clear();
                    f.shared_pools.clear();
                    f.startup_dependencies.clear();
                    f.startup_order.clear();
                    f.expected_terminals.clear();
                    f.expected_sign.clear();
                    f.states.clear();
                }
                let mut expected = Vec::new();
                legacy_header(&mut expected, (&f).into());
                let mut actual = Vec::new();
                super::super::push_header(&mut actual, (&f).into());
                assert_eq!(actual, expected, "header {mask}");
                expected.clear();
                legacy_tail(&mut expected, (&f).into());
                actual.clear();
                push_tail(&mut actual, (&f).into());
                assert_eq!(actual, expected, "owned tail {mask}");
                let connections: Vec<_> = f.connections.iter().map(Into::into).collect();
                let dependencies: Vec<_> = f.startup_dependencies.iter().map(Into::into).collect();
                let order: Vec<_> = f.startup_order.iter().map(|v| v.as_str()).collect();
                let terminals: Vec<_> = f.expected_terminals.iter().map(Into::into).collect();
                let signs: Vec<_> = f.expected_sign.iter().map(Into::into).collect();
                let mut view = FragmentFingerprintTailRef::from(&f);
                view.connections = PlannedConnectionRefs::Borrowed(&connections);
                view.startup_dependencies = StartupDependencyRefs::Borrowed(&dependencies);
                view.startup_order = PlacementIdRefs::Borrowed(&order);
                view.expected_terminals = ExpectedTerminalRefs::Borrowed(&terminals);
                view.expected_sign = ExpectedSignRefs::Borrowed(&signs);
                actual.clear();
                push_tail(&mut actual, view);
                assert_eq!(actual, expected, "borrowed tail {mask}");
            }
        }
    }
    let f = fixture::populated_fragment();
    let mut old = CanonicalHash::new();
    legacy_header(&mut old, (&f).into());
    let mut new = CanonicalHash::new();
    super::super::push_header(&mut new, (&f).into());
    assert_eq!(old.finish(), new.finish());
}
