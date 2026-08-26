#![no_std]

extern crate alloc;

mod pico_appliance;
pub use pico_appliance::*;
mod pico_appliance_dhcp;
pub use pico_appliance_dhcp::*;
mod pico_appliance_protocol;
pub use pico_appliance_protocol::*;

/// Base-level CDC control payload asking whether the R1 network Session can
/// now accept its exact Hello. This is target realization protocol, not
/// portable network meaning.
pub const R1_USB_NETWORK_SESSION_QUERY: &[u8] = b"CONDUIT_R1_NETWORK_SESSION_QUERY@1";
pub const R1_USB_NETWORK_SESSION_READY: &[u8] = b"CONDUIT_R1_NETWORK_SESSION_READY@1";
pub const R1_USB_NETWORK_SESSION_FAILED: &[u8] = b"CONDUIT_R1_NETWORK_SESSION_FAILED@1";
pub const R1_USB_NETWORK_FAILURE_SIGN_READY: &[u8] = b"CONDUIT_R1_NETWORK_FAILURE_SIGN_READY@1";
pub const R1_USB_NETWORK_FAILURE_SIGN_WRITTEN: &[u8] = b"CONDUIT_R1_NETWORK_FAILURE_SIGN_WRITTEN@1";
pub const R1_USB_NETWORK_FAILURE_SIGN_FORMAT_FAILED: &[u8] =
    b"CONDUIT_R1_NETWORK_FAILURE_SIGN_FORMAT_FAILED@1";
pub const R1_USB_NETWORK_FAILURE_SIGN_DISCONNECTED: &[u8] =
    b"CONDUIT_R1_NETWORK_FAILURE_SIGN_DISCONNECTED@1";
