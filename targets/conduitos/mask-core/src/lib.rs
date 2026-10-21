//! Workspace Mask reading/layout policy, with no semantic/effect/scanout authority.
#![no_std]
extern crate alloc;
pub mod data;
pub mod layout;
pub mod reader;
pub mod scene;
pub mod state;
pub mod wire;
pub const MAX_BLOB: usize = 64 * 1024;
pub const MAX_VIEW_BYTES: usize = 3072;
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
