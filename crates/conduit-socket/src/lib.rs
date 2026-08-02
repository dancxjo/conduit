//! Closed-inventory Linux-class loopback providers for bounded application sockets.
//!
//! Only numeric IPv4 loopback is admitted. The provider performs no DNS,
//! interface, route, firewall, TLS, HTTP, or public-reachability operation.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::Duration;

use conduit_core::SemanticHash;
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, ManagedAdapterBoundary, ManagedComponentDescriptor, Registry,
    RegistryError, ResolutionError, RunIo, RuntimeError, Value,
};
use conduit_std::{
    SOCKET_MAX_DATAGRAMS, SOCKET_MAX_EVIDENCE_EVENTS, SOCKET_MAX_MESSAGE_BYTES,
    SOCKET_MAX_SESSIONS, SOCKET_MAX_STREAM_BYTES,
};

pub const LOOPBACK_NETWORK_RESOURCE: &str = "conduit.resource/socket-loopback";
pub const TCP_CONNECT_GRANT: &str = "conduit.grant/socket-tcp-connect";
pub const TCP_LISTEN_GRANT: &str = "conduit.grant/socket-tcp-listen";
pub const UDP_CONNECTED_GRANT: &str = "conduit.grant/socket-udp-connected";
pub const UDP_DATAGRAM_GRANT: &str = "conduit.grant/socket-udp-datagram";

const TCP_CONNECT_ID: &str = "conduit.host/net/tcp/connect";
const TCP_LISTEN_ID: &str = "conduit.host/net/tcp/listen";
const UDP_CONNECTED_ID: &str = "conduit.host/net/udp/connected";
const UDP_DATAGRAM_ID: &str = "conduit.host/net/udp/datagram";
const LOOPBACK_ADDRESS: &str = "127.0.0.1:0";
const MAXIMUM_DEADLINE_MILLIS: u64 = 10_000;

const TCP_CONNECT_KEYS: &[&str] = &[
    "network_resource",
    "target_grant",
    "local",
    "peer",
    "maximum_sessions",
    "maximum_pending_operations",
    "maximum_send_bytes",
    "maximum_receive_bytes",
    "maximum_chunk_bytes",
    "maximum_queue_bytes",
    "maximum_timers",
    "maximum_work",
    "maximum_evidence_events",
    "deadline_ticks",
    "cleanup_ticks",
    "cancellation",
];
const TCP_LISTEN_KEYS: &[&str] = &[
    "network_resource",
    "bind_grant",
    "local",
    "backlog",
    "maximum_sessions",
    "maximum_accepts",
    "maximum_pending_operations",
    "maximum_send_bytes",
    "maximum_receive_bytes",
    "maximum_chunk_bytes",
    "maximum_queue_bytes",
    "maximum_timers",
    "maximum_work",
    "maximum_evidence_events",
    "deadline_ticks",
    "cleanup_ticks",
    "cancellation",
];
const UDP_CONNECTED_KEYS: &[&str] = &[
    "network_resource",
    "target_grant",
    "local",
    "peer",
    "path_mtu_bytes",
    "fragmentation",
    "maximum_pending_operations",
    "maximum_send_bytes",
    "maximum_receive_bytes",
    "maximum_message_bytes",
    "maximum_queued_messages",
    "maximum_queue_bytes",
    "maximum_timers",
    "maximum_work",
    "maximum_evidence_events",
    "deadline_ticks",
    "cleanup_ticks",
    "cancellation",
];
const UDP_DATAGRAM_KEYS: &[&str] = &[
    "network_resource",
    "bind_grant",
    "local",
    "path_mtu_bytes",
    "fragmentation",
    "maximum_pending_operations",
    "maximum_send_bytes",
    "maximum_receive_bytes",
    "maximum_message_bytes",
    "maximum_queued_messages",
    "maximum_queue_bytes",
    "maximum_timers",
    "maximum_work",
    "maximum_evidence_events",
    "deadline_ticks",
    "cleanup_ticks",
    "cancellation",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SocketKind {
    TcpConnect,
    TcpListen,
    UdpConnected,
    UdpDatagram,
}

impl SocketKind {
    const fn contract_id(self) -> &'static str {
        match self {
            Self::TcpConnect => TCP_CONNECT_ID,
            Self::TcpListen => TCP_LISTEN_ID,
            Self::UdpConnected => UDP_CONNECTED_ID,
            Self::UdpDatagram => UDP_DATAGRAM_ID,
        }
    }

    const fn expected_keys(self) -> &'static [&'static str] {
        match self {
            Self::TcpConnect => TCP_CONNECT_KEYS,
            Self::TcpListen => TCP_LISTEN_KEYS,
            Self::UdpConnected => UDP_CONNECTED_KEYS,
            Self::UdpDatagram => UDP_DATAGRAM_KEYS,
        }
    }

    const fn grant_key(self) -> &'static str {
        match self {
            Self::TcpConnect | Self::UdpConnected => "target_grant",
            Self::TcpListen | Self::UdpDatagram => "bind_grant",
        }
    }

    const fn grant(self) -> &'static str {
        match self {
            Self::TcpConnect => TCP_CONNECT_GRANT,
            Self::TcpListen => TCP_LISTEN_GRANT,
            Self::UdpConnected => UDP_CONNECTED_GRANT,
            Self::UdpDatagram => UDP_DATAGRAM_GRANT,
        }
    }

    const fn is_tcp(self) -> bool {
        matches!(self, Self::TcpConnect | Self::TcpListen)
    }
}

