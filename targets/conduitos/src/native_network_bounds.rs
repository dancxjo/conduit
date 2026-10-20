//! Shared finite bounds for the native binary WebSocket carrier.

// In the Source #5624 diagnostic, the required-slot component contract grows
// admission request from 13,874 to 16,670 bytes (advertisement: 16,035 bytes).
// Budget the complete envelope, not only its advertisement. This finite frame
// leaves tested headroom for representative Host/Boot identity growth. Oversized requests still refuse. Membership, Plan and Play
// authority remain unchanged.
// Compared with 16 KiB, each connection reserves 32 KiB more across its fixed
// receive/transmit arrays; each payload-sized auxiliary buffer adds 16 KiB.
// The independent serial observation cap remains 32 KiB.
pub const WEBSOCKET_FRAME_BYTES: usize = 32 * 1024;
pub const MAXIMUM_BINARY_MESSAGE_BYTES: usize = WEBSOCKET_FRAME_BYTES - 14;
