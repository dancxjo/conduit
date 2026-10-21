//! Explicit Draft service admission ceiling, shared with the independent guest.
//! These limits are not a measured minimum, peak usage or elapsed-time proof.
pub const IMAGE_START: usize = 0x0040_0000;
pub const IMAGE_BYTES: usize = 2 * 1024 * 1024;
pub const EXCHANGE: usize = 0x0060_1000;
pub const TRANSFER: usize = 0x0060_3000;
pub const METADATA: usize = 0x0061_9000;
pub const METADATA_BYTES: usize = 1024 * 1024;
pub const PRIVATE_DATA_BYTES: usize = 4096;
pub const STATE: usize = METADATA + PRIVATE_DATA_BYTES;
pub const HEADER_BYTES: usize = 20 * 1024;
pub const HEAP_START: usize = METADATA + HEADER_BYTES;
pub const HEAP_BYTES: usize = METADATA_BYTES - HEADER_BYTES;