fn contract(kind: SocketKind) -> &'static conduit_core::NodeContract<'static> {
    conduit_std::standard_node_contract(kind.contract_id()).expect("socket contract is published")
}

fn resolution_error(node: &Node, detail: &str) -> ResolutionError {
    ResolutionError::new(
        "CND-SOCK-010",
        format!("socket node `{}` {detail}", node.id),
    )
}

fn runtime_error(code: &'static str, detail: impl Into<String>) -> RuntimeError {
    RuntimeError::new(code, detail)
}

fn required_usize(node: &Node, key: &str) -> Result<usize, ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => usize::try_from(*value)
            .map_err(|_| resolution_error(node, "has an invalid finite bound")),
        _ => Err(resolution_error(node, "is missing a finite bound")),
    }
}

fn required_u64(node: &Node, key: &str) -> Result<u64, ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => {
            u64::try_from(*value).map_err(|_| resolution_error(node, "has an invalid finite bound"))
        }
        _ => Err(resolution_error(node, "is missing a finite bound")),
    }
}

fn exact_secret(node: &Node, key: &str, expected: &str) -> bool {
    matches!(
        node.config_value(key),
        Some(SourceValue::SecretReference(value)) if value == expected
    )
}

fn validate(kind: SocketKind, node: &Node) -> Result<(), ResolutionError> {
    let expected = kind.expected_keys();
    if node.config.len() != expected.len()
        || expected
            .iter()
            .any(|key| !node.config.iter().any(|entry| entry.key == *key))
        || !exact_secret(node, "network_resource", LOOPBACK_NETWORK_RESOURCE)
        || !exact_secret(node, kind.grant_key(), kind.grant())
        || node.config("local") != Some(LOOPBACK_ADDRESS)
        || matches!(kind, SocketKind::TcpConnect | SocketKind::UdpConnected)
            && node.config("peer") != Some(LOOPBACK_ADDRESS)
        || !matches!(
            node.config("cancellation"),
            Some("none" | "cancel-before-commit" | "cancel-after-commit")
        )
    {
        return Err(resolution_error(
            node,
            "does not match the exact numeric-loopback provider profile",
        ));
    }

    let pending = required_usize(node, "maximum_pending_operations")?;
    let send = required_usize(node, "maximum_send_bytes")?;
    let receive = required_usize(node, "maximum_receive_bytes")?;
    let queue = required_usize(node, "maximum_queue_bytes")?;
    let timers = required_usize(node, "maximum_timers")?;
    let work = required_usize(node, "maximum_work")?;
    let evidence = required_usize(node, "maximum_evidence_events")?;
    let deadline = required_u64(node, "deadline_ticks")?;
    let cleanup = required_u64(node, "cleanup_ticks")?;
    if pending == 0
        || pending > 4
        || send == 0
        || send > SOCKET_MAX_STREAM_BYTES
        || receive == 0
        || receive > SOCKET_MAX_STREAM_BYTES
        || queue == 0
        || queue > SOCKET_MAX_STREAM_BYTES
        || timers == 0
        || timers > 2
        || work == 0
        || evidence == 0
        || evidence > SOCKET_MAX_EVIDENCE_EVENTS
        || deadline == 0
        || deadline > MAXIMUM_DEADLINE_MILLIS
        || cleanup == 0
        || cleanup > deadline
    {
        return Err(resolution_error(
            node,
            "exceeds the installed finite provider limits",
        ));
    }

    if kind.is_tcp() {
        let sessions = required_usize(node, "maximum_sessions")?;
        let chunk = required_usize(node, "maximum_chunk_bytes")?;
        if sessions == 0
            || sessions > SOCKET_MAX_SESSIONS
            || chunk == 0
            || chunk > SOCKET_MAX_MESSAGE_BYTES
        {
            return Err(resolution_error(node, "exceeds the installed TCP limits"));
        }
        if kind == SocketKind::TcpListen {
            let backlog = required_usize(node, "backlog")?;
            let accepts = required_usize(node, "maximum_accepts")?;
            if backlog == 0 || backlog > sessions || accepts == 0 || accepts > sessions {
                return Err(resolution_error(
                    node,
                    "exceeds the installed accept limits",
                ));
            }
        }
    } else {
        let mtu = required_usize(node, "path_mtu_bytes")?;
        let message = required_usize(node, "maximum_message_bytes")?;
        let messages = required_usize(node, "maximum_queued_messages")?;
        if mtu == 0
            || mtu > SOCKET_MAX_MESSAGE_BYTES
            || message == 0
            || message > SOCKET_MAX_MESSAGE_BYTES
            || messages == 0
            || messages > SOCKET_MAX_DATAGRAMS
            || !matches!(
                node.config_value("fragmentation"),
                Some(SourceValue::Boolean(_))
            )
        {
            return Err(resolution_error(node, "exceeds the installed UDP limits"));
        }
    }
    Ok(())
}

