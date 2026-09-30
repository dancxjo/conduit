//! Canonical fingerprints for immutable Plan fragments and their commitment set.
//!
//! This module encodes admitted truth. It neither selects realizations nor
//! executes or mutates Plans.

use crate::{
    characteristic, execution_fusion, hash_bytes, plan_realization, push_resource_binding,
    push_string, push_u32, push_u64, AdmittedLine, BoundLink, CancellationPolicy, CheckedFront,
    ConfigurationValue, ExpectedSign, ExpectedTerminal, FormBack, FormIdentity, FragmentCommitment,
    FragmentId, LinkAuthorityReference, LinkCredentialReference, PlanFragment, PlanId,
    PlannedActivationEntry, PortDescriptor, PortDirection, PortTemporal, TerminalPolicy,
};
use alloc::string::String;
use alloc::vec::Vec;

fn push_optional_string(canonical: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            canonical.push(1);
            push_string(canonical, value);
        }
        None => canonical.push(0),
    }
}

pub fn compute_fragment_id(fragment: &PlanFragment) -> FragmentId {
    let mut canonical = Vec::new();
    push_string(&mut canonical, fragment.source_document_id.as_str());
    push_string(&mut canonical, fragment.checked_form_id.as_str());
    push_string(&mut canonical, fragment.expanded_form_id.as_str());
    canonical.push(fragment.completion_policy as u8);
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
                ConfigurationValue::Quantity(value) => {
                    canonical.push(5);
                    canonical.extend_from_slice(&value.encode());
                }
            }
        }
        push_string(&mut canonical, gear.host_id.as_str());
        push_string(&mut canonical, gear.boot_id.as_str());
        push_u64(&mut canonical, gear.offer_generation.0);
        push_string(&mut canonical, gear.capability_id.as_str());
        push_string(&mut canonical, gear.implementation_id.as_str());
        push_string(&mut canonical, gear.artifact_id.as_str());
        if let Some(span) = gear.source_span {
            push_string(&mut canonical, "source-span@1");
            push_u64(&mut canonical, span.start);
            push_u64(&mut canonical, span.end);
            push_u64(&mut canonical, span.line);
            push_u64(&mut canonical, span.column);
            push_u64(&mut canonical, span.end_line);
            push_u64(&mut canonical, span.end_column);
        }
        if let Some(base) = &gear.base {
            push_string(&mut canonical, "base-provider-binding@1");
            push_string(&mut canonical, base.base_id.as_str());
            push_string(&mut canonical, base.provider_instance_id.as_str());
            push_u64(&mut canonical, base.provider_generation);
            push_string(&mut canonical, base.implementation_id.as_str());
            push_string(&mut canonical, base.mechanism_family.as_str());
            canonical.push(base.enforcement_class as u8);
        }
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
        push_semantic_contract(&mut canonical, &gear.semantic_contract);
        push_u32(&mut canonical, gear.terminal_transductions.len() as u32);
        for profile in &gear.terminal_transductions {
            push_string(&mut canonical, "terminal-transduction@2");
            push_terminal_transduction(&mut canonical, profile);
        }
        push_u32(&mut canonical, gear.host_calls.len() as u32);
        for requirement in &gear.host_calls {
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
            push_string(&mut canonical, binding.host_call_contract_id.as_str());
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
        match &connection.resource {
            Some(resource) => {
                canonical.push(1);
                push_string(&mut canonical, resource.contract.port_id.as_str());
                push_string(&mut canonical, resource.contract.class_id.as_str());
                canonical.push(resource.contract.ownership as u8);
                canonical.push(resource.contract.lifecycle as u8);
                canonical.push(resource.contract.mobility as u8);
                push_string(&mut canonical, resource.owner_placement_id.as_str());
                push_resource_binding(&mut canonical, &resource.source_binding);
            }
            None => canonical.push(0),
        }
        match &connection.abnormal_kind {
            Some(kind) => {
                canonical.push(1);
                push_string(&mut canonical, kind.as_str());
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
        canonical.push(connection.pressure_policy as u8);
    }
    push_u32(&mut canonical, fragment.fore_ports.len() as u32);
    for port in &fragment.fore_ports {
        push_string(&mut canonical, port.front_port_id.as_str());
        canonical.push(port.direction as u8);
        push_string(&mut canonical, port.placement_id.as_str());
        push_string(&mut canonical, port.gear_port_id.as_str());
        push_string(&mut canonical, port.value_kind.as_str());
        match &port.value_contract {
            Some(contract) => {
                canonical.push(1);
                push_value_contract(&mut canonical, contract);
            }
            None => canonical.push(0),
        }
        push_optional_string(
            &mut canonical,
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
        push_u32(&mut canonical, port.byte_capacity);
    }
    push_u32(&mut canonical, fragment.shared_pools.len() as u32);
    for pool in &fragment.shared_pools {
        push_string(&mut canonical, pool.pool_id.as_str());
        push_string(&mut canonical, pool.declaration_id.as_str());
        push_checked_front(&mut canonical, &pool.member_front);
        canonical.extend_from_slice(&pool.maximum_members.to_le_bytes());
        canonical.extend_from_slice(&pool.member_limits.queue_item_capacity.to_le_bytes());
        push_u32(&mut canonical, pool.member_limits.queue_byte_capacity);
        canonical.extend_from_slice(&pool.member_limits.sign_item_capacity.to_le_bytes());
        push_u32(&mut canonical, pool.member_limits.sign_byte_capacity);
        canonical.push(u8::from(pool.member_sessions_required));
        canonical.push(match pool.selection_policy {
            crate::SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder => 0,
        });
        push_u32(&mut canonical, pool.realization_envelope.len() as u32);
        for realization in &pool.realization_envelope {
            push_string(&mut canonical, realization.host_id.as_str());
            push_string(&mut canonical, realization.boot_id.as_str());
            canonical.extend_from_slice(&realization.offer_generation.0.to_le_bytes());
            push_string(&mut canonical, realization.capability_id.as_str());
            push_string(&mut canonical, realization.implementation_id.as_str());
            push_string(&mut canonical, realization.artifact_id.as_str());
            canonical.extend_from_slice(&realization.member_capacity.to_le_bytes());
            push_u32(&mut canonical, realization.resources.len() as u32);
            for resource in &realization.resources {
                push_string(&mut canonical, resource.pool_id.as_str());
                push_string(&mut canonical, resource.class_id.as_str());
                push_u32(&mut canonical, resource.units);
                match &resource.compute {
                    None => canonical.push(0),
                    Some(compute) => {
                        canonical.push(1);
                        push_u32(&mut canonical, compute.selected_lanes);
                        canonical.push(compute.service_guarantee as u8);
                        push_string(&mut canonical, compute.architecture_base_id.as_str());
                        canonical.push(compute.architecture_base_kind as u8);
                        push_optional_string(
                            &mut canonical,
                            compute.topology_group_id.as_ref().map(|id| id.as_str()),
                        );
                        push_optional_string(
                            &mut canonical,
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
            push_u32(&mut canonical, realization.admitted_lines.len() as u32);
            for line in &realization.admitted_lines {
                push_admitted_line(&mut canonical, line);
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
    crate::state_delay::push_canonical_state(&mut canonical, &fragment.states);
    FragmentId::from(hash_bytes(&canonical))
}

fn push_semantic_contract(canonical: &mut Vec<u8>, contract: &crate::KindSemanticContract) {
    use crate::{KindConfigurationRule as Rule, KindSemanticLaw as Law};

    push_string(canonical, "kind-semantic-contract@1");
    push_u32(canonical, contract.configuration.len() as u32);
    for field in &contract.configuration {
        push_string(canonical, &field.key);
        push_configuration_value(canonical, &field.default_value);
        match &field.rule {
            Rule::Any => canonical.push(0),
            Rule::U64Range { minimum, maximum } => {
                canonical.push(1);
                push_u64(canonical, *minimum);
                push_u64(canonical, *maximum);
            }
            Rule::I64Range { minimum, maximum } => {
                canonical.push(2);
                canonical.extend_from_slice(&minimum.to_le_bytes());
                canonical.extend_from_slice(&maximum.to_le_bytes());
            }
            Rule::DurationMillis { minimum, maximum } => {
                canonical.push(3);
                push_u64(canonical, *minimum);
                push_u64(canonical, *maximum);
            }
            Rule::QuantityRange {
                minimum,
                maximum,
                canonical_unit,
            } => {
                canonical.push(4);
                canonical.extend_from_slice(&minimum.to_le_bytes());
                canonical.extend_from_slice(&maximum.to_le_bytes());
                push_string(canonical, canonical_unit.semantic_id());
            }
            Rule::TextBytes { maximum } => {
                canonical.push(5);
                push_u32(canonical, *maximum);
            }
            Rule::TextOneOf { values } => {
                canonical.push(6);
                push_u32(canonical, values.len() as u32);
                for value in values {
                    push_string(canonical, value);
                }
            }
            Rule::Structured { profile } => {
                canonical.push(7);
                push_string(canonical, profile.as_str());
            }
        }
    }
    push_u32(canonical, contract.laws.len() as u32);
    for law in &contract.laws {
        match law {
            Law::Terminal(value) => {
                use crate::KindTerminalBehavior as Terminal;
                canonical.push(0);
                match value {
                    Terminal::EmitsOnce => canonical.push(0),
                    Terminal::EmitsOnceWhenScopeIsEligible => canonical.push(1),
                    Terminal::CompletesAfterConfiguredCount => canonical.push(2),
                    Terminal::CompletesAfterFixedCount { count } => {
                        canonical.push(3);
                        push_u64(canonical, *count);
                    }
                    Terminal::CompletesWhenInputsClose => canonical.push(4),
                    Terminal::MirrorsInputTerminal => canonical.push(5),
                    Terminal::RetainsLatestUntilReleased => canonical.push(6),
                    Terminal::EmitsCurrentAndCompletesWhenInputCloses => canonical.push(7),
                    Terminal::CoupledAtomicFanoutAndMirrorsInputTerminal => canonical.push(8),
                    Terminal::FirstReadyLeftTieCancelsLoserOrCompletesWithoutWinner => {
                        canonical.push(24)
                    }
                    Terminal::CurrentBooleanGateDefaultsClosedAndCompletesWhenInputsClose => {
                        canonical.push(9)
                    }
                    Terminal::CurrentScalarSelectorCompletesWhenInputsClose => canonical.push(10),
                    Terminal::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible => {
                        canonical.push(11)
                    }
                    Terminal::TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses => {
                        canonical.push(12)
                    }
                    Terminal::InactivityStateCancelsDeadlineAndCompletesWhenInputCloses => {
                        canonical.push(13)
                    }
                    Terminal::DelaysEachValueInOrderAndDrainsOnInputClosure => canonical.push(14),
                    Terminal::LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses => {
                        canonical.push(15)
                    }
                    Terminal::SamplesLatestValueAtCadenceAndCompletesWhenCadenceCloses => {
                        canonical.push(25)
                    }
                    Terminal::TumblingProcessingTimeWindow { maximum_items } => {
                        canonical.push(26);
                        canonical.extend_from_slice(&maximum_items.to_le_bytes());
                    }
                    Terminal::SimulatedCurrentObservationEmitsOnce => canonical.push(16),
                    Terminal::HostInputEndsOrFailsSource => canonical.push(17),
                    Terminal::HostObservationEndsOrFailsSource => canonical.push(18),
                    Terminal::EmitsInitialAndTogglesUntilInputCloses => canonical.push(19),
                    Terminal::EmitsOneField => canonical.push(20),
                    Terminal::EvolvesAfterTicksAndCompletesWhenTickCloses => canonical.push(21),
                    Terminal::PresentsEachFieldAndCompletesWhenInputCloses => canonical.push(22),
                    Terminal::CompletesAfterDockedRefusedOrDeadline => canonical.push(23),
                }
            }
            Law::TerminalTransduction(value) => {
                canonical.push(1);
                push_terminal_transduction(canonical, value);
            }
            Law::ExternalEffects(value) => {
                canonical.push(2);
                canonical.push(*value as u8);
            }
            Law::TemporalState(value) => {
                canonical.push(3);
                canonical.push(*value as u8);
            }
            Law::TimeDependence(value) => {
                canonical.push(4);
                canonical.push(*value as u8);
            }
            Law::RandomDependence(value) => {
                canonical.push(5);
                canonical.push(*value as u8);
            }
            Law::ResourceDependence(value) => {
                canonical.push(6);
                canonical.push(*value as u8);
            }
            Law::Suspension(value) => {
                canonical.push(7);
                canonical.push(*value as u8);
            }
            Law::Variability(value) => {
                canonical.push(8);
                canonical.push(*value as u8);
            }
            Law::Replay(value) => {
                use crate::ReplayBehavior as Replay;
                canonical.push(9);
                match value {
                    Replay::Exact => canonical.push(0),
                    Replay::Ineligible => canonical.push(1),
                    Replay::Idempotent { operation_key_kind } => {
                        canonical.push(2);
                        push_string(canonical, operation_key_kind.as_str());
                    }
                    Replay::Transactional {
                        transaction_contract,
                    } => {
                        canonical.push(3);
                        push_string(canonical, transaction_contract.as_str());
                    }
                    Replay::Compensatable {
                        compensation_contract,
                    } => {
                        canonical.push(4);
                        push_string(canonical, compensation_contract.as_str());
                    }
                }
            }
            Law::ResourcePorts(ports) => {
                canonical.push(10);
                push_u32(canonical, ports.len() as u32);
                for port in ports {
                    push_string(canonical, port.port_id.as_str());
                    push_string(canonical, port.class_id.as_str());
                    canonical.push(port.ownership as u8);
                    canonical.push(port.lifecycle as u8);
                    canonical.push(port.mobility as u8);
                }
            }
            Law::ValueContracts(contracts) => {
                canonical.push(11);
                push_u32(canonical, contracts.len() as u32);
                for value_contract in contracts {
                    match &value_contract.location {
                        crate::FrontValueLocation::Startup(name) => {
                            canonical.push(0);
                            push_string(canonical, name);
                        }
                        crate::FrontValueLocation::Input(port) => {
                            canonical.push(1);
                            push_string(canonical, port.as_str());
                        }
                        crate::FrontValueLocation::Output(port) => {
                            canonical.push(2);
                            push_string(canonical, port.as_str());
                        }
                        crate::FrontValueLocation::InputAbnormal(port) => {
                            canonical.push(3);
                            push_string(canonical, port.as_str());
                        }
                        crate::FrontValueLocation::OutputAbnormal(port) => {
                            canonical.push(4);
                            push_string(canonical, port.as_str());
                        }
                    }
                    push_value_contract(canonical, &value_contract.contract);
                }
            }
            Law::KeyedJoin(join) => {
                canonical.push(12);
                push_value_contract(canonical, &join.key);
                push_value_contract(canonical, &join.left_value);
                push_value_contract(canonical, &join.right_value);
                canonical.extend_from_slice(&join.maximum_pending_per_side.to_le_bytes());
                canonical.push(join.pairing as u8);
                canonical.push(join.output_order as u8);
                canonical.push(join.capacity as u8);
                canonical.push(join.unmatched_on_close as u8);
            }
            Law::BoundedCollect(collect) => {
                canonical.push(13);
                push_string(canonical, collect.input_port_id.as_str());
                push_string(canonical, collect.output_port_id.as_str());
                push_value_contract(canonical, &collect.element);
                push_value_contract(canonical, &collect.collection);
                canonical.extend_from_slice(&collect.maximum_items.to_le_bytes());
                push_value_contract(canonical, &collect.overflow_disposition);
            }
            Law::FlowSelect(select) => {
                canonical.push(14);
                push_string(canonical, select.input_port_id.as_str());
                push_string(canonical, select.output_port_id.as_str());
                push_string(canonical, select.predicate_input_kind.as_str());
                push_string(canonical, select.predicate_output_kind.as_str());
                canonical.extend_from_slice(&select.maximum_active.to_le_bytes());
                canonical.extend_from_slice(&select.maximum_queued.to_le_bytes());
                canonical.extend_from_slice(&select.maximum_items.to_le_bytes());
                canonical.push(select.invocation as u8);
                canonical.push(select.retained_input as u8);
                canonical.push(select.true_disposition as u8);
                canonical.push(select.false_disposition as u8);
            }
            Law::FlowFold(fold) => {
                canonical.push(15);
                push_string(canonical, fold.input_port_id.as_str());
                push_string(canonical, fold.output_port_id.as_str());
                push_value_contract(canonical, &fold.item);
                push_value_contract(canonical, &fold.accumulator);
                push_u32(canonical, fold.initial_accumulator.len() as u32);
                canonical.extend_from_slice(&fold.initial_accumulator);
                push_string(canonical, fold.combine_accumulator_port_id.as_str());
                push_string(canonical, fold.combine_item_port_id.as_str());
                push_string(canonical, fold.combine_output_port_id.as_str());
                canonical.extend_from_slice(&fold.maximum_active.to_le_bytes());
                canonical.extend_from_slice(&fold.maximum_queued.to_le_bytes());
                canonical.extend_from_slice(&fold.maximum_items.to_le_bytes());
                canonical.push(fold.invocation as u8);
                canonical.push(fold.close as u8);
                canonical.push(fold.abnormal as u8);
                canonical.push(fold.cancellation as u8);
            }
            Law::FlowScan(scan) => {
                canonical.push(17);
                push_string(canonical, scan.input_port_id.as_str());
                push_string(canonical, scan.output_port_id.as_str());
                push_value_contract(canonical, &scan.item);
                push_value_contract(canonical, &scan.accumulator);
                push_u32(canonical, scan.initial_accumulator.len() as u32);
                canonical.extend_from_slice(&scan.initial_accumulator);
                push_string(canonical, scan.combine_accumulator_port_id.as_str());
                push_string(canonical, scan.combine_item_port_id.as_str());
                push_string(canonical, scan.combine_output_port_id.as_str());
                canonical.extend_from_slice(&scan.maximum_active.to_le_bytes());
                canonical.extend_from_slice(&scan.maximum_queued.to_le_bytes());
                canonical.extend_from_slice(&scan.maximum_items.to_le_bytes());
                canonical.push(scan.invocation as u8);
                canonical.push(scan.progression as u8);
                canonical.push(scan.empty as u8);
                canonical.push(scan.close as u8);
                canonical.push(scan.abnormal as u8);
                canonical.push(scan.cancellation as u8);
            }
            Law::FlowEach(each) => {
                canonical.push(16);
                push_string(canonical, each.input_port_id.as_str());
                push_string(canonical, each.output_port_id.as_str());
                canonical.extend_from_slice(&each.maximum_items.to_le_bytes());
            }
        }
    }
}

fn push_configuration_value(canonical: &mut Vec<u8>, value: &ConfigurationValue) {
    match value {
        ConfigurationValue::Bool(value) => {
            canonical.push(0);
            canonical.push(u8::from(*value));
        }
        ConfigurationValue::U64(value) => {
            canonical.push(1);
            push_u64(canonical, *value);
        }
        ConfigurationValue::Text(value) => {
            canonical.push(2);
            push_string(canonical, value);
        }
        ConfigurationValue::I64(value) => {
            canonical.push(3);
            canonical.extend_from_slice(&value.to_le_bytes());
        }
        ConfigurationValue::Structured(value) => {
            canonical.push(4);
            push_string(canonical, value.profile().as_str());
            push_u32(canonical, value.canonical_value().len() as u32);
            canonical.extend_from_slice(value.canonical_value());
        }
        ConfigurationValue::Quantity(value) => {
            canonical.push(5);
            canonical.extend_from_slice(&value.encode());
        }
    }
}

fn push_terminal_transduction(
    canonical: &mut Vec<u8>,
    profile: &crate::TerminalTransductionProfile,
) {
    use crate::{
        AbnormalTerminalTransduction as Abnormal, CancellationTransduction as Cancellation,
        NormalCloseTransduction as Normal,
    };
    push_string(canonical, profile.input_port_id.as_str());
    push_string(canonical, profile.output_port_id.as_str());
    match &profile.normal_close {
        Normal::NotAccepted => canonical.push(0),
        Normal::PropagateAfterDrain => canonical.push(1),
        Normal::Consume => canonical.push(2),
        Normal::FlushThenPropagate(bound) => {
            canonical.push(3);
            canonical.extend_from_slice(&bound.maximum_items.to_le_bytes());
            push_u32(canonical, bound.maximum_bytes);
        }
        Normal::DomainSpecific { law } => {
            canonical.push(4);
            push_string(canonical, law.as_str());
        }
        Normal::FlushThenPropagateWhenAllClose(bound) => {
            canonical.push(5);
            canonical.extend_from_slice(&bound.maximum_items.to_le_bytes());
            push_u32(canonical, bound.maximum_bytes);
        }
        Normal::PropagateWhenAllClose => canonical.push(6),
    }
    match &profile.abnormal {
        Abnormal::NotAccepted => canonical.push(0),
        Abnormal::PropagateAfterDrain => canonical.push(1),
        Abnormal::Recover => canonical.push(2),
        Abnormal::FinalizeThenPropagate(bound) => {
            canonical.push(3);
            canonical.extend_from_slice(&bound.maximum_items.to_le_bytes());
            push_u32(canonical, bound.maximum_bytes);
        }
        Abnormal::DomainSpecific { law } => {
            canonical.push(4);
            push_string(canonical, law.as_str());
        }
    }
    match &profile.cancellation {
        Cancellation::NotCancellable => canonical.push(0),
        Cancellation::Request { disposition_kind } => {
            canonical.push(1);
            push_string(canonical, disposition_kind.as_str());
        }
        Cancellation::DomainSpecific { law } => {
            canonical.push(2);
            push_string(canonical, law.as_str());
        }
    }
}

fn push_checked_front(canonical: &mut Vec<u8>, front: &CheckedFront) {
    push_u32(canonical, front.startup_parameters().len() as u32);
    for parameter in front.startup_parameters() {
        push_string(canonical, &parameter.name);
        push_string(canonical, parameter.value_type.as_str());
        canonical.push(u8::from(parameter.has_default));
    }
    push_ports(canonical, front.inputs());
    push_ports(canonical, front.outputs());
    push_u32(canonical, front.resource_ports().len() as u32);
    for resource in front.resource_ports() {
        push_string(canonical, resource.port_id.as_str());
        push_string(canonical, resource.class_id.as_str());
        canonical.push(resource.ownership as u8);
        canonical.push(resource.lifecycle as u8);
        canonical.push(resource.mobility as u8);
    }
    push_u32(canonical, front.value_contracts().len() as u32);
    for value_contract in front.value_contracts() {
        match &value_contract.location {
            crate::FrontValueLocation::Startup(name) => {
                canonical.push(0);
                push_string(canonical, name);
            }
            crate::FrontValueLocation::Input(port) => {
                canonical.push(1);
                push_string(canonical, port.as_str());
            }
            crate::FrontValueLocation::Output(port) => {
                canonical.push(2);
                push_string(canonical, port.as_str());
            }
            crate::FrontValueLocation::InputAbnormal(port) => {
                canonical.push(3);
                push_string(canonical, port.as_str());
            }
            crate::FrontValueLocation::OutputAbnormal(port) => {
                canonical.push(4);
                push_string(canonical, port.as_str());
            }
        }
        push_value_contract(canonical, &value_contract.contract);
    }
    match front.shorthand() {
        Some((input, output)) => {
            canonical.push(1);
            push_string(canonical, input.as_str());
            push_string(canonical, output.as_str());
        }
        None => canonical.push(0),
    }
}

fn push_value_contract(canonical: &mut Vec<u8>, contract: &crate::CheckedValueContract) {
    let identity = contract.identity_bytes();
    push_u32(canonical, identity.len() as u32);
    canonical.extend_from_slice(&identity);
}

/// Returns the canonical semantic fingerprint of an executable Front.
///
/// Nominal callable names and authoring aliases are deliberately absent. The
/// digest changes only when the exact callable contract changes.
pub fn compute_checked_front_fingerprint(front: &CheckedFront) -> String {
    let mut canonical = Vec::new();
    push_checked_front(&mut canonical, front);
    hash_bytes(&canonical)
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
    realization_backs: &[FormBack],
    activations: &[PlannedActivationEntry],
    commitments: &[FragmentCommitment],
) -> PlanId {
    let mut canonical = Vec::new();
    push_string(&mut canonical, form_identity.source_document_id.as_str());
    push_string(&mut canonical, form_identity.checked_form_id.as_str());
    push_string(&mut canonical, form_identity.expanded_form_id.as_str());
    if !realization_backs.is_empty() {
        plan_realization::push_canonical(&mut canonical, realization_backs);
    }
    if !activations.is_empty() {
        push_u32(&mut canonical, activations.len() as u32);
        for entry in activations {
            match entry {
                PlannedActivationEntry::Unary(activation) => {
                    push_string(&mut canonical, "planned-activation@1");
                    push_string(&mut canonical, &activation.activation_id);
                    push_string(&mut canonical, activation.owner_placement_id.as_str());
                    push_string(&mut canonical, activation.selected_plan_id.as_str());
                    push_activation_front(&mut canonical, &activation.input);
                    push_activation_front(&mut canonical, &activation.output);
                    canonical.extend_from_slice(&activation.limits.maximum_active.to_le_bytes());
                    canonical
                        .extend_from_slice(&activation.limits.maximum_queue_items.to_le_bytes());
                    push_u32(&mut canonical, activation.limits.maximum_queue_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_items.to_le_bytes());
                    canonical.push(activation.terminal_policy as u8);
                    canonical.push(activation.cancellation_policy as u8);
                    canonical.push(activation.effect_multiplicity as u8);
                    canonical.extend_from_slice(
                        &activation
                            .per_activation_sign_budget
                            .item_capacity
                            .to_le_bytes(),
                    );
                    push_u32(
                        &mut canonical,
                        activation.per_activation_sign_budget.byte_capacity,
                    );
                }
                PlannedActivationEntry::Fold(activation) => {
                    push_string(&mut canonical, "planned-fold-activation@1");
                    push_string(&mut canonical, &activation.activation_id);
                    push_string(&mut canonical, activation.owner_placement_id.as_str());
                    push_string(&mut canonical, activation.selected_plan_id.as_str());
                    push_activation_front(&mut canonical, &activation.accumulator_input);
                    push_activation_front(&mut canonical, &activation.item_input);
                    push_activation_front(&mut canonical, &activation.output);
                    push_u32(&mut canonical, activation.initial_accumulator.len() as u32);
                    canonical.extend_from_slice(&activation.initial_accumulator);
                    push_u32(&mut canonical, activation.retained_accumulator_bytes);
                    push_u32(&mut canonical, activation.retained_item_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_active.to_le_bytes());
                    canonical
                        .extend_from_slice(&activation.limits.maximum_queue_items.to_le_bytes());
                    push_u32(&mut canonical, activation.limits.maximum_queue_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_items.to_le_bytes());
                    canonical.push(activation.terminal_policy as u8);
                    canonical.push(activation.abnormal_policy as u8);
                    canonical.push(activation.cancellation_policy as u8);
                    canonical.push(activation.effect_multiplicity as u8);
                    canonical.extend_from_slice(
                        &activation
                            .per_activation_sign_budget
                            .item_capacity
                            .to_le_bytes(),
                    );
                    push_u32(
                        &mut canonical,
                        activation.per_activation_sign_budget.byte_capacity,
                    );
                }
                PlannedActivationEntry::Scan(activation) => {
                    push_string(&mut canonical, "planned-scan-activation@1");
                    push_string(&mut canonical, &activation.activation_id);
                    push_string(&mut canonical, activation.owner_placement_id.as_str());
                    push_string(&mut canonical, activation.selected_plan_id.as_str());
                    push_activation_front(&mut canonical, &activation.accumulator_input);
                    push_activation_front(&mut canonical, &activation.item_input);
                    push_activation_front(&mut canonical, &activation.output);
                    push_u32(&mut canonical, activation.initial_accumulator.len() as u32);
                    canonical.extend_from_slice(&activation.initial_accumulator);
                    push_u32(&mut canonical, activation.retained_accumulator_bytes);
                    push_u32(&mut canonical, activation.retained_item_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_active.to_le_bytes());
                    canonical
                        .extend_from_slice(&activation.limits.maximum_queue_items.to_le_bytes());
                    push_u32(&mut canonical, activation.limits.maximum_queue_bytes);
                    canonical.extend_from_slice(&activation.limits.maximum_items.to_le_bytes());
                    canonical.push(activation.terminal_policy as u8);
                    canonical.push(activation.abnormal_policy as u8);
                    canonical.push(activation.cancellation_policy as u8);
                    canonical.push(activation.effect_multiplicity as u8);
                    canonical.extend_from_slice(
                        &activation
                            .per_activation_sign_budget
                            .item_capacity
                            .to_le_bytes(),
                    );
                    push_u32(
                        &mut canonical,
                        activation.per_activation_sign_budget.byte_capacity,
                    );
                }
            }
        }
    }
    push_u32(&mut canonical, commitments.len() as u32);
    for commitment in commitments {
        push_string(&mut canonical, commitment.host_id.as_str());
        push_string(&mut canonical, commitment.fragment_id.as_str());
    }
    PlanId::from(hash_bytes(&canonical))
}

fn push_activation_front(canonical: &mut Vec<u8>, front: &crate::PlannedActivationFront) {
    push_string(canonical, front.front_port_id.as_str());
    push_string(canonical, front.value_kind.as_str());
    push_optional_string(
        canonical,
        front.abnormal_kind.as_ref().map(|kind| kind.as_str()),
    );
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
        match &port.abnormal_kind {
            Some(kind) => {
                canonical.push(1);
                push_string(canonical, kind.as_str());
            }
            None => canonical.push(0),
        }
    }
}
