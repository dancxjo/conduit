//! Exact acknowledged-input correlation at the actual unbound-owner boundary.
use super::known::Owner;
use conduit_environment_http::network_setup::Submission;

/// Caller first returns the actual unselected provider and source. This only
/// retires an unused frozen snapshot; it neither consumes ACK nor grants Play.
pub(super) fn retire_superseded(
    owner: &mut Owner,
    frozen_origin: &mut Option<Submission>,
    acknowledged: Option<Submission>,
) -> Result<(), &'static str> {
    if acknowledged.is_some_and(|token| Some(token) != *frozen_origin) {
        if let Some(selection) = owner.selection() {
            owner
                .retire_unconsumed(selection)
                .map_err(|_| "credential-selection-retirement")?;
            *frozen_origin = None;
        }
    }
    Ok(())
}
