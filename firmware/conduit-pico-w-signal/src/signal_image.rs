//! Generated Pico Signal image interpretation.

mod generated_signal {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/pico_signal_image.rs"));
}

use conduit_kernel::{
    CordId, FixedHostOperationBindings, FixedRoutes, HostOperationId, NodeId, PortId,
    RemoteEndpointId, ValueRef,
};
use conduit_signal::{PULSE_KIND, SHOW_KIND, SIGNAL_ENCODED_LEN, SIGNAL_PORT, SIGNAL_VALUE_KIND};

pub const NODES: usize = generated_signal::GENERATED_NODES.len();
pub const CORDS: usize = generated_signal::GENERATED_CORDS.len();
pub const PORTS: usize = generated_signal::GENERATED_PORTS_PER_NODE;
pub const QUEUE_SLOTS: usize = generated_signal::CORD_VALUE_SLOTS as usize;
pub const ROUTE_SLOTS: usize = generated_signal::GENERATED_ROUTES.len();
pub const ROUTE_TARGETS: usize = generated_signal::GENERATED_ROUTE_TARGETS.len();
pub const HOST_BINDING_SLOTS: usize = generated_signal::GENERATED_HOST_OPERATIONS.len();
pub const PENDING_REQUESTS: usize = generated_signal::GENERATED_HOST_OPERATIONS.len();

#[allow(dead_code)]
pub const MAX_STORED_SIGNAL_VALUES: usize = generated_signal::MAX_STORED_SIGNAL_VALUES;
#[allow(dead_code)]
pub const MAX_STORED_WAIT_VALUES: usize = MAX_STORED_SIGNAL_VALUES - 1;
#[allow(dead_code)]
pub const VALUE_SLOTS: usize = MAX_STORED_SIGNAL_VALUES + MAX_STORED_WAIT_VALUES;
#[allow(dead_code)]
pub const WAIT_VALUE_BYTES: u32 = generated_signal::WAIT_VALUE_BYTES;
pub const RUNTIME_CLUE_EVENTS: usize = generated_signal::RUNTIME_CLUE_EVENTS;
pub const RUNTIME_CLUE_BYTES: u32 =
    (RUNTIME_CLUE_EVENTS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32;

pub const SOURCE_DOCUMENT_ID: &str = generated_signal::SOURCE_DOCUMENT_ID;
pub const CHECKED_FORM_ID: &str = generated_signal::CHECKED_FORM_ID;
pub const EXPANDED_FORM_ID: &str = generated_signal::EXPANDED_FORM_ID;
pub const PLAN_ID: &str = generated_signal::PLAN_ID;
pub const FRAGMENT_ID: &str = generated_signal::FRAGMENT_ID;
pub const HOST_ID: &str = generated_signal::HOST_ID;
pub const BOOT_ID: &str = generated_signal::BOOT_ID;
pub const ACTIVE_PLAY_ID: &str = generated_signal::ACTIVE_PLAY_ID;
pub const FIRMWARE_BUILD_ID: &str = generated_signal::FIRMWARE_BUILD_ID;
#[cfg(not(feature = "wifi-bootstrap"))]
pub const BOOT_CLUE_ID: &str = generated_signal::BOOT_CLUE_ID;
pub const TERMINAL_CLUE_ID: &str = generated_signal::TERMINAL_CLUE_ID;

#[allow(dead_code)]
const WAIT_HOST_OPERATION_CONTRACT: &str = "conduit.host/wait@1";
const PRESENT_HOST_OPERATION_CONTRACT: &str = "conduit.host/present@1";

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
#[derive(Clone, Copy)]
pub struct PresentationIdentity {
    pub presentation_id: &'static str,
    pub clue_id: &'static str,
}

#[cfg(any(feature = "pico-local", feature = "pico-local-minimal"))]
pub fn presentation_identity(sequence: usize) -> Option<PresentationIdentity> {
    Some(PresentationIdentity {
        presentation_id: generated_signal::PRESENTATION_IDS.get(sequence)?,
        clue_id: generated_signal::PRESENTATION_CLUE_IDS.get(sequence)?,
    })
}

#[cfg(any(feature = "usb-remote", feature = "triple-remote", feature = "wifi-bootstrap"))]
pub fn presentation_ids() -> &'static [&'static str] {
    &generated_signal::PRESENTATION_IDS
}

#[cfg(any(feature = "usb-remote", feature = "triple-remote", feature = "wifi-bootstrap"))]
pub fn presentation_clue_ids() -> &'static [&'static str] {
    &generated_signal::PRESENTATION_CLUE_IDS
}

pub fn generated_nodes() -> [conduit_kernel::scheduler::NodeSpec<PORTS>; NODES] {
    generated_signal::GENERATED_NODES
}

pub fn generated_cords() -> [conduit_kernel::scheduler::CordSpec; CORDS] {
    generated_signal::GENERATED_CORDS
}

