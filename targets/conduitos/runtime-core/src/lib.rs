//! One canonical fixed scheduler and finite pointer-free runtime transport.
#![no_std]
pub mod admission;
pub mod backs;
pub mod morse;
pub mod runtime;
pub mod snapshot;
pub mod text;
pub mod wire;
pub const NODES: usize = 14;
pub const CORDS: usize = 10;
pub const PORTS: usize = 16;
pub const ROUTES: usize = NODES * PORTS;
pub const VALUE_BYTES: usize = 3072;
pub const SIGN_ITEMS: usize = 1024;
pub type Scheduler = conduit_kernel::scheduler::FixedScheduler<
    backs::PlannedBack,
    conduit_kernel::FixedValueStore<10, VALUE_BYTES>,
    conduit_kernel::FixedSignLog<SIGN_ITEMS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    ROUTES,
    CORDS,
    NODES,
    NODES,
>;
pub mod memory {
    pub const EXCHANGE: usize = 0x601000;
    pub const TRANSFER: usize = 0x603000;
    pub const TRANSFER_BYTES: usize = 16 * 1024;
    pub const METADATA: usize = 0x709000;
    pub const METADATA_BYTES: usize = 1024 * 1024;
    pub const STATE: usize = METADATA + 4096;
}
