//! Preparation and finite-budget contract for one installed operation family.

use super::back::InstalledBack;
use conduit_core::PlannedGear;

pub(crate) struct BackBudget {
    pub(crate) value_items: u16,
    pub(crate) value_bytes: u32,
    pub(crate) host_requests: usize,
    pub(crate) sign_items: u16,
    pub(crate) maximum_value_bytes: u32,
}

pub(super) struct BackFactory {
    pub(super) implementation_id: &'static str,
    pub(super) budget: fn(&PlannedGear) -> Result<BackBudget, String>,
    pub(super) prepare:
        fn(&PlannedGear, &mut conduit_kernel::HostedValueStore) -> Result<InstalledBack, String>,
}
