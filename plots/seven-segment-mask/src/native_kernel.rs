//! Bounded realization of the ordinary read-only Face/renderer/interaction Plot.
//! This prepares shared-kernel tables, not a provider, Body membership or effect
//! authority. The selected renderer Host Call still needs an admitted service.
use conduit_kernel::{self as k, scheduler::*};
pub type Kernel<V = k::HostedValueStore> =
    FixedScheduler<MaskBack, V, k::FixedSignLog<64>, 3, 6, 2, 6, 6, 6, 3, 3>;
#[path = "native_assigned.rs"]
mod assigned;
pub use assigned::*;
mod backs;
pub use backs::MaskBack;

/// Retain the same finite lifecycle quota on first preparation and rearm.
pub fn prepare_signs() -> Result<k::FixedSignLog<64>, k::SignError> {
    k::FixedSignLog::new_with_remote_storage(
        (64 * core::mem::size_of::<k::KernelEvent>()) as u32,
        12,
        k::remote_sign_storage_bytes(12).ok_or(k::SignError::InvalidBudget)?,
    )
}
