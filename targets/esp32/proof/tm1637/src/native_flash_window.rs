//! ESP uses the shared affine local flash-window arbitration.
//! Device ownership and physical timing remain the native adapter's obligations.
pub(crate) use conduit_monotonic_wait::flash_window::*;
