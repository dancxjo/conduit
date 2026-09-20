//! ConduitOS admission for the shared protected Line session.
//!
//! This adapter supplies only fresh ephemeral key material and finite session
//! storage. The caller separately owns the carrier, peer/session binding,
//! rendezvous PSK, admission policy, and any membership or effect authority.

use conduit_protected_line::{
    IMPLEMENTATION_ID, ProtectedCarrier, ProtectedFrameCarrier, ProtectedLineError,
    ProtectedSessionPolicy, Role, SessionBinding, establish_protected_session,
};

use crate::cryptographic_entropy::{
    CryptographicEntropyBase, CryptographicEntropySource, EntropyRefusal,
};
#[cfg(feature = "virtio-net-proof")]
use crate::{
    arch::VirtioNetReady,
    bounded_websocket::{BinaryWebSocketIo, WebSocketError},
    virtio_tcp::VirtioTcpEndpoint,
    virtio_tls::{self, VirtioTlsError, VirtioWebSocketRunError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitOsProtectedLineRefusal {
    Entropy(EntropyRefusal),
    Session(ProtectedLineError),
}

#[cfg(feature = "virtio-net-proof")]
#[derive(Debug, PartialEq, Eq)]
pub enum ConduitOsProtectedWebSocketRefusal<E> {
    Transport(VirtioTlsError),
    Protected(ConduitOsProtectedLineRefusal),
    Operation(E),
}

/// Narrow type-erased session boundary presented to ConduitOS protocol code.
/// The outer socket, TLS, WebSocket, and closure lifetimes cannot escape it.
pub trait ConduitOsProtectedLineIo {
    fn send(&mut self, payload: &[u8]) -> Result<(), ProtectedLineError>;
    fn receive(&mut self) -> Result<&[u8], ProtectedLineError>;
    fn close(&mut self) -> Result<(), ProtectedLineError>;
}

impl<C: ProtectedFrameCarrier> ConduitOsProtectedLineIo for ProtectedCarrier<C> {
    fn send(&mut self, payload: &[u8]) -> Result<(), ProtectedLineError> {
        ProtectedCarrier::send(self, payload)
    }

    fn receive(&mut self) -> Result<&[u8], ProtectedLineError> {
        ProtectedCarrier::receive(self)
    }

    fn close(&mut self) -> Result<(), ProtectedLineError> {
        ProtectedCarrier::close(self)
    }
}

impl ConduitOsProtectedLineRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entropy(error) => error.as_str(),
            Self::Session(_) => "protected-line-session-refused",
        }
    }
}

pub const fn protected_line_implementation_id() -> &'static str {
    IMPLEMENTATION_ID
}

/// Establish one admitted protected session over an already-admitted carrier.
///
/// Exactly one entropy request is consumed. Both the ephemeral key and this
/// function's PSK copy are volatile-erased before return on success or refusal.
pub fn establish_conduitos_protected_line<C, S, const REQUESTS: u32>(
    mut carrier: C,
    role: Role,
    binding: &SessionBinding,
    policy: ProtectedSessionPolicy,
    mut preshared_key: [u8; 32],
    entropy: &mut CryptographicEntropyBase<S, REQUESTS>,
) -> Result<ProtectedCarrier<C>, ConduitOsProtectedLineRefusal>
where
    C: ProtectedFrameCarrier,
    S: CryptographicEntropySource,
{
    let mut ephemeral_private_key = [0_u8; 32];
    if let Err(error) = entropy.fill(&mut ephemeral_private_key) {
        erase(&mut preshared_key);
        erase(&mut ephemeral_private_key);
        return Err(ConduitOsProtectedLineRefusal::Entropy(error));
    }
    let session = establish_protected_session(
        &mut carrier,
        role,
        binding,
        policy,
        preshared_key,
        ephemeral_private_key,
    );
    erase(&mut preshared_key);
    erase(&mut ephemeral_private_key);
    let session = session.map_err(ConduitOsProtectedLineRefusal::Session)?;
    ProtectedCarrier::new(carrier, session, policy).map_err(ConduitOsProtectedLineRefusal::Session)
}

