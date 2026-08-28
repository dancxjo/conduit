use conduit_core::{
    assigned_plan_magic, assigned_plan_payload_digest, AssignedIdentity, AssignedPlanMaxima,
    ASSIGNED_CONFIGURATION, ASSIGNED_CORD, ASSIGNED_HOST_OPERATION, ASSIGNED_NODE,
    ASSIGNED_PLAN_HEADER_BYTES, ASSIGNED_PLAN_SCHEMA, ASSIGNED_PORT, ASSIGNED_REMOTE_ENDPOINT,
    ASSIGNED_RESOURCE, ASSIGNED_ROUTE, ASSIGNED_ROUTE_TARGET, ASSIGNED_SIGN, ASSIGNED_STARTUP,
    ASSIGNED_TERMINAL,
};
use conduit_plan_lowering::lowering::RemoteCordDirection;

use crate::{
    GeneratedConfigurationValue, GeneratedCordEndpoint, GeneratedEmbeddedPlan, GeneratedSignTarget,
    GenerationError,
};

/// Encodes the already selected, sealed Host fragment. This is deliberately a
/// projection, not a planner: every record comes from `GeneratedEmbeddedPlan`.
pub fn encode_assigned_plan(
    plan: &GeneratedEmbeddedPlan,
    maxima: AssignedPlanMaxima,
) -> Result<Vec<u8>, GenerationError> {
    let mut payload = Vec::new();
    let mut counts = [0_u8; 12];
    let mut record = |tag: u8, value: Vec<u8>| -> Result<(), GenerationError> {
        let index = usize::from(tag - 1);
        counts[index] = counts[index]
            .checked_add(1)
            .ok_or(GenerationError::ArithmeticOverflow(
                "assigned-plan record count",
            ))?;
        put_record(&mut payload, tag, &value)?;
        Ok(())
    };

    for node in &plan.nodes {
        let mut value = Vec::new();
        u16_to(&mut value, node.node);
        u16_to(&mut value, node.maximum_step_work);
        identity_to(&mut value, &node.kind_id);
        identity_to(&mut value, &node.implementation_id);
        identity_to(&mut value, &node.artifact_id);
        record(ASSIGNED_NODE, value)?;
    }
    for (direction, ports) in [(0_u8, &plan.input_ports), (1, &plan.output_ports)] {
        for port in ports {
            let mut value = Vec::new();
            u16_to(&mut value, port.node);
            u16_to(&mut value, port.port);
            value.push(direction);
            identity_to(&mut value, &port.port_id);
            identity_to(&mut value, &port.value_kind);
            record(ASSIGNED_PORT, value)?;
        }
    }
    for entry in &plan.configuration {
        let mut value = Vec::new();
        u16_to(&mut value, entry.node);
        identity_to(&mut value, &entry.key);
        match entry.value {
            GeneratedConfigurationValue::Bool(item) => {
                value.push(0);
                value.extend_from_slice(&[u8::from(item), 0, 0, 0, 0, 0, 0, 0]);
            }
            GeneratedConfigurationValue::I64(item) => {
                value.push(1);
                value.extend_from_slice(&item.to_le_bytes());
            }
            GeneratedConfigurationValue::U64(item) => {
                value.push(2);
                value.extend_from_slice(&item.to_le_bytes());
            }
        }
        record(ASSIGNED_CONFIGURATION, value)?;
    }
    for cord in &plan.cords {
        let mut value = Vec::new();
        u16_to(&mut value, cord.cord);
        identity_to(&mut value, &cord.connection_id);
        endpoint_to(&mut value, cord.source);
        endpoint_to(&mut value, cord.sink);
        u16_to(&mut value, cord.slot_start);
        u16_to(&mut value, cord.item_capacity);
        u32_to(&mut value, cord.byte_capacity);
        record(ASSIGNED_CORD, value)?;
    }
    for route in &plan.routes {
        let mut value = Vec::new();
        for item in [
            route.source_node,
            route.source_port,
            route.target_start,
            route.target_len,
        ] {
            u16_to(&mut value, item);
        }
        record(ASSIGNED_ROUTE, value)?;
    }
    for target in &plan.route_targets {
        let mut value = Vec::new();
        u16_to(&mut value, target.cord);
        match target.sink {
            GeneratedCordEndpoint::Local { node, port } => {
                u16_to(&mut value, node);
                u16_to(&mut value, port);
            }
            GeneratedCordEndpoint::Remote { endpoint } => {
                value.push(1);
                u16_to(&mut value, endpoint);
                u16_to(&mut value, 0);
            }
        }
        record(ASSIGNED_ROUTE_TARGET, value)?;
    }
    for operation in &plan.host_operations {
        let mut value = Vec::new();
        u16_to(&mut value, operation.node);
        u16_to(&mut value, operation.operation);
        identity_to(&mut value, &operation.contract_id);
        optional_identity_to(&mut value, operation.target_kind.as_deref());
        u16_to(&mut value, operation.maximum_in_flight);
        u32_to(&mut value, operation.maximum_input_bytes);
        u32_to(&mut value, operation.maximum_output_bytes);
        record(ASSIGNED_HOST_OPERATION, value)?;
    }
    for resource in &plan.resources {
        let mut value = Vec::new();
        u16_to(&mut value, resource.node);
        u16_to(&mut value, resource.resource);
        u32_to(&mut value, resource.units);
        record(ASSIGNED_RESOURCE, value)?;
    }
    for sign in &plan.signs {
        let mut value = Vec::new();
        u16_to(&mut value, sign.expectation);
        identity_to(&mut value, sign.kind);
        optional_identity_to(&mut value, sign.subject.as_deref());
        match sign.target {
            GeneratedSignTarget::Fragment => {
                value.push(0);
                u16_to(&mut value, 0);
            }
            GeneratedSignTarget::Node(node) => {
                value.push(1);
                u16_to(&mut value, node);
            }
            GeneratedSignTarget::Cord(cord) => {
                value.push(2);
                u16_to(&mut value, cord);
            }
        }
        record(ASSIGNED_SIGN, value)?;
    }
    for remote in &plan.remote_endpoints {
        let mut value = Vec::new();
        for item in [
            &remote.line_id,
            &remote.local_host,
            &remote.local_boot,
            &remote.peer_host,
            &remote.peer_boot,
            &remote.connection_id,
            &remote.source_fragment_id,
            &remote.sink_fragment_id,
            &remote.local_endpoint,
            &remote.peer_endpoint,
            &remote.base_instance_id,
            &remote.link_binding_id,
            &remote.value_kind,
        ] {
            identity_to(&mut value, item);
        }
        u16_to(&mut value, remote.endpoint);
        u16_to(&mut value, remote.cord);
        identity_to(&mut value, remote.base.as_str());
        value.push(match remote.direction {
            RemoteCordDirection::Egress => 0,
            RemoteCordDirection::Ingress => 1,
        });
        value.extend_from_slice(&[
            remote.contract.scope as u8,
            remote.contract.traffic_shape as u8,
            remote.contract.duplex as u8,
            remote.contract.ordering as u8,
            remote.contract.reliability as u8,
            remote.contract.continuation as u8,
            remote.contract.security as u8,
        ]);
        u16_to(&mut value, remote.maximum_in_flight_items);
        u32_to(&mut value, remote.maximum_payload_bytes);
        u32_to(&mut value, remote.maximum_buffered_bytes);
        u32_to(&mut value, remote.maximum_frame_bytes);
        record(ASSIGNED_REMOTE_ENDPOINT, value)?;
    }
    for dependency in &plan.startup_dependencies {
        let mut value = vec![0];
        u16_to(&mut value, dependency.prerequisite_node);
        u16_to(&mut value, dependency.dependent_node);
        record(ASSIGNED_STARTUP, value)?;
    }
    for node in &plan.startup_order {
        let mut value = vec![1];
        u16_to(&mut value, *node);
        record(ASSIGNED_STARTUP, value)?;
    }
    for terminal in &plan.expected_terminals {
        let mut value = Vec::new();
        identity_to(&mut value, terminal.kind);
        optional_identity_to(&mut value, terminal.subject.as_deref());
        record(ASSIGNED_TERMINAL, value)?;
    }

    for (index, (actual, maximum)) in counts.iter().zip(maxima.counts).enumerate() {
        bound(
            &format!("assigned record {}", index + 1),
            u64::from(*actual),
            u64::from(maximum),
        )?;
    }
    let runtime_state_bytes = runtime_state_bytes(plan)?;
    bound(
        "assigned runtime state bytes",
        u64::from(runtime_state_bytes),
        u64::from(maxima.runtime_state_bytes),
    )?;
    let total = ASSIGNED_PLAN_HEADER_BYTES
        .checked_add(payload.len())
        .ok_or(GenerationError::ArithmeticOverflow(
            "assigned-plan encoded bytes",
        ))?;
    bound(
        "assigned encoded bytes",
        total as u64,
        u64::from(maxima.encoded_bytes),
    )?;
    let total = u16::try_from(total).map_err(|_| GenerationError::BoundExceeded {
        table: "assigned encoded bytes",
        actual: total as u64,
        maximum: u64::from(u16::MAX),
    })?;

    let mut bytes = Vec::with_capacity(usize::from(total));
    bytes.extend_from_slice(&assigned_plan_magic());
    u16_to(&mut bytes, ASSIGNED_PLAN_SCHEMA);
    u16_to(&mut bytes, total);
    u16_to(&mut bytes, runtime_state_bytes);
    u16_to(&mut bytes, 0);
    for identity in [
        &plan.plan_id,
        &plan.fragment_id,
        &plan.host_id,
        &plan.boot_id,
    ] {
        identity_to(&mut bytes, identity);
    }
    bytes.extend_from_slice(&counts);
    bytes.extend_from_slice(&assigned_plan_payload_digest(&payload));
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn runtime_state_bytes(plan: &GeneratedEmbeddedPlan) -> Result<u16, GenerationError> {
    let table_state = plan.nodes.len() * 8
        + plan.cords.len() * 12
        + plan.route_targets.len() * 6
        + plan.host_operations.len() * 12
        + plan.resources.len() * 8
        + plan.remote_endpoints.len() * 16;
    let total = usize::try_from(plan.cord_value_bytes)
        .ok()
        .and_then(|bytes| bytes.checked_add(usize::try_from(plan.sign_bytes).ok()?))
        .and_then(|bytes| bytes.checked_add(table_state))
        .ok_or(GenerationError::ArithmeticOverflow(
            "assigned runtime state bytes",
        ))?;
    u16::try_from(total).map_err(|_| GenerationError::BoundExceeded {
        table: "assigned runtime state bytes",
        actual: total as u64,
        maximum: u64::from(u16::MAX),
    })
}

fn put_record(target: &mut Vec<u8>, tag: u8, value: &[u8]) -> Result<(), GenerationError> {
    let length = u16::try_from(value.len()).map_err(|_| GenerationError::BoundExceeded {
        table: "assigned record bytes",
        actual: value.len() as u64,
        maximum: u64::from(u16::MAX),
    })?;
    target.push(tag);
    u16_to(target, length);
    target.extend_from_slice(value);
    Ok(())
}

fn endpoint_to(target: &mut Vec<u8>, endpoint: GeneratedCordEndpoint) {
    match endpoint {
        GeneratedCordEndpoint::Local { node, port } => {
            target.push(0);
            u16_to(target, node);
            u16_to(target, port);
        }
        GeneratedCordEndpoint::Remote { endpoint } => {
            target.push(1);
            u16_to(target, endpoint);
            u16_to(target, 0);
        }
    }
}

fn optional_identity_to(target: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => identity_to(target, value),
        None => target.extend_from_slice(&[0; 16]),
    }
}

fn identity_to(target: &mut Vec<u8>, value: &str) {
    target.extend_from_slice(&AssignedIdentity::from_text(value).0);
}
fn u16_to(target: &mut Vec<u8>, value: u16) {
    target.extend_from_slice(&value.to_le_bytes());
}
fn u32_to(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn bound(table: &str, actual: u64, maximum: u64) -> Result<(), GenerationError> {
    if actual > maximum {
        // Error table labels are static elsewhere; this encoder reports the
        // stable category while retaining exact values.
        return Err(GenerationError::BoundExceeded {
            table: "assigned projection",
            actual,
            maximum,
        });
    }
    let _ = table;
    Ok(())
}
