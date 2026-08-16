use conduit_core::{
    verify_plan_fragment, ConfigurationValue, ConnectionBase, ExpectedSign, ExpectedTerminal,
    PlanFragment,
};
use conduit_kernel::{CordEndpoint, SignExpectationTarget};
use conduit_runtime::lowering::{
    LoweredPlanFragment, RemoteCordDirection, MAXIMUM_KERNEL_PORTS_PER_NODE,
};

use crate::model::{
    EmbeddedImageBounds, GeneratedConfigurationEntry, GeneratedConfigurationValue,
    GeneratedCordEndpoint, GeneratedEmbeddedPlan, GeneratedExpectedTerminal,
    GeneratedHostOperation, GeneratedPort, GeneratedSignTarget, GeneratedStartupDependency,
    GeneratedStaticCord, GeneratedStaticNode, GeneratedStaticRemoteEndpoint,
    GeneratedStaticResource, GeneratedStaticRoute, GeneratedStaticRouteTarget, GeneratedStaticSign,
    GenerationError, UnsupportedPlanFeature,
};
use crate::validate::validate_shape;
use crate::GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION;

/// Validate and generate one current fixed image. The caller must pass the
/// lowering of the same exact fragment; this function does not lower a second
/// shadow model or consult archived plan types.
pub fn generate_embedded_plan(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    bounds: EmbeddedImageBounds,
) -> Result<GeneratedEmbeddedPlan, GenerationError> {
    if !verify_plan_fragment(fragment) {
        return Err(GenerationError::InvalidFragment);
    }
    if fragment.placements.is_empty() || lowered.nodes.is_empty() {
        return Err(GenerationError::EmptyFragment);
    }
    if fragment.plan_id != lowered.identity.plan_id
        || fragment.fragment_id != lowered.identity.fragment_id
    {
        return Err(GenerationError::IdentityMismatch);
    }
    if bounds.maximum_ports_per_node > MAXIMUM_KERNEL_PORTS_PER_NODE {
        return Err(GenerationError::Unsupported(
            UnsupportedPlanFeature::WiderKernelPortTable,
        ));
    }
    validate_shape(fragment, lowered, bounds)?;

    let configuration = generate_configuration(fragment)?;
    let nodes = generate_nodes(fragment, lowered)?;
    let input_ports = generate_ports(lowered, true);
    let output_ports = generate_ports(lowered, false);
    let cords = generate_cords(lowered)?;
    let remote_endpoints = generate_remote_endpoints(lowered, bounds)?;
    let (routes, route_targets) = generate_routes(lowered)?;
    let host_operations = lowered
        .host_operations
        .iter()
        .map(|operation| GeneratedHostOperation {
            node: operation.node.0,
            operation: operation.operation.0,
            contract_id: operation.contract_id.as_str().to_owned(),
            target_kind: operation
                .target_kind
                .as_ref()
                .map(|kind| kind.as_str().to_owned()),
            maximum_in_flight: operation.maximum_in_flight,
            maximum_input_bytes: operation.binding.maximum_input_bytes,
            maximum_output_bytes: operation.binding.maximum_output_bytes,
        })
        .collect();
    let resources = lowered
        .resources
        .iter()
        .map(|resource| GeneratedStaticResource {
            node: resource.node.0,
            resource: resource.binding.resource.0,
            units: resource.binding.units,
        })
        .collect();
    let signs = lowered.signs.iter().map(generate_sign).collect();
    let startup_dependencies = generate_startup_dependencies(fragment, lowered)?;
    let startup_order = fragment
        .startup_order
        .iter()
        .map(|placement_id| {
            lowered
                .identity
                .node_for_placement(placement_id)
                .map(|node| node.0)
                .ok_or(GenerationError::InconsistentLowering("startup order"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_terminals = fragment
        .expected_terminals
        .iter()
        .map(generate_expected_terminal)
        .collect();

    Ok(GeneratedEmbeddedPlan {
        schema_version: GENERATED_EMBEDDED_PLAN_SCHEMA_VERSION,
        plan_id: fragment.plan_id.as_str().to_owned(),
        fragment_id: fragment.fragment_id.as_str().to_owned(),
        host_id: fragment.host_id.as_str().to_owned(),
        boot_id: fragment.boot_id.as_str().to_owned(),
        offer_generation: fragment.offer_generation.0,
        cancellation_policy: fragment.cancellation_policy,
        terminal_policy: fragment.terminal_policy,
        nodes,
        input_ports,
        output_ports,
        configuration,
        cords,
        remote_endpoints,
        routes,
        route_targets,
        host_operations,
        resources,
        signs,
        startup_dependencies,
        startup_order,
        expected_terminals,
        cord_value_slots: lowered.cord_value_slots,
        cord_value_bytes: lowered.cord_value_bytes,
        sign_items: lowered.sign_items,
        sign_bytes: lowered.sign_bytes,
    })
}

fn generate_remote_endpoints(
    lowered: &LoweredPlanFragment,
    bounds: EmbeddedImageBounds,
) -> Result<Vec<GeneratedStaticRemoteEndpoint>, GenerationError> {
    if lowered.remote_endpoints.len() > bounds.maximum_remote_endpoints {
        return Err(GenerationError::BoundExceeded {
            table: "remote endpoints",
            actual: lowered.remote_endpoints.len() as u64,
            maximum: bounds.maximum_remote_endpoints as u64,
        });
    }
    let mut result = Vec::with_capacity(lowered.remote_endpoints.len());
    for endpoint in &lowered.remote_endpoints {
        if endpoint.direction != RemoteCordDirection::Ingress {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::RemoteConnection,
            ));
        }
        if !matches!(
            endpoint.line.binding.base,
            ConnectionBase::UsbCdc | ConnectionBase::WebSocket | ConnectionBase::BluetoothLeGatt
        ) {
            return Err(GenerationError::Unsupported(
                UnsupportedPlanFeature::RemoteConnection,
            ));
        }
        result.push(GeneratedStaticRemoteEndpoint {
            endpoint: endpoint.endpoint.0,
            cord: endpoint.cord.0,
            connection_id: endpoint.connection_id.as_str().to_owned(),
            source_fragment_id: endpoint.source_fragment_id.as_str().to_owned(),
            sink_fragment_id: endpoint.sink_fragment_id.as_str().to_owned(),
            direction: endpoint.direction,
            local_host: endpoint.local.host_id.as_str().to_owned(),
            local_boot: endpoint.local.boot_id.as_str().to_owned(),
            local_endpoint: endpoint.local.endpoint_id.as_str().to_owned(),
            peer_host: endpoint.peer.host_id.as_str().to_owned(),
            peer_boot: endpoint.peer.boot_id.as_str().to_owned(),
            peer_endpoint: endpoint.peer.endpoint_id.as_str().to_owned(),
            line_id: endpoint.line.line_id.as_str().to_owned(),
            base: endpoint.line.binding.base,
            base_instance_id: endpoint.line.binding.base_instance_id.as_str().to_owned(),
            link_binding_id: endpoint.line.binding.binding_id.as_str().to_owned(),
            value_kind: endpoint.value_kind.as_str().to_owned(),
            maximum_in_flight_items: endpoint.line.binding.limits.maximum_in_flight_items,
            maximum_payload_bytes: endpoint.line.binding.limits.maximum_payload_bytes,
            maximum_buffered_bytes: endpoint.line.binding.limits.maximum_buffered_bytes,
            maximum_frame_bytes: endpoint.line.binding.limits.maximum_frame_bytes,
        });
    }
    Ok(result)
}

fn generate_configuration(
    fragment: &PlanFragment,
) -> Result<Vec<GeneratedConfigurationEntry>, GenerationError> {
    let mut generated = Vec::new();
    for (index, placement) in fragment.placements.iter().enumerate() {
        let node = as_u16(index, "configuration node ordinal")?;
        for entry in &placement.configuration {
            let value = match &entry.value {
                ConfigurationValue::Bool(value) => GeneratedConfigurationValue::Bool(*value),
                ConfigurationValue::I64(value) => GeneratedConfigurationValue::I64(*value),
                ConfigurationValue::U64(value) => GeneratedConfigurationValue::U64(*value),
                ConfigurationValue::Text(_) => {
                    return Err(GenerationError::Unsupported(
                        UnsupportedPlanFeature::TextConfiguration,
                    ));
                }
                ConfigurationValue::Structured(_) => {
                    return Err(GenerationError::Unsupported(
                        UnsupportedPlanFeature::StructuredConfiguration,
                    ));
                }
            };
            generated.push(GeneratedConfigurationEntry {
                node,
                key: entry.key.clone(),
                value,
            });
        }
    }
    Ok(generated)
}

fn generate_nodes(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Vec<GeneratedStaticNode>, GenerationError> {
    lowered
        .nodes
        .iter()
        .zip(&lowered.node_specs)
        .enumerate()
        .map(|(index, (node, spec))| {
            if usize::from(node.node.0) != index {
                return Err(GenerationError::InconsistentLowering(
                    "non-dense node ordinals",
                ));
            }
            let placement = fragment
                .placements
                .get(index)
                .ok_or(GenerationError::InconsistentLowering("node placement"))?;
            if placement.placement_id != node.placement_id
                || spec.maximum_step_work != node.maximum_step_work
            {
                return Err(GenerationError::InconsistentLowering("node table"));
            }
            Ok(GeneratedStaticNode {
                node: node.node.0,
                placement_id: node.placement_id.as_str().to_owned(),
                kind_id: placement.kind_id.as_str().to_owned(),
                implementation_id: placement.implementation_id.as_str().to_owned(),
                artifact_id: placement.artifact_id.as_str().to_owned(),
                input_cords: spec.input_cords.map(|cord| cord.map(|cord| cord.0)),
                maximum_step_work: node.maximum_step_work,
            })
        })
        .collect()
}

fn generate_ports(lowered: &LoweredPlanFragment, inputs: bool) -> Vec<GeneratedPort> {
    lowered
        .nodes
        .iter()
        .flat_map(|node| {
            if inputs {
                node.inputs.iter()
            } else {
                node.outputs.iter()
            }
        })
        .map(|port| GeneratedPort {
            node: port.node.0,
            port: port.port.0,
            port_id: port.port_id.as_str().to_owned(),
            value_kind: port.value_kind.as_str().to_owned(),
        })
        .collect()
}

fn generate_cords(
    lowered: &LoweredPlanFragment,
) -> Result<Vec<GeneratedStaticCord>, GenerationError> {
    lowered
        .cords
        .iter()
        .map(|cord| {
            let source = match cord.spec.source {
                CordEndpoint::Local { node, port } => GeneratedCordEndpoint::Local {
                    node: node.0,
                    port: port.0,
                },
                CordEndpoint::Remote(endpoint) => {
                    if (endpoint.0 as usize) >= lowered.remote_endpoints.len() {
                        return Err(GenerationError::Unsupported(
                            UnsupportedPlanFeature::RemoteConnection,
                        ));
                    }
                    GeneratedCordEndpoint::Remote {
                        endpoint: endpoint.0,
                    }
                }
            };
            let sink = match cord.spec.sink {
                CordEndpoint::Local { node, port } => GeneratedCordEndpoint::Local {
                    node: node.0,
                    port: port.0,
                },
                CordEndpoint::Remote(endpoint) => {
                    if (endpoint.0 as usize) >= lowered.remote_endpoints.len() {
                        return Err(GenerationError::Unsupported(
                            UnsupportedPlanFeature::RemoteConnection,
                        ));
                    }
                    GeneratedCordEndpoint::Remote {
                        endpoint: endpoint.0,
                    }
                }
            };
            Ok(GeneratedStaticCord {
                cord: cord.spec.cord.0,
                connection_id: cord.connection_id.as_str().to_owned(),
                source,
                sink,
                slot_start: cord.spec.slot_start,
                item_capacity: cord.spec.item_capacity,
                byte_capacity: cord.spec.byte_capacity,
            })
        })
        .collect()
}

fn generate_routes(
    lowered: &LoweredPlanFragment,
) -> Result<(Vec<GeneratedStaticRoute>, Vec<GeneratedStaticRouteTarget>), GenerationError> {
    let mut routes = Vec::with_capacity(lowered.routes.len());
    let mut targets = Vec::new();
    for route in &lowered.routes {
        routes.push(GeneratedStaticRoute {
            source_node: route.source_node.0,
            source_port: route.source_port.0,
            target_start: route.range.start,
            target_len: route.range.len,
        });
        for target in &route.targets {
            let CordEndpoint::Local { node, port } = target.sink else {
                return Err(GenerationError::Unsupported(
                    UnsupportedPlanFeature::RemoteRouteTarget,
                ));
            };
            targets.push(GeneratedStaticRouteTarget {
                cord: target.cord.0,
                sink_node: node.0,
                sink_port: port.0,
            });
        }
    }
    Ok((routes, targets))
}

fn generate_startup_dependencies(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<Vec<GeneratedStartupDependency>, GenerationError> {
    fragment
        .startup_dependencies
        .iter()
        .filter_map(|dependency| {
            let prerequisite = lowered
                .identity
                .node_for_placement(&dependency.prerequisite_placement_id);
            let dependent = lowered
                .identity
                .node_for_placement(&dependency.dependent_placement_id);
            match (prerequisite, dependent) {
                (Some(prerequisite), Some(dependent)) => Some(Ok(GeneratedStartupDependency {
                    prerequisite_node: prerequisite.0,
                    dependent_node: dependent.0,
                })),
                // A remote cord's exact fragment dependency names one peer
                // placement. Link/session readiness owns that cross-host gate;
                // it cannot become a local fixed-kernel node dependency.
                (Some(_), None) | (None, Some(_)) => None,
                (None, None) => Some(Err(GenerationError::InconsistentLowering(
                    "startup dependency endpoints",
                ))),
            }
        })
        .collect()
}

fn generate_sign(sign: &conduit_runtime::lowering::LoweredSign) -> GeneratedStaticSign {
    let (kind, subject) = match &sign.expected {
        ExpectedSign::PlanFragmentReceived => ("plan-fragment-received", None),
        ExpectedSign::PlacementPrepared(id) => ("placement-prepared", Some(id.as_str().to_owned())),
        ExpectedSign::PlacementTerminal(id) => ("placement-terminal", Some(id.as_str().to_owned())),
        ExpectedSign::ConnectionTerminal(id) => {
            ("connection-terminal", Some(id.as_str().to_owned()))
        }
        ExpectedSign::PlanTerminal => ("plan-terminal", None),
    };
    let target = match sign.target {
        SignExpectationTarget::Fragment => GeneratedSignTarget::Fragment,
        SignExpectationTarget::Node(node) => GeneratedSignTarget::Node(node.0),
        SignExpectationTarget::Cord(cord) => GeneratedSignTarget::Cord(cord.0),
    };
    GeneratedStaticSign {
        expectation: sign.expectation.0,
        kind,
        subject,
        target,
    }
}

fn generate_expected_terminal(expected: &ExpectedTerminal) -> GeneratedExpectedTerminal {
    match expected {
        ExpectedTerminal::PlacementCompleted(id) => GeneratedExpectedTerminal {
            kind: "placement-completed",
            subject: Some(id.as_str().to_owned()),
        },
        ExpectedTerminal::ConnectionCompleted(id) => GeneratedExpectedTerminal {
            kind: "connection-completed",
            subject: Some(id.as_str().to_owned()),
        },
        ExpectedTerminal::PlanCompleted => GeneratedExpectedTerminal {
            kind: "plan-completed",
            subject: None,
        },
    }
}

fn as_u16(value: usize, subject: &'static str) -> Result<u16, GenerationError> {
    u16::try_from(value).map_err(|_| GenerationError::ArithmeticOverflow(subject))
}
