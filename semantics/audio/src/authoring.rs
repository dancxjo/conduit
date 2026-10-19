//! Owner-checked Audio Types for downstream authored composition.
//!
//! Import retains nominal identities, primitive field refinements and complete
//! portable Boolean laws from the same checked source as Audio's Native ABI.
//! Source is checked at build time; importing does not parse source at runtime.

use alloc::{string::String, vec::Vec};
use conduit_plot::{rust_binding::NativeBindingRefusal, CheckedNativeType, StartupCatalog};

/// Returns every checked Type owned by Audio's generated Native bindings.
///
/// Allocation and metadata reconstruction belong to authoring/preparation.
/// Invariant programs are reconstructed from their exact canonical bytes.
pub fn checked_types() -> Result<Vec<CheckedNativeType>, NativeBindingRefusal> {
    crate::generated::checked_authoring_types()
}

/// Installs the owner names and their complete checked contracts for authoring.
pub fn install(startup: &mut StartupCatalog) -> Result<(), String> {
    for ty in conduit_data::clock_authoring::checked_types()
        .map_err(|error| alloc::format!("Data clock Type metadata: {error:?}"))?
    {
        startup.insert_checked_native_type(ty.name.clone(), &ty)?;
    }
    for ty in checked_types().map_err(|error| alloc::format!("Audio Type metadata: {error:?}"))? {
        startup.insert_checked_native_type(ty.name.clone(), &ty)?;
    }
    Ok(())
}
