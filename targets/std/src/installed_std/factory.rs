//! Preparation and finite-budget contract for one installed operation family.

use super::operation::InstalledOperation;
use conduit_core::PlannedGear;

pub(super) struct OperationBudget {
    pub(super) value_items: u16,
    pub(super) value_bytes: u32,
    pub(super) host_requests: usize,
    pub(super) sign_items: u16,
    pub(super) maximum_value_bytes: u32,
}

pub(super) struct InstalledFactory {
    pub(super) implementation_id: &'static str,
    pub(super) budget: fn(&PlannedGear) -> Result<OperationBudget, String>,
    pub(super) prepare: fn(
        &PlannedGear,
        &mut conduit_kernel::HostedValueStore,
    ) -> Result<InstalledOperation, String>,
}