fn collect_payload(
    node: &Node,
    inputs: &[Value],
    expected_type: conduit_core::TypeContractRef<'static>,
    maximum_chunk: usize,
) -> Result<Vec<u8>, RuntimeError> {
    let maximum = required_usize(node, "maximum_send_bytes")
        .map_err(|error| runtime_error(error.code, error.message))?;
    let mut payload = Vec::new();
    for input in inputs
        .iter()
        .filter(|input| input.value_type == expected_type)
    {
        if input.bytes.len() > maximum_chunk
            || payload.len().saturating_add(input.bytes.len()) > maximum
        {
            return Err(runtime_error(
                "CND-SOCK-011",
                "socket input exceeds its exact byte ceiling",
            ));
        }
        payload.extend_from_slice(&input.bytes);
    }
    Ok(payload)
}

fn timeout(node: &Node) -> Result<Duration, RuntimeError> {
    required_u64(node, "deadline_ticks")
        .map(Duration::from_millis)
        .map_err(|error| runtime_error(error.code, error.message))
}

fn tcp_exchange(
    kind: SocketKind,
    payload: Vec<u8>,
    maximum_receive: usize,
    timeout: Duration,
) -> Result<Vec<u8>, RuntimeError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| runtime_error("CND-SOCK-012", format!("loopback bind failed: {error}")))?;
    let address = listener.local_addr().map_err(|error| {
        runtime_error(
            "CND-SOCK-012",
            format!("loopback lease observation failed: {error}"),
        )
    })?;
    if kind == SocketKind::TcpConnect {
        let peer = thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut received = Vec::new();
            stream.read_to_end(&mut received)?;
            stream.write_all(&received)?;
            stream.shutdown(Shutdown::Write)
        });
        let mut stream = TcpStream::connect(address).map_err(|error| {
            runtime_error("CND-SOCK-013", format!("loopback connect failed: {error}"))
        })?;
        stream.set_read_timeout(Some(timeout)).map_err(|error| {
            runtime_error("CND-SOCK-014", format!("read deadline failed: {error}"))
        })?;
        stream.write_all(&payload).map_err(|error| {
            runtime_error("CND-SOCK-015", format!("loopback write failed: {error}"))
        })?;
        stream.shutdown(Shutdown::Write).map_err(|error| {
            runtime_error("CND-SOCK-015", format!("write half-close failed: {error}"))
        })?;
        let mut received = Vec::new();
        stream
            .take(u64::try_from(maximum_receive).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut received)
            .map_err(|error| {
                runtime_error("CND-SOCK-016", format!("loopback read failed: {error}"))
            })?;
        peer.join()
            .map_err(|_| runtime_error("CND-SOCK-017", "loopback peer panicked"))?
            .map_err(|error| {
                runtime_error("CND-SOCK-017", format!("loopback peer failed: {error}"))
            })?;
        if received.len() > maximum_receive {
            return Err(runtime_error(
                "CND-SOCK-018",
                "TCP receive exceeded its exact byte ceiling",
            ));
        }
        Ok(received)
    } else {
        let client = thread::spawn(move || -> std::io::Result<()> {
            let mut stream = TcpStream::connect(address)?;
            stream.write_all(&payload)?;
            stream.shutdown(Shutdown::Write)?;
            let mut echoed = Vec::new();
            stream.read_to_end(&mut echoed)?;
            Ok(())
        });
        let (mut stream, _) = listener.accept().map_err(|error| {
            runtime_error("CND-SOCK-019", format!("loopback accept failed: {error}"))
        })?;
        stream.set_read_timeout(Some(timeout)).map_err(|error| {
            runtime_error(
                "CND-SOCK-014",
                format!("accept read deadline failed: {error}"),
            )
        })?;
        let mut received = Vec::new();
        Read::by_ref(&mut stream)
            .take(u64::try_from(maximum_receive).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut received)
            .map_err(|error| {
                runtime_error("CND-SOCK-016", format!("accepted read failed: {error}"))
            })?;
        if received.len() > maximum_receive {
            return Err(runtime_error(
                "CND-SOCK-018",
                "TCP receive exceeded its exact byte ceiling",
            ));
        }
        stream.write_all(&received).map_err(|error| {
            runtime_error("CND-SOCK-015", format!("accepted write failed: {error}"))
        })?;
        stream.shutdown(Shutdown::Write).map_err(|error| {
            runtime_error(
                "CND-SOCK-015",
                format!("accepted half-close failed: {error}"),
            )
        })?;
        client
            .join()
            .map_err(|_| runtime_error("CND-SOCK-017", "loopback client panicked"))?
            .map_err(|error| {
                runtime_error("CND-SOCK-017", format!("loopback client failed: {error}"))
            })?;
        Ok(received)
    }
}

