//! Exact common fragment materialization, preserving original table order.
use super::*;
pub(super) fn lower(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<LoweredPlanFragment, LoweringError> {
    let placement_nodes = PlacementNodes(&fragment.placements);
    let materialize::NodeTables {
        nodes,
        specs: mut node_specs,
        ports: identity_ports,
    } = materialize::nodes(fragment, profile)?;
    let mut cords = reserved(
        fragment
            .connections
            .len()
            .checked_add(fragment.fore_ports.len())
            .ok_or(LoweringError::CapacityOverflow)?,
    )?;
    let mut remote_endpoints = reserved(fragment.connections.len())?;
    let mut value_slots = 0u16;
    let mut value_bytes = 0u32;
    let mut resource_cords = reserved(fragment.connections.len())?;
    for (connection_index, connection) in fragment.connections.iter().enumerate() {
        if fragment.connections[..connection_index]
            .iter()
            .any(|prior| prior.connection_id == connection.connection_id)
        {
            return Err(LoweringError::DuplicateConnection(
                connection.connection_id.clone(),
            ));
        }
        if connection.item_capacity == 0 || connection.byte_capacity == 0 {
            return Err(LoweringError::InvalidConnectionBudget(
                connection.connection_id.clone(),
            ));
        }
        let source_node = placement_nodes.get(&connection.source_placement_id);
        let sink_node = placement_nodes.get(&connection.sink_placement_id);
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
        if let Some(resource) = &connection.resource {
            let (Some(source_node), Some(source_port), Some(sink_node), Some(sink_port)) =
                (source_node, source_port, sink_node, sink_port)
            else {
                return Err(LoweringError::ResourceAuthorityTransferUnsupported(
                    connection.connection_id.clone(),
                ));
            };
            if resource.owner_placement_id != connection.source_placement_id
                || resource.contract.port_id != connection.source_port_id
                || resource.contract.mobility != conduit_core::ResourcePortMobility::HostLocal
            {
                return Err(LoweringError::ResourceAuthorityTransferUnsupported(
                    connection.connection_id.clone(),
                ));
            }
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == connection.source_placement_id)
                .ok_or_else(|| {
                    LoweringError::UnknownConnectionEndpoint(connection.connection_id.clone())
                })?;
            let index = placement
                .resources
                .iter()
                .position(|binding| binding == &resource.source_binding)
                .ok_or_else(|| {
                    LoweringError::ResourceBindingInvalid(placement.placement_id.clone())
                })?;
            resource_cords.push(LoweredResourceCord {
                connection_id: connection.connection_id.clone(),
                source_node,
                source_port,
                sink_node,
                sink_port,
                resource: ResourceId(as_u16(index)?),
                contract: resource.contract.clone(),
                binding: resource.source_binding.clone(),
            });
            continue;
        }
        let cord = CordId(as_u16(cords.len())?);
        if source_node.zip(source_port).is_some_and(|(node, port)| {
            let descriptor = &nodes[usize::from(node.0)].outputs[usize::from(port.0)];
            !source_contract_matches(
                descriptor,
                connection.track,
                &connection.value_kind,
                connection.temporal,
            )
        }) || sink_node.zip(sink_port).is_some_and(|(node, port)| {
            let descriptor = &nodes[usize::from(node.0)].inputs[usize::from(port.0)];
            descriptor.value_kind != connection.value_kind
                || (descriptor.temporal != connection.temporal
                    && !matches!(
                        (connection.temporal, descriptor.temporal),
                        (
                            conduit_core::PortTemporal::Flow { .. },
                            conduit_core::PortTemporal::Value
                        )
                    )
                    && !matches!(
                        (connection.temporal, descriptor.temporal),
                        (
                            conduit_core::PortTemporal::Flow { closes: true },
                            conduit_core::PortTemporal::Flow { closes: false }
                        )
                    ))
        }) {
            return Err(LoweringError::ConnectionContractMismatch(
                connection.connection_id.clone(),
            ));
        }
        let slot_start = value_slots;
        let source_value_bound = source_node.zip(source_port).and_then(|(node, port)| {
            let descriptor = &nodes[usize::from(node.0)].outputs[usize::from(port.0)];
            match connection.track {
                ConnectionTrack::AbnormalTerminal => descriptor.maximum_abnormal_value_bytes,
                ConnectionTrack::Payload => descriptor.maximum_value_bytes,
                ConnectionTrack::NormalClose | ConnectionTrack::Quiescence => None,
            }
        });
        let sink_value_bound = sink_node.zip(sink_port).and_then(|(node, port)| {
            nodes[usize::from(node.0)].inputs[usize::from(port.0)].maximum_value_bytes
        });
        if !matches!(
            connection.track,
            ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
        ) && source_value_bound
            .zip(sink_value_bound)
            .is_some_and(|(source, sink)| source > sink)
        {
            return Err(LoweringError::ConnectionContractMismatch(
                connection.connection_id.clone(),
            ));
        }
        let maximum_value_bytes = if !matches!(
            connection.track,
            ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
        ) {
            admitted_maximum_value_bytes(
                source_value_bound
                    .or(sink_value_bound)
                    .map(u32::try_from)
                    .transpose()
                    .map_err(|_| LoweringError::CapacityOverflow)?,
                connection.byte_capacity,
            )
        } else {
            connection.byte_capacity
        };
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
                        pressure_policy: lower_pressure_policy(connection.pressure_policy),
                    },
                )
                .with_track(lower_connection_track(connection.track))
                .with_maximum_value_bytes(maximum_value_bytes)
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
                        pressure_policy: lower_pressure_policy(connection.pressure_policy),
                    },
                )
                .with_track(lower_connection_track(connection.track))
                .with_maximum_value_bytes(maximum_value_bytes)
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
                        pressure_policy: lower_pressure_policy(connection.pressure_policy),
                    },
                )
                .with_track(lower_connection_track(connection.track))
                .with_maximum_value_bytes(maximum_value_bytes)
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

    let mut fore_ports = reserved(fragment.fore_ports.len())?;
    for planned in &fragment.fore_ports {
        if planned.item_capacity == 0 || planned.byte_capacity == 0 {
            return Err(LoweringError::InvalidConnectionBudget(ConnectionId::from(
                planned.front_port_id.as_str(),
            )));
        }
        let node = placement_nodes.get(&planned.placement_id).ok_or_else(|| {
            LoweringError::UnknownConnectionEndpoint(ConnectionId::from(
                planned.front_port_id.as_str(),
            ))
        })?;
        let ports = match planned.direction {
            PortDirection::Input => &nodes[usize::from(node.0)].inputs,
            PortDirection::Output => &nodes[usize::from(node.0)].outputs,
        };
        let port = find_port(ports, &planned.gear_port_id).ok_or_else(|| {
            LoweringError::UnknownConnectionPort(ConnectionId::from(planned.front_port_id.as_str()))
        })?;
        let descriptor = &ports[usize::from(port.0)];
        let descriptor_matches = match planned.track {
            ConnectionTrack::Payload => {
                descriptor.value_kind == planned.value_kind
                    && descriptor.temporal == planned.temporal
            }
            ConnectionTrack::AbnormalTerminal => {
                descriptor.abnormal_kind.as_ref() == Some(&planned.value_kind)
                    && planned.temporal == PortTemporal::Value
            }
            ConnectionTrack::NormalClose => {
                descriptor.temporal == (PortTemporal::Flow { closes: true })
                    && planned.value_kind.as_str() == conduit_core::EMPTY_INFO_ID
                    && planned.temporal == PortTemporal::Value
            }
            ConnectionTrack::Quiescence => {
                matches!(descriptor.temporal, PortTemporal::Flow { .. })
                    && planned.value_kind.as_str() == conduit_core::EMPTY_INFO_ID
                    && planned.temporal == PortTemporal::Value
            }
        };
        if !descriptor_matches {
            return Err(LoweringError::ConnectionContractMismatch(
                ConnectionId::from(planned.front_port_id.as_str()),
            ));
        }
        let cord = CordId(as_u16(cords.len())?);
        let endpoint = RemoteEndpointId(as_u16(remote_endpoints.len() + fore_ports.len())?);
        let capacity = CordCapacity {
            slot_start: value_slots,
            item_capacity: planned.item_capacity,
            byte_capacity: planned.byte_capacity,
            pressure_policy: lower_pressure_policy(planned.pressure_policy),
        };
        value_slots = value_slots
            .checked_add(planned.item_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        value_bytes = value_bytes
            .checked_add(planned.byte_capacity)
            .ok_or(LoweringError::CapacityOverflow)?;
        let maximum_value_bytes = if matches!(
            planned.track,
            ConnectionTrack::NormalClose | ConnectionTrack::Quiescence
        ) {
            planned.byte_capacity
        } else {
            admitted_maximum_value_bytes(
                planned
                    .value_contract
                    .as_ref()
                    .map(|contract| contract.maximum_bytes),
                planned.byte_capacity,
            )
        };
        let spec = match planned.direction {
            PortDirection::Input => {
                let input = &mut node_specs[usize::from(node.0)].input_cords[usize::from(port.0)];
                if input.replace(cord).is_some() {
                    return Err(LoweringError::MultipleConnectionsToInput {
                        placement_id: planned.placement_id.clone(),
                        port_id: planned.gear_port_id.clone(),
                    });
                }
                CordSpec::remote_ingress(cord, endpoint, (node, port), capacity)
            }
            PortDirection::Output => {
                CordSpec::remote_egress(cord, (node, port), endpoint, capacity)
            }
        }
        .with_track(lower_connection_track(planned.track))
        .with_maximum_value_bytes(maximum_value_bytes);
        cords.push(LoweredCord {
            connection_id: materialize::fore_connection_id(planned)?,
            spec,
        });
        fore_ports.push(LoweredForePort {
            front_port_id: planned.front_port_id.clone(),
            direction: planned.direction,
            track: planned.track,
            endpoint,
            cord,
            value_kind: planned.value_kind.clone(),
            value_contract: planned.value_contract.clone(),
            quantity_profile: fore_value::selected_profile(fragment, planned)?,
            abnormal_kind: planned.abnormal_kind.clone(),
            temporal: planned.temporal,
            item_capacity: planned.item_capacity,
            byte_capacity: planned.byte_capacity,
            selected_line: planned.selected_line.clone(),
        });
    }

    let routes = lower_routes(&cords)?;
    let mut host_calls = reserved(materialize::sum_lengths(
        fragment.placements.iter().map(|p| p.host_calls.len()),
    )?)?;
    let mut resources = reserved(materialize::sum_lengths(
        fragment.placements.iter().map(|p| p.resources.len()),
    )?)?;
    for (placement, node) in fragment.placements.iter().zip(nodes.iter()) {
        for (index, requirement) in placement.host_calls.iter().enumerate() {
            if requirement.maximum_in_flight != 1 {
                return Err(LoweringError::UnsupportedHostCallConcurrency(
                    placement.placement_id.clone(),
                ));
            }
            let call = HostCallId(as_u16(index)?);
            host_calls.push(LoweredHostCall {
                node: node.node,
                call,
                contract_id: requirement.contract_id.clone(),
                target_kind: requirement.target_kind.clone(),
                maximum_in_flight: requirement.maximum_in_flight,
                binding: HostCallBinding {
                    call,
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

    let mut signs = reserved(fragment.expected_sign.len())?;
    for (index, expected) in fragment.expected_sign.iter().enumerate() {
        let target = match expected {
            ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => {
                SignExpectationTarget::Fragment
            }
            ExpectedSign::PlacementPrepared(id) | ExpectedSign::PlacementTerminal(id) => {
                SignExpectationTarget::Node(
                    placement_nodes
                        .get(id)
                        .ok_or(LoweringError::SignReferenceMissing)?,
                )
            }
            ExpectedSign::ConnectionTerminal(id) => {
                if let Some(cord) = cords.iter().find(|cord| &cord.connection_id == id) {
                    SignExpectationTarget::Cord(cord.spec.cord)
                } else {
                    let resource = resource_cords
                        .iter()
                        .find(|cord| &cord.connection_id == id)
                        .ok_or(LoweringError::SignReferenceMissing)?;
                    SignExpectationTarget::Resource {
                        node: resource.source_node,
                        resource: resource.resource,
                    }
                }
            }
        };
        signs.push(LoweredSign {
            expectation: SignExpectationId(as_u16(index)?),
            expected: expected.clone(),
            target,
        });
    }

    let shared_pools = lower_shared_pools(fragment, &placement_nodes)?;
    let fusions = lower_fusions(fragment, &placement_nodes, &cords)?;
    let states = state::lower_states(fragment, &placement_nodes)?;

    Ok(LoweredPlanFragment {
        completion_policy: fragment.completion_policy,
        identity: materialize_identity::identity(
            fragment,
            identity_ports,
            materialize_identity::IdentityTables {
                nodes: &nodes,
                cords: &cords,
                remote: &remote_endpoints,
                fore: &fore_ports,
                calls: &host_calls,
                resources: &resources,
                resource_cords: &resource_cords,
            },
        )?,
        nodes,
        node_specs,
        states,
        cords,
        fusions,
        remote_endpoints,
        fore_ports,
        routes,
        host_calls,
        resources,
        resource_cords,
        signs,
        shared_pools,
        cord_value_slots: value_slots,
        cord_value_bytes: value_bytes,
        sign_items: fragment.sign_storage_budget.item_capacity,
        sign_bytes: fragment.sign_storage_budget.byte_capacity,
    })
}
