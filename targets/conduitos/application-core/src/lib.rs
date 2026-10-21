//! Canonical retained native applications; Root owns every external effect grant.
#![no_std]
extern crate alloc;
pub mod data;
pub mod models;
pub mod state;
pub mod tutorial;
pub mod wire;
pub const SLOTS: usize = 4;
pub const VIEW_BYTES: usize = 3072;
pub const EVENT_BYTES: usize = 128;
pub const MAX_BLOB: usize = 128 * 1024;
pub mod memory {
    pub const EXCHANGE: usize = 0x601000;
    pub const TRANSFER: usize = 0x603000;
    pub const TRANSFER_BYTES: usize = 16 * 1024;
    pub const METADATA: usize = 0x709000;
    pub const METADATA_BYTES: usize = 2 * 1024 * 1024;
    pub const STATE: usize = METADATA + 4096;
    pub const HEADER_BYTES: usize = 24 * 1024;
    pub const HEAP_START: usize = METADATA + HEADER_BYTES;
    pub const HEAP_BYTES: usize = METADATA_BYTES - HEADER_BYTES;
}
pub mod graph_facts;