fn udp_exchange(
    kind: SocketKind,
    payload: &[u8],
    maximum_receive: usize,
    timeout: Duration,
) -> Result<Vec<u8>, RuntimeError> {
    let first = UdpSocket::bind(("127.0.0.1", 0))
        .map_err(|error| runtime_error("CND-SOCK-012", format!("UDP bind failed: {error}")))?;
    let second = UdpSocket::bind(("127.0.0.1", 0))
        .map_err(|error| runtime_error("CND-SOCK-012", format!("UDP peer bind failed: {error}")))?;
    first.set_read_timeout(Some(timeout)).map_err(|error| {
        runtime_error("CND-SOCK-014", format!("UDP read deadline failed: {error}"))
    })?;
    second.set_read_timeout(Some(timeout)).map_err(|error| {
        runtime_error("CND-SOCK-014", format!("UDP peer deadline failed: {error}"))
    })?;
    let first_address = first
        .local_addr()
        .map_err(|error| runtime_error("CND-SOCK-012", error.to_string()))?;
    let second_address = second
        .local_addr()
        .map_err(|error| runtime_error("CND-SOCK-012", error.to_string()))?;
    if kind == SocketKind::UdpConnected {
        first
            .connect(second_address)
            .and_then(|()| second.connect(first_address))
            .map_err(|error| {
                runtime_error("CND-SOCK-013", format!("UDP connect failed: {error}"))
            })?;
        first
            .send(payload)
            .map_err(|error| runtime_error("CND-SOCK-015", format!("UDP send failed: {error}")))?;
    } else {
        first.send_to(payload, second_address).map_err(|error| {
            runtime_error("CND-SOCK-015", format!("UDP send-to failed: {error}"))
        })?;
    }
    let mut peer_buffer = vec![0; maximum_receive.saturating_add(1)];
    let (peer_length, source) = second.recv_from(&mut peer_buffer).map_err(|error| {
        runtime_error("CND-SOCK-016", format!("UDP receive-from failed: {error}"))
    })?;
    if peer_length > maximum_receive {
        return Err(runtime_error(
            "CND-SOCK-018",
            "UDP receive exceeded its exact byte ceiling",
        ));
    }
    if kind == SocketKind::UdpConnected {
        second.send(&peer_buffer[..peer_length])
    } else {
        second.send_to(&peer_buffer[..peer_length], source)
    }
    .map_err(|error| runtime_error("CND-SOCK-015", format!("UDP echo failed: {error}")))?;
    let mut received = vec![0; maximum_receive.saturating_add(1)];
    let length = first
        .recv(&mut received)
        .map_err(|error| runtime_error("CND-SOCK-016", format!("UDP response failed: {error}")))?;
    if length > maximum_receive {
        return Err(runtime_error(
            "CND-SOCK-018",
            "UDP response exceeded its exact byte ceiling",
        ));
    }
    received.truncate(length);
    Ok(received)
}

