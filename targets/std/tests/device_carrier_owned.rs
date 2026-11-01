#![cfg(target_os = "linux")]
// Compile the exact production owner and its private CPU stream tests without
// compiling unrelated std library unit-test fixtures. This is no physical USB
// proof: native identity/correlation/lifetime paths run over owned local sockets.
pub use conduit_std_host::{usb_cdc,StdHost};
#[path="../src/device_carrier.rs"]
mod device_carrier;
