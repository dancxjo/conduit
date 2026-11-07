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