struct SocketHandler(SocketKind);

impl Handler for SocketHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate(self.0, node).map_err(|error| runtime_error(error.code, error.message))?;
        if node.config("cancellation") == Some("cancel-before-commit") {
            return Err(runtime_error(
                "CND-SOCK-020",
                "socket cancelled before any network effect",
            ));
        }
        let contract = contract(self.0);
        let maximum_unit = required_usize(
            node,
            if self.0.is_tcp() {
                "maximum_chunk_bytes"
            } else {
                "maximum_message_bytes"
            },
        )
        .map_err(|error| runtime_error(error.code, error.message))?;
        let payload = collect_payload(node, inputs, contract.inputs[0].value_type, maximum_unit)?;
        let maximum_receive = required_usize(node, "maximum_receive_bytes")
            .map_err(|error| runtime_error(error.code, error.message))?;
        let timeout = timeout(node)?;
        if node.config("cancellation") == Some("cancel-after-commit") {
            if self.0.is_tcp() {
                let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| {
                    runtime_error("CND-SOCK-012", format!("loopback bind failed: {error}"))
                })?;
                drop(listener);
            } else {
                let socket = UdpSocket::bind(("127.0.0.1", 0)).map_err(|error| {
                    runtime_error("CND-SOCK-012", format!("UDP bind failed: {error}"))
                })?;
                drop(socket);
            }
            return Err(runtime_error(
                "CND-SOCK-020",
                "socket cancelled after commit; resource cleanup completed",
            ));
        }
        let received = if self.0.is_tcp() {
            tcp_exchange(self.0, payload, maximum_receive, timeout)?
        } else {
            let mtu = required_usize(node, "path_mtu_bytes")
                .map_err(|error| runtime_error(error.code, error.message))?;
            let fragmentation = matches!(
                node.config_value("fragmentation"),
                Some(SourceValue::Boolean(true))
            );
            if payload.len() > mtu && !fragmentation {
                return Err(runtime_error(
                    "CND-SOCK-021",
                    "UDP datagram exceeds the exact unfragmented MTU",
                ));
            }
            udp_exchange(self.0, &payload, maximum_receive, timeout)?
        };
        let mut result = Vec::with_capacity(if self.0.is_tcp() { 3 } else { 2 });
        result.push(Value {
            value_type: contract.outputs[0].value_type,
            bytes: received,
        });
        if self.0.is_tcp() {
            result.push(Value {
                value_type: contract.outputs[1].value_type,
                bytes: 1_u64.to_le_bytes().to_vec(),
            });
        }
        result.push(Value {
            value_type: contract.outputs[if self.0.is_tcp() { 2 } else { 1 }].value_type,
            bytes: vec![0],
        });
        Ok(result)
    }
}

