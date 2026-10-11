//! Shared finite bounds for the native binary WebSocket carrier.

// The native Host's actual advertisement includes the checked component Mask
// offer. Its routed admission request no longer fits an 8 KiB frame. Keep one
// finite frame, including the maximum 14-byte WebSocket header, for that offer.
pub const WEBSOCKET_FRAME_BYTES: usize = 16 * 1024;
pub const MAXIMUM_BINARY_MESSAGE_BYTES: usize = WEBSOCKET_FRAME_BYTES - 14;
