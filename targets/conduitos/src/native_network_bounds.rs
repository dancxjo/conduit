//! Shared finite bounds for the native binary WebSocket carrier.

// #6288: the actual @3 layout product advertisement is 32,570 bytes before
// signed join envelopes (64-byte Host/Boot identities, observed provider).
// Admit complete envelopes, retaining finite refusal beyond this frame. Each
// fixed receive/transmit array grows by 16 KiB versus the former 32 KiB frame;
// every payload-sized auxiliary buffer grows by the same amount. Scene bounds
// and the Application private grant are unchanged; native proof owns admission.
pub const WEBSOCKET_FRAME_BYTES: usize = 48 * 1024;
pub const MAXIMUM_BINARY_MESSAGE_BYTES: usize = WEBSOCKET_FRAME_BYTES - 14;

// The production x86_64 codec lives in its protected service. Host diagnostics
// and Root admission must use exactly the same envelope capacity.
#[cfg(feature = "native-owner-network")]
const _: () = assert!(WEBSOCKET_FRAME_BYTES == conduitos_network_core::FRAME_BYTES);
