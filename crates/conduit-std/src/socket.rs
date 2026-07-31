//! Allocator-free semantics for optional bounded application sockets.
//!
//! This module performs no DNS, interface, route, firewall, TLS, or OS socket
//! operation. Exact planning owns those bindings and observations.

pub const SOCKET_MAX_STREAM_BYTES: usize = 65_536;
pub const SOCKET_MAX_MESSAGE_BYTES: usize = 4_096;
pub const SOCKET_MAX_SESSIONS: usize = 8;
pub const SOCKET_MAX_DATAGRAMS: usize = 16;
pub const SOCKET_MAX_EVIDENCE_EVENTS: usize = 128;

/// Host-language-neutral endpoint value, scoped by an opaque network resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SocketAddress {
    pub network_resource: u32,
    pub address: [u8; 16],
    pub port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SocketSession(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TcpOperation {
    Connect,
    ListenAccept,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UdpOperation {
    Connected,
    Unconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SocketLimits {
    pub maximum_sessions: usize,
    pub maximum_pending_operations: usize,
    pub maximum_send_bytes: usize,
    pub maximum_receive_bytes: usize,
    pub maximum_message_bytes: usize,
    pub maximum_queued_messages: usize,
    pub maximum_queue_bytes: usize,
    pub maximum_timers: usize,
    pub maximum_work: usize,
    pub maximum_evidence_events: usize,
    pub deadline_ticks: u64,
    pub cleanup_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TcpRequest {
    pub operation: TcpOperation,
    pub local: SocketAddress,
    pub peer: Option<SocketAddress>,
    pub backlog: usize,
    pub limits: SocketLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UdpRequest {
    pub operation: UdpOperation,
    pub local: SocketAddress,
    pub peer: Option<SocketAddress>,
    pub path_mtu_bytes: usize,
    pub fragmentation: bool,
    pub limits: SocketLimits,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SocketControl {
    pub deny_target: bool,
    pub stale_observation: bool,
    pub refuse: bool,
    pub reset_after_commit: bool,
    pub timeout: bool,
    pub cancel_before_commit: bool,
    pub cancel_after_commit: bool,
    pub provider_loss_after_commit: bool,
    pub duplicate_datagram: bool,
    pub reorder_datagrams: bool,
    pub drop_datagram: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketEventKind {
    Admitted,
    ConnectCommitted,
    ListenCommitted,
    Accepted,
    SendCommitted,
    ReceiveObserved,
    WriteHalfClosed,
    ReadEof,
    DatagramDropped,
    DatagramDuplicated,
    DatagramReordered,
    Cancelled,
    ProviderLost,
    CleanupComplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SocketEvent {
    pub sequence: u16,
    pub tick: u64,
    pub kind: SocketEventKind,
    pub bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketTerminal {
    Completed,
    Refused,
    Reset,
    TimedOut,
    Cancelled,
    ProviderLost,
    SendOverflow,
    ReceiveOverflow,
    WorkExhausted,
    DatagramOversized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SocketResult {
    pub session: SocketSession,
    pub terminal: SocketTerminal,
    pub sent_bytes: usize,
    pub received_bytes: usize,
    pub received_messages: usize,
    pub evidence_events: usize,
    pub cleanup_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketError {
    InvalidAddress,
    MissingPeer,
    UnexpectedPeer,
    InvalidLimits,
    TargetDenied,
    StaleObservation,
    CancelledBeforeCommit,
    EvidenceOverflow,
}

impl SocketError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidAddress => "CND-SOCK-001",
            Self::MissingPeer => "CND-SOCK-002",
            Self::UnexpectedPeer => "CND-SOCK-003",
            Self::InvalidLimits => "CND-SOCK-004",
            Self::TargetDenied => "CND-SOCK-005",
            Self::StaleObservation => "CND-SOCK-006",
            Self::CancelledBeforeCommit => "CND-SOCK-007",
            Self::EvidenceOverflow => "CND-SOCK-008",
        }
    }
}

fn validate_address(value: SocketAddress) -> Result<(), SocketError> {
    if value.network_resource == 0 || value.port == 0 {
        return Err(SocketError::InvalidAddress);
    }
    Ok(())
}

fn validate_limits(value: SocketLimits) -> Result<(), SocketError> {
    if value.maximum_sessions == 0
        || value.maximum_sessions > SOCKET_MAX_SESSIONS
        || value.maximum_pending_operations == 0
        || value.maximum_send_bytes == 0
        || value.maximum_send_bytes > SOCKET_MAX_STREAM_BYTES
        || value.maximum_receive_bytes == 0
        || value.maximum_receive_bytes > SOCKET_MAX_STREAM_BYTES
        || value.maximum_message_bytes == 0
        || value.maximum_message_bytes > SOCKET_MAX_MESSAGE_BYTES
        || value.maximum_queued_messages == 0
        || value.maximum_queued_messages > SOCKET_MAX_DATAGRAMS
        || value.maximum_queue_bytes == 0
        || value.maximum_timers == 0
        || value.maximum_work == 0
        || value.maximum_evidence_events == 0
        || value.maximum_evidence_events > SOCKET_MAX_EVIDENCE_EVENTS
        || value.deadline_ticks == 0
        || value.cleanup_ticks == 0
    {
        return Err(SocketError::InvalidLimits);
    }
    Ok(())
}

pub fn validate_tcp_request(value: TcpRequest) -> Result<(), SocketError> {
    validate_address(value.local)?;
    validate_limits(value.limits)?;
    match value.operation {
        TcpOperation::Connect if value.peer.is_none() => return Err(SocketError::MissingPeer),
        TcpOperation::ListenAccept if value.peer.is_some() => {
            return Err(SocketError::UnexpectedPeer);
        }
        TcpOperation::ListenAccept
            if value.backlog == 0 || value.backlog > value.limits.maximum_sessions =>
        {
            return Err(SocketError::InvalidLimits);
        }
        _ => {}
    }
    if let Some(peer) = value.peer {
        validate_address(peer)?;
    }
    Ok(())
}

pub fn validate_udp_request(value: UdpRequest) -> Result<(), SocketError> {
    validate_address(value.local)?;
    validate_limits(value.limits)?;
    match value.operation {
        UdpOperation::Connected if value.peer.is_none() => return Err(SocketError::MissingPeer),
        UdpOperation::Unconnected if value.peer.is_some() => {
            return Err(SocketError::UnexpectedPeer);
        }
        _ => {}
    }
    if let Some(peer) = value.peer {
        validate_address(peer)?;
    }
    if value.path_mtu_bytes == 0 || value.path_mtu_bytes > value.limits.maximum_message_bytes {
        return Err(SocketError::InvalidLimits);
    }
    Ok(())
}

pub struct SocketBuffers<'a> {
    pub outbound: &'a [u8],
    pub inbound_fixture: &'a [u8],
    pub received: &'a mut [u8],
    pub evidence: &'a mut [SocketEvent],
}

fn pre_effect(control: SocketControl) -> Result<(), SocketError> {
    if control.deny_target {
        Err(SocketError::TargetDenied)
    } else if control.stale_observation {
        Err(SocketError::StaleObservation)
    } else if control.cancel_before_commit {
        Err(SocketError::CancelledBeforeCommit)
    } else {
        Ok(())
    }
}

fn push(
    evidence: &mut [SocketEvent],
    length: &mut usize,
    maximum: usize,
    kind: SocketEventKind,
    bytes: usize,
) -> Result<(), SocketError> {
    if *length >= maximum {
        return Err(SocketError::EvidenceOverflow);
    }
    evidence[*length] = SocketEvent {
        sequence: *length as u16,
        tick: *length as u64,
        kind,
        bytes,
    };
    *length += 1;
    Ok(())
}

pub fn run_tcp_fixture(
    request: TcpRequest,
    buffers: SocketBuffers<'_>,
    control: SocketControl,
) -> Result<SocketResult, SocketError> {
    validate_tcp_request(request)?;
    pre_effect(control)?;
    let maximum = buffers
        .evidence
        .len()
        .min(request.limits.maximum_evidence_events);
    let mut events = 0;
    push(
        buffers.evidence,
        &mut events,
        maximum,
        SocketEventKind::Admitted,
        0,
    )?;
    push(
        buffers.evidence,
        &mut events,
        maximum,
        match request.operation {
            TcpOperation::Connect => SocketEventKind::ConnectCommitted,
            TcpOperation::ListenAccept => SocketEventKind::ListenCommitted,
        },
        0,
    )?;
    if request.operation == TcpOperation::ListenAccept {
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::Accepted,
            0,
        )?;
    }
    let terminal = if control.refuse {
        SocketTerminal::Refused
    } else if control.timeout {
        SocketTerminal::TimedOut
    } else if control.cancel_after_commit {
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::Cancelled,
            0,
        )?;
        SocketTerminal::Cancelled
    } else if control.provider_loss_after_commit {
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::ProviderLost,
            0,
        )?;
        SocketTerminal::ProviderLost
    } else if buffers.outbound.len() > request.limits.maximum_send_bytes {
        SocketTerminal::SendOverflow
    } else if buffers.inbound_fixture.len() > request.limits.maximum_receive_bytes
        || buffers.inbound_fixture.len() > buffers.received.len()
    {
        SocketTerminal::ReceiveOverflow
    } else if buffers
        .outbound
        .len()
        .saturating_add(buffers.inbound_fixture.len())
        > request.limits.maximum_work
    {
        SocketTerminal::WorkExhausted
    } else if control.reset_after_commit {
        SocketTerminal::Reset
    } else {
        buffers.received[..buffers.inbound_fixture.len()].copy_from_slice(buffers.inbound_fixture);
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::SendCommitted,
            buffers.outbound.len(),
        )?;
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::ReceiveObserved,
            buffers.inbound_fixture.len(),
        )?;
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::WriteHalfClosed,
            0,
        )?;
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::ReadEof,
            0,
        )?;
        SocketTerminal::Completed
    };
    push(
        buffers.evidence,
        &mut events,
        maximum,
        SocketEventKind::CleanupComplete,
        0,
    )?;
    Ok(SocketResult {
        session: SocketSession(1),
        terminal,
        sent_bytes: usize::from(terminal == SocketTerminal::Completed) * buffers.outbound.len(),
        received_bytes: usize::from(terminal == SocketTerminal::Completed)
            * buffers.inbound_fixture.len(),
        received_messages: usize::from(terminal == SocketTerminal::Completed),
        evidence_events: events,
        cleanup_complete: true,
    })
}

