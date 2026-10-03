//! Shared finite bounds for the native binary WebSocket carrier.

pub const WEBSOCKET_FRAME_BYTES: usize = 8192;
pub const MAXIMUM_BINARY_MESSAGE_BYTES: usize = WEBSOCKET_FRAME_BYTES - 14;
