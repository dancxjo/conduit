//! Exact owner metadata for downstream composition of declared clock relations.
//! This exports checked Types, not a clock offer or calibration authority.
use alloc::vec::Vec;
use conduit_plot::{rust_binding::NativeBindingRefusal, CheckedNativeType};

/// Returns the two Data-owned clock roots with every nested contract and law.
/// Reconstruction allocates during authoring; it does not parse Source at runtime.
pub fn checked_types() -> Result<Vec<CheckedNativeType>, NativeBindingRefusal> {
    crate::generated::checked_clock_authoring_types()
}
