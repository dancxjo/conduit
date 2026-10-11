//! Finite tables shared by standalone and Body runs of the normal std kernel.
//! Preparation refuses Plans exceeding any table before Play starts.
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub(super) const MAX_NODES: usize = 32;
// A chain with 32 nodes has 33 Cords including its two external Fores.
pub(super) const MAX_CORDS: usize = 64;
pub(super) const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub(super) const MAX_QUEUE_SLOTS: usize = 64;
pub(super) const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
pub(super) const ROUTE_TARGETS: usize = 64;
pub(super) const HOST_CALLS_PER_NODE: u16 = 4;
pub(super) const HOST_BINDING_SLOTS: usize = MAX_NODES * HOST_CALLS_PER_NODE as usize;
pub(super) const PENDING_REQUESTS: usize = MAX_NODES;
