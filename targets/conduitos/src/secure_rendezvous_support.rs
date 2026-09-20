//! ConduitOS consumer for one direct authenticated LAN rendezvous candidate.
//!
//! The shared CBOR descriptor owns candidate semantics and the one-use
//! capability. This module only realizes an admitted `AuthenticatedTlsStream`
//! candidate through the bounded VirtIO TCP/TLS/WebSocket Base.

use conduit_body::{
    RendezvousCborRefusal, RendezvousLineFamily, decode_running_host_rendezvous_cbor,
};

use crate::{
    arch::VirtioNetReady,
    bounded_websocket::{BinaryWebSocketIo, MAXIMUM_BINARY_MESSAGE_BYTES, WebSocketError},
    virtio_tcp::VirtioTcpEndpoint,
    virtio_tls::{self, VirtioTlsError, VirtioWebSocketRunError},
    wss_candidate_support::{certificate_matches, locator_matches},
};

pub const AUTHENTICATED_TLS_LINE_IMPLEMENTATION_ID: &str =
    "conduit-line/authenticated-tls-stream@1";

#[derive(Debug, PartialEq, Eq)]
pub enum ConduitOsTlsRendezvousRefusal<E> {
    Descriptor(RendezvousCborRefusal),
    CandidateMissing,
    UnsupportedLineFamily,
    EndpointBinding,
    CertificateBinding,
    Transport(VirtioTlsError),
    Operation(E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitOsRendezvousLineError {
    Pressure,
    Lost,
    Closed,
}

pub trait ConduitOsRendezvousLineIo {
    fn send(&mut self, frame: &[u8]) -> Result<(), ConduitOsRendezvousLineError>;
    fn receive(&mut self, output: &mut [u8]) -> Result<usize, ConduitOsRendezvousLineError>;
    fn close(&mut self) -> Result<(), ConduitOsRendezvousLineError>;
}

pub struct ConduitOsTlsRendezvousContext<'a> {
    pub candidate_id: &'a str,
    pub line_implementation_id: &'static str,
    pub session_secret: &'a [u8; 32],
}

/// Consume one exact shared descriptor candidate without allowing its socket,
/// TLS session, WebSocket, or capability to escape the operation closure.
#[allow(clippy::too_many_arguments)]
pub fn with_conduitos_tls_rendezvous<T, E>(
    device: VirtioNetReady,
    tcp_seed: u64,
    tls_seed: [u8; 32],
    websocket_seed: [u8; 32],
    endpoint: VirtioTcpEndpoint,
    pinned_certificate_der: &[u8],
    maximum_polls: u32,
    encoded_descriptor: &[u8],
    candidate_id: &str,
    now_millis: u64,
    operation: impl FnOnce(
        &mut dyn ConduitOsRendezvousLineIo,
        ConduitOsTlsRendezvousContext<'_>,
    ) -> Result<T, E>,
) -> Result<(T, u32), ConduitOsTlsRendezvousRefusal<E>> {
    let descriptor = decode_running_host_rendezvous_cbor(encoded_descriptor, now_millis)
        .map_err(ConduitOsTlsRendezvousRefusal::Descriptor)?;
    let candidate = descriptor
        .candidates()
        .find(|candidate| candidate.candidate_id == candidate_id)
        .ok_or(ConduitOsTlsRendezvousRefusal::CandidateMissing)?;
    if candidate.line_family != RendezvousLineFamily::AuthenticatedTlsStream {
        return Err(ConduitOsTlsRendezvousRefusal::UnsupportedLineFamily);
    }
    if !locator_matches(
        candidate.reachability,
        candidate.server_identity,
        endpoint.remote_port,
    ) {
        return Err(ConduitOsTlsRendezvousRefusal::EndpointBinding);
    }
    if !certificate_matches(candidate.transport_binding_sha256, pinned_certificate_der) {
        return Err(ConduitOsTlsRendezvousRefusal::CertificateBinding);
    }
    let attempt_polls = maximum_polls.min(candidate.attempt_timeout_millis);
    if attempt_polls == 0 {
        return Err(ConduitOsTlsRendezvousRefusal::EndpointBinding);
    }

    let mut secret = descriptor.copy_session_secret_for_attempt();
    let result = virtio_tls::with_websocket(
        device,
        tcp_seed,
        tls_seed,
        websocket_seed,
        endpoint,
        candidate.server_identity,
        pinned_certificate_der,
        attempt_polls,
        |websocket| {
            let mut line = TlsRendezvousLine {
                websocket,
                closed: false,
            };
            let context = ConduitOsTlsRendezvousContext {
                candidate_id: candidate.candidate_id,
                line_implementation_id: AUTHENTICATED_TLS_LINE_IMPLEMENTATION_ID,
                session_secret: &secret,
            };
            operation(&mut line, context).map_err(TlsRendezvousOperationRefusal::Operation)
        },
    );
    erase(&mut secret);
    result.map_err(|error| match error {
        VirtioWebSocketRunError::Transport(error) => {
            ConduitOsTlsRendezvousRefusal::Transport(error)
        }
        VirtioWebSocketRunError::Operation(TlsRendezvousOperationRefusal::Operation(error)) => {
            ConduitOsTlsRendezvousRefusal::Operation(error)
        }
    })
}

// Keep the operation refusal distinct while it crosses the transport helper;
// transport loss and caller refusal remain separately machine-readable.
enum TlsRendezvousOperationRefusal<E> {
    Operation(E),
}

struct TlsRendezvousLine<'a> {
    websocket: &'a mut dyn BinaryWebSocketIo,
    closed: bool,
}

impl ConduitOsRendezvousLineIo for TlsRendezvousLine<'_> {
    fn send(&mut self, frame: &[u8]) -> Result<(), ConduitOsRendezvousLineError> {
        if self.closed {
            return Err(ConduitOsRendezvousLineError::Closed);
        }
        if frame.is_empty() || frame.len() > MAXIMUM_BINARY_MESSAGE_BYTES {
            return Err(ConduitOsRendezvousLineError::Pressure);
        }
        self.websocket.send_binary(frame).map_err(map_websocket)
    }

    fn receive(&mut self, output: &mut [u8]) -> Result<usize, ConduitOsRendezvousLineError> {
        if self.closed {
            return Err(ConduitOsRendezvousLineError::Closed);
        }
        if output.is_empty() || output.len() > MAXIMUM_BINARY_MESSAGE_BYTES {
            return Err(ConduitOsRendezvousLineError::Pressure);
        }
        self.websocket.receive_binary(output).map_err(map_websocket)
    }

    fn close(&mut self) -> Result<(), ConduitOsRendezvousLineError> {
        if self.closed {
            return Err(ConduitOsRendezvousLineError::Closed);
        }
        self.closed = true;
        Ok(())
    }
}

fn map_websocket(error: WebSocketError) -> ConduitOsRendezvousLineError {
    match error {
        WebSocketError::RequestTooLarge | WebSocketError::ResponseTooLarge => {
            ConduitOsRendezvousLineError::Pressure
        }
        _ => ConduitOsRendezvousLineError::Lost,
    }
}

fn erase(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: every byte is exclusively borrowed and valid for a volatile write.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}

#[cfg(test)]
mod tests;
