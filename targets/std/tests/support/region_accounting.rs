//! Reuse the actual ledger/placement admission modules without the Std test binary.
use conduit_core::{
    HostAdvertisement, PlanFragment, PlanId, ResourceBinding, ResourceHealth, ResourceObservation,
    SignId, PROTOCOL_VERSION,
};
#[path = "../../src/kernel_preparation/pool_usage.rs"]
mod pool_usage;
use pool_usage::PoolUsage;
#[path = "../../src/kernel_preparation/ledger.rs"]
mod ledger;
pub(crate) use ledger::{KernelResourceLedger, KernelResourceReservation};
#[path = "../../src/kernel_preparation/exact_profile.rs"]
mod exact_profile;
#[path = "../../src/kernel_preparation/placement_resources.rs"]
mod placement_resources;
#[path = "../../src/kernel_preparation/region_resources.rs"]
pub(crate) mod region_resources;
