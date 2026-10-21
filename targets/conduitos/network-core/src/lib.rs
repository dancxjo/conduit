//! Pure bounded binary WebSocket codec. No socket/TLS/device/effect authority.
#![no_std]
extern crate alloc;
pub mod framer;
mod rng;
pub mod state;
pub mod wire;
pub const HANDSHAKE_BYTES: usize = 1024;
pub const FRAME_BYTES: usize = 32 * 1024;
pub const MAXIMUM_BINARY_MESSAGE_BYTES: usize = FRAME_BYTES - 14;
/// Raw binary transport, never JSON/decimal byte expansion.
pub const MAX_BLOB: usize = FRAME_BYTES;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum WebSocketError {
    InvalidDescriptor = 1,
    RequestTooLarge,
    ResponseTooLarge,
    HandshakeWrite,
    HandshakeRead,
    HandshakeAuthentication,
    FrameWrite,
    FrameRead,
    UnexpectedFrame,
    Close,
}
impl WebSocketError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidDescriptor => "websocket-descriptor-invalid",
            Self::RequestTooLarge => "websocket-request-too-large",
            Self::ResponseTooLarge => "websocket-response-too-large",
            Self::HandshakeWrite => "websocket-handshake-write-failed",
            Self::HandshakeRead => "websocket-handshake-read-failed",
            Self::HandshakeAuthentication => "websocket-handshake-authentication-failed",
            Self::FrameWrite => "websocket-frame-write-failed",
            Self::FrameRead => "websocket-frame-read-failed",
            Self::UnexpectedFrame => "websocket-frame-unexpected",
            Self::Close => "websocket-close-failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Bounds,
    Malformed,
    Order,
    Capacity,
    WebSocket(WebSocketError),
}
pub mod memory {
    pub const EXCHANGE: usize = 0x601000;
    pub const TRANSFER: usize = 0x603000;
    pub const TRANSFER_BYTES: usize = 16 * 1024;
    pub const METADATA: usize = 0x709000;
    pub const METADATA_BYTES: usize = 1024 * 1024;
    pub const STATE: usize = METADATA + 4096;
    pub const HEADER_BYTES: usize = 20 * 1024;
    pub const HEAP_START: usize = METADATA + HEADER_BYTES;
    pub const HEAP_BYTES: usize = METADATA_BYTES - HEADER_BYTES;
}
