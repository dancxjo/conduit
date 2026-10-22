//! One binary frame boundary: x86_64 production framing lives in its guest.
pub use conduitos_network_core::WebSocketError;
pub(crate) const MAXIMUM_BINARY_MESSAGE_BYTES: usize =
    crate::native_network_bounds::MAXIMUM_BINARY_MESSAGE_BYTES;

/// Socket, authenticated TLS, endpoint approval, deadlines and reconnect remain
/// with the enclosing Root network realization.
pub(crate) trait BinaryWebSocketIo {
    fn send_binary(&mut self, payload: &[u8]) -> Result<(), WebSocketError>;
    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, WebSocketError>;
}
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub(crate) use crate::protected_network::BoundedWebSocket;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
mod local;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub(crate) use local::BoundedWebSocket;
