//! Admission for retained routed abnormal-origin evidence.
use super::{AssignedConnectionTrack, CordSpec, SchedulerError};

/// A full node-indexed stock preserves the default profile. A zero stock is
/// valid only when no active Cord can route an abnormal terminal. Intermediate
/// capacities are not sparse node maps and therefore cannot be admitted.
pub(super) fn validate_capacity<const STOCK: usize, const NODES: usize>(
    active_cords: &[CordSpec],
) -> Result<(), SchedulerError> {
    if STOCK == NODES {
        return Ok(());
    }
    if STOCK != 0
        || active_cords
            .iter()
            .any(|cord| cord.track == AssignedConnectionTrack::AbnormalTerminal)
    {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
}
