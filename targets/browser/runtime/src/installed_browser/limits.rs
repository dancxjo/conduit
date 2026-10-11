//! Exact finite envelope shared by browser planning evidence and execution.

use serde::Serialize;

pub(crate) const MAXIMUM_BROWSER_PLOT_GEARS: usize = 16;
pub(crate) const MAXIMUM_BROWSER_PLOT_CORDS: usize = 24;
pub(crate) const MAXIMUM_BROWSER_GEARS: usize = 32;
pub(crate) const MAXIMUM_BROWSER_CORDS: usize = 48;
pub(crate) const MAXIMUM_BROWSER_VALUE_BYTES: usize = 4_096;
// Four canonical position/delta quantities plus bounded record metadata.
pub(crate) const NORMALIZED_POINTER_VALUE_BYTES: usize =
    4 * conduit_core::QUANTITY_ENCODED_LEN + 2048;
pub(crate) const MAXIMUM_BROWSER_STORED_VALUE_BYTES: usize = 8_192;
pub(crate) const BROWSER_PORTS_PER_GEAR: usize =
    conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub(crate) const BROWSER_QUEUE_SLOTS: usize = MAXIMUM_BROWSER_CORDS * 4;
pub(crate) const BROWSER_ROUTE_SLOTS: usize = MAXIMUM_BROWSER_GEARS * BROWSER_PORTS_PER_GEAR;
pub(crate) const BROWSER_ROUTE_TARGETS: usize = BROWSER_QUEUE_SLOTS;
pub(crate) const BROWSER_HOST_CALLS_PER_GEAR: u16 = 2;
pub(crate) const BROWSER_HOST_CALL_BINDINGS: usize =
    MAXIMUM_BROWSER_GEARS * BROWSER_HOST_CALLS_PER_GEAR as usize;
pub(crate) const BROWSER_PENDING_REQUESTS: usize = MAXIMUM_BROWSER_GEARS;
// At most 272 heterogeneous slots are derived from the selected Plan.
pub(crate) const BROWSER_VALUE_ITEMS: u16 = 272;
// The native structured contracts admit up to 256 KiB per value. The reviewed
// resident Theremin Body requires 2,150,400 bytes, including retained PCM and
// pointer values; reserve the next 128 KiB tier before Play. This value arena
// remains inside the browser profile's 8 MiB heap allowance.
pub(crate) const BROWSER_TOTAL_VALUE_BYTES: u32 = 2_176 * 1_024;
pub(crate) const BROWSER_SIGN_ITEMS: u16 = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct BrowserEnvelopeLimits {
    pub maximum_gears: usize,
    pub maximum_cords: usize,
    pub ports_per_gear: usize,
    pub queue_slots: usize,
    pub route_slots: usize,
    pub route_targets: usize,
    pub host_calls_per_gear: u16,
    pub host_call_bindings: usize,
    pub pending_requests: usize,
    pub value_items: u16,
    /// Existing generic value profile; reviewed Data profiles have explicit bounds below.
    pub maximum_value_bytes: usize,
    /// Existing generic arena tier; actual Plan reservations appear in kernel inspection.
    pub total_value_bytes: u32,
    pub runtime_memory_pool_bytes: u32,
    pub measurement_profile_value_bytes: usize,
    pub measurement_sample_value_bytes: usize,
    pub measurement_window_value_bytes: usize,
    pub measurement_summary_value_bytes: usize,
    pub measurement_hysteresis_value_bytes: usize,
    pub measurement_decision_value_bytes: usize,
    pub measurement_series_value_bytes: usize,
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
        host_calls_per_gear: BROWSER_HOST_CALLS_PER_GEAR,
        host_call_bindings: BROWSER_HOST_CALL_BINDINGS,
        pending_requests: BROWSER_PENDING_REQUESTS,
        value_items: BROWSER_VALUE_ITEMS,
        maximum_value_bytes: MAXIMUM_BROWSER_STORED_VALUE_BYTES,
        total_value_bytes: BROWSER_TOTAL_VALUE_BYTES,
        runtime_memory_pool_bytes: super::measurement_limits::MEMORY_POOL_BYTES,
        measurement_profile_value_bytes: super::measurement_limits::PROFILE,
        measurement_sample_value_bytes: super::measurement_limits::SAMPLE,
        measurement_window_value_bytes: super::measurement_limits::WINDOW,
        measurement_summary_value_bytes: super::measurement_limits::SUMMARY,
        measurement_hysteresis_value_bytes: super::measurement_limits::HYSTERESIS,
        measurement_decision_value_bytes: super::measurement_limits::DECISION,
        measurement_series_value_bytes: super::measurement_limits::SERIES,
        sign_items: BROWSER_SIGN_ITEMS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_value_arena_constructs_with_finite_slots_inside_page_profile() {
        let store = conduit_kernel::HostedValueStore::new(
            BROWSER_VALUE_ITEMS,
            MAXIMUM_BROWSER_STORED_VALUE_BYTES as u32,
            BROWSER_TOTAL_VALUE_BYTES,
        )
        .expect("the admitted aggregate must fit the preallocated slots");
        assert_eq!(
            u32::from(BROWSER_VALUE_ITEMS) * MAXIMUM_BROWSER_STORED_VALUE_BYTES as u32,
            BROWSER_TOTAL_VALUE_BYTES
        );
        let profile: serde_json::Value =
            serde_json::from_str(include_str!("../../../profiles/browser-page.profile.json"))
                .unwrap();
        assert!(
            u64::from(BROWSER_TOTAL_VALUE_BYTES)
                < profile["bounds"]["heap_arena_bytes"].as_u64().unwrap()
        );
        assert!(
            u64::from(BROWSER_VALUE_ITEMS) <= profile["bounds"]["queue_items"].as_u64().unwrap()
        );
        assert!(conduit_kernel::HostedValueStore::new(
            160,
            MAXIMUM_BROWSER_STORED_VALUE_BYTES as u32,
            BROWSER_TOTAL_VALUE_BYTES,
        )
        .is_err());
        drop(store);
    }
}