fn tcp_connect_handler() -> Box<dyn Handler> {
    Box::new(SocketHandler(SocketKind::TcpConnect))
}
fn tcp_listen_handler() -> Box<dyn Handler> {
    Box::new(SocketHandler(SocketKind::TcpListen))
}
fn udp_connected_handler() -> Box<dyn Handler> {
    Box::new(SocketHandler(SocketKind::UdpConnected))
}
fn udp_datagram_handler() -> Box<dyn Handler> {
    Box::new(SocketHandler(SocketKind::UdpDatagram))
}
fn validate_tcp_connect(node: &Node) -> Result<(), ResolutionError> {
    validate(SocketKind::TcpConnect, node)
}
fn validate_tcp_listen(node: &Node) -> Result<(), ResolutionError> {
    validate(SocketKind::TcpListen, node)
}
fn validate_udp_connected(node: &Node) -> Result<(), ResolutionError> {
    validate(SocketKind::UdpConnected, node)
}
fn validate_udp_datagram(node: &Node) -> Result<(), ResolutionError> {
    validate(SocketKind::UdpDatagram, node)
}

/// Installs all four bounded numeric-loopback providers as one explicit host profile.
pub fn register_hosted_socket_providers(registry: &mut Registry) -> Result<(), RegistryError> {
    static TCP_CONNECT_AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([0x61; 32])];
    static TCP_LISTEN_AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([0x62; 32])];
    static UDP_CONNECTED_AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([0x63; 32])];
    static UDP_DATAGRAM_AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([0x64; 32])];
    for service in [
        CompiledInHostService {
            contract: contract(SocketKind::TcpConnect),
            implementation_id: "conduit/socket-linux-tcp-connect",
            artifact_id: "conduit/socket-linux-tcp-connect-artifact",
            entrypoint: "socket-linux-tcp-connect",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &TCP_CONNECT_AUTHORITY,
            factory: tcp_connect_handler,
            validate_config: validate_tcp_connect,
        },
        CompiledInHostService {
            contract: contract(SocketKind::TcpListen),
            implementation_id: "conduit/socket-linux-tcp-listen",
            artifact_id: "conduit/socket-linux-tcp-listen-artifact",
            entrypoint: "socket-linux-tcp-listen",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &TCP_LISTEN_AUTHORITY,
            factory: tcp_listen_handler,
            validate_config: validate_tcp_listen,
        },
        CompiledInHostService {
            contract: contract(SocketKind::UdpConnected),
            implementation_id: "conduit/socket-linux-udp-connected",
            artifact_id: "conduit/socket-linux-udp-connected-artifact",
            entrypoint: "socket-linux-udp-connected",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &UDP_CONNECTED_AUTHORITY,
            factory: udp_connected_handler,
            validate_config: validate_udp_connected,
        },
        CompiledInHostService {
            contract: contract(SocketKind::UdpDatagram),
            implementation_id: "conduit/socket-linux-udp-datagram",
            artifact_id: "conduit/socket-linux-udp-datagram-artifact",
            entrypoint: "socket-linux-udp-datagram",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &UDP_DATAGRAM_AUTHORITY,
            factory: udp_datagram_handler,
            validate_config: validate_udp_datagram,
        },
    ] {
        let descriptor = if service.contract.id.as_str() == TCP_LISTEN_ID {
            ManagedComponentDescriptor::full_standing_service(ManagedAdapterBoundary::Native)
        } else {
            ManagedComponentDescriptor::leased_provider(ManagedAdapterBoundary::Native)
        };
        registry.register_managed_compiled_in_host_service(service, descriptor)?;
    }
    Ok(())
}