pub fn generated_routes() -> FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS> {
    let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
    for (node, port, range) in generated_signal::GENERATED_ROUTES {
        let start = usize::from(range.start);
        let end = start + usize::from(range.len);
        routes
            .install(
                node,
                port,
                range,
                &generated_signal::GENERATED_ROUTE_TARGETS[start..end],
            )
            .expect("generated route table valid");
    }
    routes.seal().expect("generated route table sealed");
    routes
}

pub fn generated_host_bindings() -> FixedHostOperationBindings<HOST_BINDING_SLOTS> {
    let mut host_bindings =
        FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(maximum_host_operations_per_node());
    for (node, binding) in generated_signal::GENERATED_HOST_OPERATIONS {
        host_bindings
            .install(node, binding)
            .expect("generated host-operation binding valid");
    }
    host_bindings
        .seal()
        .expect("generated host-operation bindings sealed");
    host_bindings
}

fn maximum_host_operations_per_node() -> u16 {
    let mut maximum = 0u16;
    let mut index = 0usize;
    while index < generated_signal::GENERATED_HOST_OPERATIONS.len() {
        let (node, _) = generated_signal::GENERATED_HOST_OPERATIONS[index];
        let mut count = 0u16;
        let mut inner = 0usize;
        while inner < generated_signal::GENERATED_HOST_OPERATIONS.len() {
            if generated_signal::GENERATED_HOST_OPERATIONS[inner].0 == node {
                count += 1;
            }
            inner += 1;
        }
        if count > maximum {
            maximum = count;
        }
        index += 1;
    }
    maximum
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct SignalLayout {
    pub pulse_node: NodeId,
    pub show_node: NodeId,
    pub pulse_output_port: PortId,
    pub show_input_port: PortId,
    pub wait_operation: HostOperationId,
    pub present_operation: HostOperationId,
    pub configuration: SignalConfiguration,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct SignalConfiguration {
    pub count: usize,
    pub period_ms: u64,
    pub initial_level: bool,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct RemoteSignalLayout {
    pub show_node: NodeId,
    pub show_input_port: PortId,
    pub present_operation: HostOperationId,
}

#[allow(dead_code)]
pub fn remote_signal_layout() -> Option<RemoteSignalLayout> {
    let show_node = generated_node_for_kind(SHOW_KIND)?;
    Some(RemoteSignalLayout {
        show_node,
        show_input_port: generated_port(&generated_signal::GENERATED_INPUT_PORTS, show_node)?,
        present_operation: generated_host_operation(show_node, PRESENT_HOST_OPERATION_CONTRACT)?,
    })
}

#[allow(dead_code)]
pub fn signal_layout() -> Option<SignalLayout> {
    let pulse_node = generated_node_for_kind(PULSE_KIND)?;
    let show_node = generated_node_for_kind(SHOW_KIND)?;
    let configuration = generated_signal_configuration(pulse_node)?;
    Some(SignalLayout {
        pulse_node,
        show_node,
        pulse_output_port: generated_port(&generated_signal::GENERATED_OUTPUT_PORTS, pulse_node)?,
        show_input_port: generated_port(&generated_signal::GENERATED_INPUT_PORTS, show_node)?,
        wait_operation: generated_host_operation(pulse_node, WAIT_HOST_OPERATION_CONTRACT)?,
        present_operation: generated_host_operation(show_node, PRESENT_HOST_OPERATION_CONTRACT)?,
        configuration,
    })
}

fn generated_node_for_kind(kind: &str) -> Option<NodeId> {
    generated_signal::GENERATED_KIND_IDS
        .iter()
        .position(|candidate| *candidate == kind)
        .and_then(|index| u16::try_from(index).ok())
        .map(NodeId)
}

fn generated_port(ports: &[(NodeId, PortId, &str, &str)], node: NodeId) -> Option<PortId> {
    ports
        .iter()
        .find(|(candidate_node, _, port_id, value_kind)| {
            *candidate_node == node && *port_id == SIGNAL_PORT && *value_kind == SIGNAL_VALUE_KIND
        })
        .map(|(_, port, _, _)| *port)
}

fn generated_host_operation(node: NodeId, contract_id: &str) -> Option<HostOperationId> {
    generated_signal::GENERATED_HOST_OPERATIONS
        .iter()
        .zip(generated_signal::GENERATED_HOST_OPERATION_IDENTITIES.iter())
        .find(|((candidate_node, _), (candidate_contract, _, _))| {
            *candidate_node == node && *candidate_contract == contract_id
        })
        .map(|((_, binding), _)| binding.operation)
}

#[allow(dead_code)]
fn generated_signal_configuration(pulse_node: NodeId) -> Option<SignalConfiguration> {
    let mut count = None;
    let mut period_ms = None;
    let mut initial_level = None;
    for (node, key, value) in generated_signal::GENERATED_CONFIGURATION {
        if node != pulse_node {
            continue;
        }
        match (key, value) {
            ("count", generated_signal::GeneratedConfigurationValue::U64(value)) => {
                count = usize::try_from(value).ok();
            }
            ("period-ms", generated_signal::GeneratedConfigurationValue::U64(value)) => {
                period_ms = Some(value);
            }
            ("initial", generated_signal::GeneratedConfigurationValue::Bool(value)) => {
                initial_level = Some(value);
            }
            _ => return None,
        }
    }
    let count = count?;
    if count == 0 || count > MAX_STORED_SIGNAL_VALUES {
        return None;
    }
    if SIGNAL_ENCODED_LEN != generated_signal::CORD_VALUE_BYTES {
        return None;
    }
    Some(SignalConfiguration {
        count,
        period_ms: period_ms?,
        initial_level: initial_level?,
    })
}

#[allow(dead_code)]
pub fn value_store_bytes(count: usize) -> u32 {
    let signal_bytes = count as u32 * SIGNAL_ENCODED_LEN;
    let wait_bytes = count.saturating_sub(1) as u32 * WAIT_VALUE_BYTES;
    signal_bytes + wait_bytes
}

#[allow(dead_code)]
pub fn decode_wait_ms(bytes: &[u8]) -> Option<u64> {
    if bytes.len() != WAIT_VALUE_BYTES as usize {
        return None;
    }
    let mut encoded = [0u8; WAIT_VALUE_BYTES as usize];
    encoded.copy_from_slice(bytes);
    Some(u64::from_le_bytes(encoded))
}

#[allow(dead_code)]
pub const EMPTY_VALUE_REF: ValueRef = ValueRef {
    slot: 0,
    generation: 0,
    byte_len: 0,
};

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct RemoteEndpointIdentity {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
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
    pub base_instance_id: &'static str,
    pub link_binding_id: &'static str,
    pub value_kind: &'static str,
    pub session_item_capacity: u16,
    pub session_byte_capacity: u32,
    pub maximum_in_flight_items: u16,
    pub maximum_payload_bytes: u32,
    pub maximum_buffered_bytes: u32,
    pub maximum_frame_bytes: u32,
}

#[allow(dead_code)]
pub fn generated_remote_endpoint() -> Option<RemoteEndpointIdentity> {
    if generated_signal::GENERATED_REMOTE_ENDPOINT_COUNT != 1 {
        return None;
    }
    let cord = CordId(*generated_signal::GENERATED_REMOTE_ENDPOINT_CORDS.first()?);
    let cord_spec = generated_signal::GENERATED_CORDS.get(usize::from(cord.0))?;
    Some(RemoteEndpointIdentity {
        endpoint: RemoteEndpointId(*generated_signal::GENERATED_REMOTE_ENDPOINT_IDS.first()?),
        cord,
        connection_id: generated_signal::GENERATED_REMOTE_ENDPOINT_CONNECTION_IDS.first()?,
        source_fragment_id: generated_signal::GENERATED_REMOTE_ENDPOINT_SOURCE_FRAGMENT_IDS
            .first()?,
        sink_fragment_id: generated_signal::GENERATED_REMOTE_ENDPOINT_SINK_FRAGMENT_IDS.first()?,
        local_host: generated_signal::GENERATED_REMOTE_ENDPOINT_LOCAL_HOSTS.first()?,
        local_boot: generated_signal::GENERATED_REMOTE_ENDPOINT_LOCAL_BOOTS.first()?,
        local_endpoint: generated_signal::GENERATED_REMOTE_ENDPOINT_LOCAL_ENDPOINTS.first()?,
        peer_host: generated_signal::GENERATED_REMOTE_ENDPOINT_PEER_HOSTS.first()?,
        peer_boot: generated_signal::GENERATED_REMOTE_ENDPOINT_PEER_BOOTS.first()?,
        peer_endpoint: generated_signal::GENERATED_REMOTE_ENDPOINT_PEER_ENDPOINTS.first()?,
        base_code: *generated_signal::GENERATED_REMOTE_ENDPOINT_BASE_CODES.first()?,
        base_instance_id: generated_signal::GENERATED_REMOTE_ENDPOINT_BASE_INSTANCE_IDS
            .first()?,
        link_binding_id: generated_signal::GENERATED_REMOTE_ENDPOINT_LINK_BINDING_IDS.first()?,
        value_kind: generated_signal::GENERATED_REMOTE_ENDPOINT_VALUE_KINDS.first()?,
        session_item_capacity: cord_spec.item_capacity,
        session_byte_capacity: cord_spec.byte_capacity,
        maximum_in_flight_items: *generated_signal::GENERATED_REMOTE_ENDPOINT_MAXIMUM_IN_FLIGHT_ITEMS
            .first()?,
        maximum_payload_bytes: *generated_signal::GENERATED_REMOTE_ENDPOINT_MAXIMUM_PAYLOAD_BYTES
            .first()?,
        maximum_buffered_bytes: *generated_signal::GENERATED_REMOTE_ENDPOINT_MAXIMUM_BUFFERED_BYTES
            .first()?,
        maximum_frame_bytes: *generated_signal::GENERATED_REMOTE_ENDPOINT_MAXIMUM_FRAME_BYTES
            .first()?,
    })
}
