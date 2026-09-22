//! Preparation and finite-budget contract for one installed operation family.

use super::back::InstalledBack;
use conduit_core::PlannedGear;

pub(super) struct BackBudget {
    pub(super) value_items: u16,
    pub(super) value_bytes: u32,
    pub(super) host_requests: usize,
    pub(super) sign_items: u16,
    pub(super) maximum_value_bytes: u32,
}

pub(super) struct BackFactory {
    pub(super) implementation_id: &'static str,
    pub(super) budget: fn(&PlannedGear) -> Result<BackBudget, String>,
    pub(super) prepare:
        fn(&PlannedGear, &mut conduit_kernel::HostedValueStore) -> Result<InstalledBack, String>,
}