/// Run one protected session inside the exact lifetime of one authenticated
/// VirtIO TCP/TLS/WebSocket connection. No socket or reconnect authority can
/// escape the operation closure.
#[cfg(feature = "virtio-net-proof")]
#[allow(clippy::too_many_arguments)]
pub fn with_conduitos_protected_websocket<T, E, S, const REQUESTS: u32>(
    device: VirtioNetReady,
    tcp_seed: u64,
    tls_seed: [u8; 32],
    websocket_seed: [u8; 32],
    endpoint: VirtioTcpEndpoint,
    server_name: &str,
    pinned_certificate_der: &[u8],
    maximum_polls: u32,
    role: Role,
    binding: &SessionBinding,
    policy: ProtectedSessionPolicy,
    preshared_key: [u8; 32],
    entropy: &mut CryptographicEntropyBase<S, REQUESTS>,
    operation: impl FnOnce(&mut dyn ConduitOsProtectedLineIo) -> Result<T, E>,
) -> Result<(T, u32), ConduitOsProtectedWebSocketRefusal<E>>
where
    S: CryptographicEntropySource,
{
    let policy = policy.validate().map_err(|error| {
        ConduitOsProtectedWebSocketRefusal::Protected(ConduitOsProtectedLineRefusal::Session(error))
    })?;
    let receive_budget = policy
        .handshake_timeout_millis
        .min(policy.idle_timeout_millis);
    virtio_tls::with_websocket(
        device,
        tcp_seed,
        tls_seed,
        websocket_seed,
        endpoint,
        server_name,
        pinned_certificate_der,
        maximum_polls.min(receive_budget),
        |websocket| {
            let carrier = ProtectedWebSocketCarrier {
                websocket,
                maximum_receive_millis: maximum_polls.min(receive_budget),
            };
            let mut protected = establish_conduitos_protected_line(
                carrier,
                role,
                binding,
                policy,
                preshared_key,
                entropy,
            )
            .map_err(ProtectedWebSocketOperationRefusal::Protected)?;
            match operation(&mut protected) {
                Ok(value) => {
                    protected.close().map_err(|error| {
                        ProtectedWebSocketOperationRefusal::Protected(
                            ConduitOsProtectedLineRefusal::Session(error),
                        )
                    })?;
                    Ok(value)
                }
                Err(error) => {
                    // Retire the admitted protected session even when protocol
                    // work refuses. Preserve that primary operation refusal;
                    // the enclosing realization still closes WebSocket/TLS/TCP.
                    let _ = protected.close();
                    Err(ProtectedWebSocketOperationRefusal::Operation(error))
                }
            }
        },
    )
    .map_err(|error| match error {
        VirtioWebSocketRunError::Transport(error) => {
            ConduitOsProtectedWebSocketRefusal::Transport(error)
        }
        VirtioWebSocketRunError::Operation(ProtectedWebSocketOperationRefusal::Protected(
            error,
        )) => ConduitOsProtectedWebSocketRefusal::Protected(error),
        VirtioWebSocketRunError::Operation(ProtectedWebSocketOperationRefusal::Operation(
            error,
        )) => ConduitOsProtectedWebSocketRefusal::Operation(error),
    })
}

#[cfg(feature = "virtio-net-proof")]
enum ProtectedWebSocketOperationRefusal<E> {
    Protected(ConduitOsProtectedLineRefusal),
    Operation(E),
}

#[cfg(feature = "virtio-net-proof")]
struct ProtectedWebSocketCarrier<'a> {
    websocket: &'a mut dyn BinaryWebSocketIo,
    maximum_receive_millis: u32,
}

#[cfg(feature = "virtio-net-proof")]
impl ProtectedFrameCarrier for ProtectedWebSocketCarrier<'_> {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), conduit_protected_line::CarrierFailure> {
        self.websocket.send_binary(frame).map_err(map_websocket)
    }

    fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout_millis: u32,
    ) -> Result<usize, conduit_protected_line::CarrierFailure> {
        if timeout_millis == 0 || self.maximum_receive_millis > timeout_millis {
            return Err(conduit_protected_line::CarrierFailure::TimedOut);
        }
        self.websocket.receive_binary(output).map_err(map_websocket)
    }

    fn close(&mut self) -> Result<(), conduit_protected_line::CarrierFailure> {
        // The enclosing network realization owns and performs WebSocket/TLS/TCP close.
        Ok(())
    }
}

#[cfg(feature = "virtio-net-proof")]
fn map_websocket(error: WebSocketError) -> conduit_protected_line::CarrierFailure {
    match error {
        WebSocketError::RequestTooLarge | WebSocketError::ResponseTooLarge => {
            conduit_protected_line::CarrierFailure::Pressure
        }
        _ => conduit_protected_line::CarrierFailure::Lost,
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