/// Redacted host facts; ephemeral ports and grants are never rendered.
#[must_use]
pub fn provider_description() -> Vec<(&'static str, String)> {
    vec![
        ("address_family", "numeric-ipv4-loopback".to_owned()),
        ("dns", "unsupported".to_owned()),
        ("firewall_mutation", "unsupported".to_owned()),
        ("public_reachability", "not-claimed".to_owned()),
        ("tls", "unsupported".to_owned()),
        ("maximum_sessions", SOCKET_MAX_SESSIONS.to_string()),
        ("maximum_datagrams", SOCKET_MAX_DATAGRAMS.to_string()),
        ("maximum_stream_bytes", SOCKET_MAX_STREAM_BYTES.to_string()),
        (
            "maximum_message_bytes",
            SOCKET_MAX_MESSAGE_BYTES.to_string(),
        ),
        ("network_resource", "protected".to_owned()),
        ("grant", "protected".to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_tcp_and_udp_loopback_preserve_transport_boundaries() {
        let timeout = Duration::from_secs(1);
        for kind in [SocketKind::TcpConnect, SocketKind::TcpListen] {
            assert_eq!(
                tcp_exchange(kind, b"stream".to_vec(), 64, timeout).unwrap(),
                b"stream"
            );
        }
        for kind in [SocketKind::UdpConnected, SocketKind::UdpDatagram] {
            assert_eq!(
                udp_exchange(kind, b"datagram", 64, timeout).unwrap(),
                b"datagram"
            );
        }
    }

    #[test]
    fn provider_registration_is_explicit_and_descriptions_make_no_network_claims() {
        let mut registry = Registry::hosted_primitives();
        assert_eq!(
            registry.node_availability(TCP_CONNECT_ID).state,
            conduit_runtime::AvailabilityState::ContractOnly
        );
        register_hosted_socket_providers(&mut registry).unwrap();
        for id in [
            TCP_CONNECT_ID,
            TCP_LISTEN_ID,
            UDP_CONNECTED_ID,
            UDP_DATAGRAM_ID,
        ] {
            assert_eq!(
                registry.node_availability(id).state,
                conduit_runtime::AvailabilityState::ProviderAvailable
            );
        }
        let installed = registry
            .installed_providers()
            .into_iter()
            .filter(|provider| {
                matches!(
                    provider.contract.id.as_str(),
                    TCP_CONNECT_ID | TCP_LISTEN_ID | UDP_CONNECTED_ID | UDP_DATAGRAM_ID
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(installed.len(), 4);
        assert!(installed.iter().all(|provider| {
            provider.managed_lifecycle.is_some()
                && provider.manifest.provided_interfaces[0]
                    .interface
                    .id
                    .as_str()
                    == conduit_runtime::MANAGED_COMPONENT_INTERFACE_ID
                && provider.manifest.provided_interfaces[0]
                    .interface
                    .semantic_hash
                    == conduit_runtime::managed_component_interface_hash()
        }));
        let description = provider_description();
        assert!(description.contains(&("dns", "unsupported".to_owned())));
        assert!(description.contains(&("public_reachability", "not-claimed".to_owned())));
        let rendered = format!("{description:?}");
        assert!(!rendered.contains(TCP_CONNECT_GRANT));
        assert!(!rendered.contains(TCP_LISTEN_GRANT));
        assert!(!rendered.contains(UDP_CONNECTED_GRANT));
        assert!(!rendered.contains(UDP_DATAGRAM_GRANT));
    }
}
