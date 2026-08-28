#![no_std]

extern crate alloc;

mod r1_route;
pub use r1_route::*;
mod r1_wifi_bootstrap;
pub use r1_wifi_bootstrap::*;

pub const R1_WIFI_STATION_POOL_ID: &str = "r1/pico-wifi-station-0";

/// Exact CDC coordination bytes for the R1 recovery evidence path.
///
/// These are proof topology facts, not generic RP2040 network realization
/// defaults.
pub const R1_USB_NETWORK_SESSION_QUERY: &[u8] = b"CONDUIT_R1_NETWORK_SESSION_QUERY@1";
pub const R1_USB_NETWORK_SESSION_READY: &[u8] = b"CONDUIT_R1_NETWORK_SESSION_READY@1";
pub const R1_USB_NETWORK_SESSION_FAILED: &[u8] = b"CONDUIT_R1_NETWORK_SESSION_FAILED@1";
pub const R1_USB_NETWORK_FAILURE_SIGN_READY: &[u8] = b"CONDUIT_R1_NETWORK_FAILURE_SIGN_READY@1";
pub const R1_USB_NETWORK_FAILURE_SIGN_WRITTEN: &[u8] = b"CONDUIT_R1_NETWORK_FAILURE_SIGN_WRITTEN@1";
pub const R1_USB_NETWORK_FAILURE_SIGN_FORMAT_FAILED: &[u8] =
    b"CONDUIT_R1_NETWORK_FAILURE_SIGN_FORMAT_FAILED@1";
pub const R1_USB_NETWORK_FAILURE_SIGN_DISCONNECTED: &[u8] =
    b"CONDUIT_R1_NETWORK_FAILURE_SIGN_DISCONNECTED@1";
