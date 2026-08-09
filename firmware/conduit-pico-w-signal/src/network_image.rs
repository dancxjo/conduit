//! Generated exact R1 Pico network-join fragment interpretation.

mod generated {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/pico_network_image.rs"));
}

use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, HostOperationId, NodeId, PortId,
};

pub const NODES: usize = generated::GENERATED_NODES.len();
pub const CORDS: usize = generated::GENERATED_CORDS.len();
pub const PORTS: usize = generated::GENERATED_PORTS_PER_NODE;
pub const QUEUE_SLOTS: usize = generated::CORD_VALUE_SLOTS as usize;
// FixedRoutes is directly indexed by (node * ports-per-node + port), so its
// finite slot table covers the full generated address space rather than only
// the number of populated routes.
pub const ROUTE_SLOTS: usize = NODES * PORTS;
pub const ROUTE_TARGETS: usize = generated::GENERATED_ROUTE_TARGETS.len();
pub const HOST_BINDING_SLOTS: usize = generated::GENERATED_HOST_OPERATIONS.len();
pub const PENDING_REQUESTS: usize = generated::GENERATED_HOST_OPERATIONS.len();
// Generated SIGN_ITEMS/SIGN_BYTES are the Plan's mandatory identity-bearing
// Sign budget. The kernel event log has a distinct fixed in-memory profile:
// its byte charge is target-specific `KernelEvent` storage, not serialized
// mandatory-Sign identity bytes.
#[allow(dead_code)]
pub const MANDATORY_PLAN_SIGN_ITEMS: u16 = generated::SIGN_ITEMS;
#[allow(dead_code)]
pub const MANDATORY_PLAN_SIGN_BYTES: u32 = generated::SIGN_BYTES;
pub const RUNTIME_SIGN_EVENTS: usize = 32;
pub const RUNTIME_SIGN_BYTES: u32 =
    (RUNTIME_SIGN_EVENTS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32;
pub const PLAN_ID: &str = generated::PLAN_ID;
pub const FRAGMENT_ID: &str = generated::FRAGMENT_ID;
pub const SOURCE_DOCUMENT_ID: &str = generated::SOURCE_DOCUMENT_ID;
pub const CHECKED_FORM_ID: &str = generated::CHECKED_FORM_ID;
pub const EXPANDED_FORM_ID: &str = generated::EXPANDED_FORM_ID;
pub const HOST_ID: &str = generated::HOST_ID;
pub const BOOT_ID: &str = generated::BOOT_ID;
pub const BOOT_SIGN_ID: &str = generated::BOOT_SIGN_ID;
pub const ATTACHMENT_SIGN_ID: &str = generated::ATTACHMENT_SIGN_ID;
pub const FIRMWARE_BUILD_ID: &str = generated::FIRMWARE_BUILD_ID;

#[derive(Clone, Copy)]
pub struct NetworkJoinLayout {
    pub join_node: NodeId,
    pub join_input_port: PortId,
    pub join_output_port: PortId,
    pub join_operation: HostOperationId,
    pub sign_node: NodeId,
    pub sign_input_port: PortId,
    pub sign_operation: HostOperationId,
}

pub fn network_join_layout() -> Option<NetworkJoinLayout> {
    let join_node = generated::GENERATED_KIND_IDS
        .iter()
        .position(|kind| *kind == conduit_net::NETWORK_JOIN_OPERATION)
        .and_then(|index| u16::try_from(index).ok())
        .map(NodeId)?;
    let join_input_port = generated::GENERATED_INPUT_PORTS
        .iter()
        .find(|(candidate, _, port, info)| {
            *candidate == join_node
                && *port == "request"
                && *info == conduit_net::NETWORK_JOIN_REQUEST_KIND
        })
        .map(|(_, port, _, _)| *port)?;
    let join_operation = generated::GENERATED_HOST_OPERATIONS
        .iter()
        .zip(generated::GENERATED_HOST_OPERATION_IDENTITIES.iter())
        .find(|((candidate, _), (contract, _, _))| {
            *candidate == join_node && *contract == conduit_net::NETWORK_JOIN_HOST_OPERATION
        })
        .map(|((_, binding), _)| binding.operation)?;
    let join_output_port = generated::GENERATED_OUTPUT_PORTS
        .iter()
        .find(|(candidate, _, port, info)| {
            *candidate == join_node
                && *port == "attachment"
                && *info == conduit_net::NETWORK_ATTACHMENT_KIND
        })
        .map(|(_, port, _, _)| *port)?;
    let sign_node = generated::GENERATED_KIND_IDS
        .iter()
        .position(|kind| *kind == conduit_net::NETWORK_ATTACHMENT_SIGN_OPERATION)
        .and_then(|index| u16::try_from(index).ok())
        .map(NodeId)?;
    let sign_input_port = generated::GENERATED_INPUT_PORTS
        .iter()
        .find(|(candidate, _, port, info)| {
            *candidate == sign_node
                && *port == "attachment"
                && *info == conduit_net::NETWORK_ATTACHMENT_KIND
        })
        .map(|(_, port, _, _)| *port)?;
    let sign_operation = generated::GENERATED_HOST_OPERATIONS
        .iter()
        .zip(generated::GENERATED_HOST_OPERATION_IDENTITIES.iter())
        .find(|((candidate, _), (contract, _, _))| {
            *candidate == sign_node
                && *contract == conduit_net::NETWORK_ATTACHMENT_SIGN_HOST_OPERATION
        })
        .map(|((_, binding), _)| binding.operation)?;
    Some(NetworkJoinLayout {
        join_node,
        join_input_port,
        join_output_port,
        join_operation,
        sign_node,
        sign_input_port,
        sign_operation,
    })
}

pub fn generated_nodes() -> [conduit_kernel::scheduler::NodeSpec<PORTS>; NODES] {
    generated::GENERATED_NODES
}

pub fn generated_cords() -> [conduit_kernel::scheduler::CordSpec; CORDS] {
    generated::GENERATED_CORDS
}

pub fn generated_routes(
) -> Result<FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>, conduit_kernel::ProtocolError> {
    let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
    for (node, port, range) in generated::GENERATED_ROUTES {
        let start = usize::from(range.start);
        let end = start
            .checked_add(usize::from(range.len))
            .ok_or(conduit_kernel::ProtocolError::RouteTargetExceeded)?;
        let targets = generated::GENERATED_ROUTE_TARGETS
            .get(start..end)
            .ok_or(conduit_kernel::ProtocolError::RouteTargetExceeded)?;
        routes.install(node, port, range, targets)?;
    }
    routes.seal()?;
    Ok(routes)
}

pub fn generated_host_bindings(
) -> Result<FixedHostOperationBindings<HOST_BINDING_SLOTS>, conduit_kernel::ProtocolError> {
    let mut bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(1);
    for (node, binding) in generated::GENERATED_HOST_OPERATIONS {
        bindings.install(node, binding)?;
    }
    bindings.seal()?;
    Ok(bindings)
}

#[derive(Clone, Copy)]
pub struct GeneratedRemoteEndpoint {
    pub endpoint: conduit_kernel::RemoteEndpointId,
    pub cord: conduit_kernel::CordId,
    pub connection_id: &'static str,
    pub source_fragment_id: &'static str,
    pub sink_fragment_id: &'static str,
    pub local_host: &'static str,
    pub local_boot: &'static str,
    pub local_endpoint: &'static str,
    pub peer_host: &'static str,
    pub peer_boot: &'static str,
    pub peer_endpoint: &'static str,
    pub base_code: u8,
    pub line_id: &'static str,
    pub link_binding_id: &'static str,
    pub base_instance_id: &'static str,
    pub value_kind: &'static str,
    pub maximum_in_flight_items: u16,
    pub maximum_payload_bytes: u32,
    pub maximum_buffered_bytes: u32,
    pub maximum_frame_bytes: u32,
}

pub fn generated_remote_endpoint() -> Option<GeneratedRemoteEndpoint> {
    if generated::GENERATED_REMOTE_ENDPOINT_COUNT != 1 {
        return None;
    }
    Some(GeneratedRemoteEndpoint {
        endpoint: conduit_kernel::RemoteEndpointId(generated::GENERATED_REMOTE_ENDPOINT_IDS[0]),
        cord: conduit_kernel::CordId(generated::GENERATED_REMOTE_ENDPOINT_CORDS[0]),
        connection_id: generated::GENERATED_REMOTE_ENDPOINT_CONNECTION_IDS[0],
        source_fragment_id: generated::GENERATED_REMOTE_ENDPOINT_SOURCE_FRAGMENT_IDS[0],
        sink_fragment_id: generated::GENERATED_REMOTE_ENDPOINT_SINK_FRAGMENT_IDS[0],
        local_host: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_HOSTS[0],
        local_boot: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_BOOTS[0],
        local_endpoint: generated::GENERATED_REMOTE_ENDPOINT_LOCAL_ENDPOINTS[0],
        peer_host: generated::GENERATED_REMOTE_ENDPOINT_PEER_HOSTS[0],
        peer_boot: generated::GENERATED_REMOTE_ENDPOINT_PEER_BOOTS[0],
        peer_endpoint: generated::GENERATED_REMOTE_ENDPOINT_PEER_ENDPOINTS[0],
        base_code: generated::GENERATED_REMOTE_ENDPOINT_BASE_CODES[0],
        line_id: generated::GENERATED_REMOTE_ENDPOINT_LINE_IDS[0],
        link_binding_id: generated::GENERATED_REMOTE_ENDPOINT_LINK_BINDING_IDS[0],
        base_instance_id: generated::GENERATED_REMOTE_ENDPOINT_BASE_INSTANCE_IDS[0],
        value_kind: generated::GENERATED_REMOTE_ENDPOINT_VALUE_KINDS[0],
        maximum_in_flight_items:
            generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_IN_FLIGHT_ITEMS[0],
        maximum_payload_bytes: generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_PAYLOAD_BYTES[0],
        maximum_buffered_bytes: generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_BUFFERED_BYTES[0],
        maximum_frame_bytes: generated::GENERATED_REMOTE_ENDPOINT_MAXIMUM_FRAME_BYTES[0],
    })
}
