#![no_std]
extern crate alloc;
mod device;
pub mod state;
pub mod wire;
pub const FRAME_BYTES: usize = 1514;
pub const SOCKET_BYTES: usize = 4096;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Refusal {
    Bounds = 1,
    Malformed = 2,
    Order = 3,
    Capacity = 4,
    Connect = 5,
    Send = 6,
    Receive = 7,
    Closed = 8,
    Clock = 9,
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
pub(crate) fn buffer(length: usize) -> Result<alloc::vec::Vec<u8>, Refusal> {
    let mut bytes = alloc::vec::Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| Refusal::Capacity)?;
    bytes.resize(length, 0);
    Ok(bytes)
}