pub fn run_udp_fixture(
    request: UdpRequest,
    buffers: SocketBuffers<'_>,
    control: SocketControl,
) -> Result<SocketResult, SocketError> {
    validate_udp_request(request)?;
    pre_effect(control)?;
    let maximum = buffers
        .evidence
        .len()
        .min(request.limits.maximum_evidence_events);
    let mut events = 0;
    push(
        buffers.evidence,
        &mut events,
        maximum,
        SocketEventKind::Admitted,
        0,
    )?;
    push(
        buffers.evidence,
        &mut events,
        maximum,
        SocketEventKind::ConnectCommitted,
        0,
    )?;
    let mut received_messages = 0;
    let terminal = if control.timeout {
        SocketTerminal::TimedOut
    } else if buffers.outbound.len() > request.limits.maximum_send_bytes {
        SocketTerminal::SendOverflow
    } else if buffers.outbound.len() > request.path_mtu_bytes && !request.fragmentation {
        SocketTerminal::DatagramOversized
    } else if control.drop_datagram {
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::DatagramDropped,
            0,
        )?;
        SocketTerminal::Completed
    } else if control.cancel_after_commit {
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::Cancelled,
            0,
        )?;
        SocketTerminal::Cancelled
    } else if control.provider_loss_after_commit {
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::ProviderLost,
            0,
        )?;
        SocketTerminal::ProviderLost
    } else if buffers.inbound_fixture.len() > request.limits.maximum_message_bytes
        || buffers.inbound_fixture.len() > request.limits.maximum_receive_bytes
        || buffers.inbound_fixture.len() > request.limits.maximum_queue_bytes
        || buffers.inbound_fixture.len() > buffers.received.len()
    {
        SocketTerminal::ReceiveOverflow
    } else if buffers
        .outbound
        .len()
        .saturating_add(buffers.inbound_fixture.len())
        > request.limits.maximum_work
    {
        SocketTerminal::WorkExhausted
    } else {
        buffers.received[..buffers.inbound_fixture.len()].copy_from_slice(buffers.inbound_fixture);
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::SendCommitted,
            buffers.outbound.len(),
        )?;
        push(
            buffers.evidence,
            &mut events,
            maximum,
            SocketEventKind::ReceiveObserved,
            buffers.inbound_fixture.len(),
        )?;
        received_messages = 1;
        if control.duplicate_datagram {
            push(
                buffers.evidence,
                &mut events,
                maximum,
                SocketEventKind::DatagramDuplicated,
                buffers.inbound_fixture.len(),
            )?;
            received_messages = 2;
        }
        if control.reorder_datagrams {
            push(
                buffers.evidence,
                &mut events,
                maximum,
                SocketEventKind::DatagramReordered,
                0,
            )?;
        }
        SocketTerminal::Completed
    };
    push(
        buffers.evidence,
        &mut events,
        maximum,
        SocketEventKind::CleanupComplete,
        0,
    )?;
    Ok(SocketResult {
        session: SocketSession(1),
        terminal,
        sent_bytes: usize::from(terminal == SocketTerminal::Completed) * buffers.outbound.len(),
        received_bytes: usize::from(received_messages > 0) * buffers.inbound_fixture.len(),
        received_messages,
        evidence_events: events,
        cleanup_complete: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(port: u16) -> SocketAddress {
        SocketAddress {
            network_resource: 1,
            address: [0; 16],
            port,
        }
    }

    fn limits() -> SocketLimits {
        SocketLimits {
            maximum_sessions: 2,
            maximum_pending_operations: 3,
            maximum_send_bytes: 64,
            maximum_receive_bytes: 64,
            maximum_message_bytes: 32,
            maximum_queued_messages: 4,
            maximum_queue_bytes: 128,
            maximum_timers: 2,
            maximum_work: 128,
            maximum_evidence_events: 16,
            deadline_ticks: 10,
            cleanup_ticks: 2,
        }
    }

    #[test]
    fn tcp_and_udp_keep_distinct_terminal_and_delivery_semantics() {
        let tcp = TcpRequest {
            operation: TcpOperation::Connect,
            local: address(1000),
            peer: Some(address(2000)),
            backlog: 0,
            limits: limits(),
        };
        let mut received = [0; 64];
        let mut evidence = [SocketEvent {
            sequence: 0,
            tick: 0,
            kind: SocketEventKind::Admitted,
            bytes: 0,
        }; 16];
        let result = run_tcp_fixture(
            tcp,
            SocketBuffers {
                outbound: b"request",
                inbound_fixture: b"response",
                received: &mut received,
                evidence: &mut evidence,
            },
            SocketControl::default(),
        )
        .unwrap();
        assert_eq!(result.terminal, SocketTerminal::Completed);
        assert_eq!(&received[..result.received_bytes], b"response");

        let udp = UdpRequest {
            operation: UdpOperation::Connected,
            local: address(1000),
            peer: Some(address(2000)),
            path_mtu_bytes: 8,
            fragmentation: false,
            limits: limits(),
        };
        let oversized = run_udp_fixture(
            udp,
            SocketBuffers {
                outbound: b"too-large",
                inbound_fixture: b"",
                received: &mut received,
                evidence: &mut evidence,
            },
            SocketControl::default(),
        )
        .unwrap();
        assert_eq!(oversized.terminal, SocketTerminal::DatagramOversized);
    }

    #[test]
    fn validation_and_cancellation_are_pre_effect() {
        let request = TcpRequest {
            operation: TcpOperation::Connect,
            local: address(1000),
            peer: None,
            backlog: 0,
            limits: limits(),
        };
        assert_eq!(validate_tcp_request(request), Err(SocketError::MissingPeer));
        let valid = TcpRequest {
            peer: Some(address(2000)),
            ..request
        };
        let mut received = [0; 1];
        let mut evidence = [SocketEvent {
            sequence: 0,
            tick: 0,
            kind: SocketEventKind::Admitted,
            bytes: 0,
        }; 1];
        assert_eq!(
            run_tcp_fixture(
                valid,
                SocketBuffers {
                    outbound: b"",
                    inbound_fixture: b"",
                    received: &mut received,
                    evidence: &mut evidence,
                },
                SocketControl {
                    cancel_before_commit: true,
                    ..SocketControl::default()
                }
            ),
            Err(SocketError::CancelledBeforeCommit)
        );
    }

    #[test]
    fn tcp_listener_and_failures_are_normalized() {
        let request = TcpRequest {
            operation: TcpOperation::ListenAccept,
            local: address(1000),
            peer: None,
            backlog: 2,
            limits: limits(),
        };
        for (control, terminal) in [
            (
                SocketControl {
                    refuse: true,
                    ..SocketControl::default()
                },
                SocketTerminal::Refused,
            ),
            (
                SocketControl {
                    reset_after_commit: true,
                    ..SocketControl::default()
                },
                SocketTerminal::Reset,
            ),
            (
                SocketControl {
                    timeout: true,
                    ..SocketControl::default()
                },
                SocketTerminal::TimedOut,
            ),
            (
                SocketControl {
                    provider_loss_after_commit: true,
                    ..SocketControl::default()
                },
                SocketTerminal::ProviderLost,
            ),
        ] {
            let mut received = [0; 64];
            let mut evidence = [SocketEvent {
                sequence: 0,
                tick: 0,
                kind: SocketEventKind::Admitted,
                bytes: 0,
            }; 16];
            let result = run_tcp_fixture(
                request,
                SocketBuffers {
                    outbound: b"request",
                    inbound_fixture: b"response",
                    received: &mut received,
                    evidence: &mut evidence,
                },
                control,
            )
            .unwrap();
            assert_eq!(result.terminal, terminal);
            assert!(result.cleanup_complete);
            assert_eq!(evidence[1].kind, SocketEventKind::ListenCommitted);
            assert_eq!(evidence[2].kind, SocketEventKind::Accepted);
        }
    }

    #[test]
    fn udp_preserves_loss_duplicate_reorder_and_unconnected_rules() {
        let request = UdpRequest {
            operation: UdpOperation::Unconnected,
            local: address(1000),
            peer: None,
            path_mtu_bytes: 16,
            fragmentation: false,
            limits: limits(),
        };
        let mut received = [0; 64];
        let mut evidence = [SocketEvent {
            sequence: 0,
            tick: 0,
            kind: SocketEventKind::Admitted,
            bytes: 0,
        }; 16];
        let result = run_udp_fixture(
            request,
            SocketBuffers {
                outbound: b"one",
                inbound_fixture: b"two",
                received: &mut received,
                evidence: &mut evidence,
            },
            SocketControl {
                duplicate_datagram: true,
                reorder_datagrams: true,
                ..SocketControl::default()
            },
        )
        .unwrap();
        assert_eq!(result.terminal, SocketTerminal::Completed);
        assert_eq!(result.received_messages, 2);
        assert!(
            evidence[..result.evidence_events]
                .iter()
                .any(|event| event.kind == SocketEventKind::DatagramDuplicated)
        );
        assert!(
            evidence[..result.evidence_events]
                .iter()
                .any(|event| event.kind == SocketEventKind::DatagramReordered)
        );

        let invalid = UdpRequest {
            peer: Some(address(2000)),
            ..request
        };
        assert_eq!(
            validate_udp_request(invalid),
            Err(SocketError::UnexpectedPeer)
        );
    }
}
