//! Exact Signal target advertisements and cross-host conformance topologies.
//!
//! This crate owns proof facts. It does not define portable Signal meaning and
//! none of its fixed Host, Boot, Line, board, or transport identities are
//! semantic Signal constants.

extern crate alloc;

use conduit_signal::*;

mod distributed_identity;
pub use distributed_identity::*;
mod distributed_plan;
pub use distributed_plan::*;
mod esp32_c3;
pub use esp32_c3::*;
mod esp32_s3;
pub use esp32_s3::*;
mod esp32_wroom;
pub use esp32_wroom::*;
mod std_esp32_bluetooth;
pub use std_esp32_bluetooth::*;
mod std_pico_bluetooth;
pub use std_pico_bluetooth::*;
mod std_pico_usb;
pub use std_pico_usb::*;
mod topology;
pub use topology::*;
mod toggle_topology;
pub use toggle_topology::*;
pub mod triple;
