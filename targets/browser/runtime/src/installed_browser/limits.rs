//! Exact finite envelope shared by browser planning evidence and execution.

use serde::Serialize;

pub(crate) const MAXIMUM_BROWSER_GEARS: usize = 16;
pub(crate) const MAXIMUM_BROWSER_CORDS: usize = 24;
pub(crate) const MAXIMUM_BROWSER_VALUE_BYTES: usize = 4_096;
pub(crate) const BROWSER_PORTS_PER_GEAR: usize =
    conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub(crate) const BROWSER_QUEUE_SLOTS: usize = 96;
pub(crate) const BROWSER_ROUTE_SLOTS: usize = MAXIMUM_BROWSER_GEARS * BROWSER_PORTS_PER_GEAR;
pub(crate) const BROWSER_ROUTE_TARGETS: usize = 96;
pub(crate) const BROWSER_HOST_OPERATIONS_PER_GEAR: u16 = 2;
pub(crate) const BROWSER_HOST_OPERATION_BINDINGS: usize =
    MAXIMUM_BROWSER_GEARS * BROWSER_HOST_OPERATIONS_PER_GEAR as usize;
pub(crate) const BROWSER_PENDING_REQUESTS: usize = MAXIMUM_BROWSER_GEARS;
pub(crate) const BROWSER_VALUE_ITEMS: u16 = 128;
pub(crate) const BROWSER_TOTAL_VALUE_BYTES: u32 = 512 * 1_024;
pub(crate) const BROWSER_SIGN_ITEMS: u16 = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct BrowserEnvelopeLimits {
    pub maximum_gears: usize,
    pub maximum_cords: usize,
    pub ports_per_gear: usize,
    pub queue_slots: usize,
    pub route_slots: usize,
    pub route_targets: usize,
    pub host_operations_per_gear: u16,
    pub host_operation_bindings: usize,
    pub pending_requests: usize,
    pub value_items: u16,
    pub maximum_value_bytes: usize,
    pub total_value_bytes: u32,
    pub sign_items: u16,
}

pub(crate) const fn envelope_limits() -> BrowserEnvelopeLimits {
    BrowserEnvelopeLimits {
        maximum_gears: MAXIMUM_BROWSER_GEARS,
        maximum_cords: MAXIMUM_BROWSER_CORDS,
        ports_per_gear: BROWSER_PORTS_PER_GEAR,
        queue_slots: BROWSER_QUEUE_SLOTS,
        route_slots: BROWSER_ROUTE_SLOTS,
        route_targets: BROWSER_ROUTE_TARGETS,
        host_operations_per_gear: BROWSER_HOST_OPERATIONS_PER_GEAR,
        host_operation_bindings: BROWSER_HOST_OPERATION_BINDINGS,
        pending_requests: BROWSER_PENDING_REQUESTS,
        value_items: BROWSER_VALUE_ITEMS,
        maximum_value_bytes: MAXIMUM_BROWSER_VALUE_BYTES,
        total_value_bytes: BROWSER_TOTAL_VALUE_BYTES,
        sign_items: BROWSER_SIGN_ITEMS,
    }
}
