//! Reuse only the immutable Mask Plan already issued by this running owner.
//!
//! A Face change needs a new route seal and Show, not another expansion of the
//! same compiled source catalog. This is not an incoming-Plan admission path.
use conduit_core::{HostAdvertisement, LineOffer};
use conduit_face::{PlannedMaskPlot, RemoteOwnerMaskRouteSeal};

pub(super) fn current_browser_plan<'a>(
    retained: Option<&'a RemoteOwnerMaskRouteSeal>,
    browser: &HostAdvertisement,
    root: &str,
    face: &LineOffer,
    returned: &LineOffer,
    interaction: &LineOffer,
) -> Option<&'a PlannedMaskPlot> {
    let retained = retained?;
    if retained.planned_mask.mask.plot_name != root
        || retained.face_line != face.admitted_line()
        || retained.return_line != returned.admitted_line()
        || retained.interaction_line.as_ref() != Some(&interaction.admitted_line())
        || retained.validate_mask_host_offer(browser).is_err()
    {
        return None;
    }
    Some(&retained.planned_mask)
}

/// Called only after current owner membership, observed Back and carrier
/// authorization checks. A miss uses ordinary source checking and planning.
pub(super) fn prepare_browser_plan(
    retained: Option<&RemoteOwnerMaskRouteSeal>,
    browser: &HostAdvertisement,
    owner: &HostAdvertisement,
    source: &str,
    root: &str,
    evidence: &super::BrowserCarrierLineEvidence,
) -> Result<PlannedMaskPlot, String> {
    // Only the immutable catalog compiled into this owner is cacheable. A
    // caller supplying different source bytes must take ordinary admission.
    let catalog_source = if root == "browser-graphical" {
        Some(conduit_browser_mask_offer::MASK_SOURCE)
    } else if conduit_face::ComponentVisualSelection::from_todo_root(root).is_some() {
        Some(conduit_face::TODO_COMPONENT_MASK_SOURCE)
    } else {
        None
    };
    if catalog_source == Some(source) {
        if let Some(planned) = current_browser_plan(
            retained,
            browser,
            root,
            &evidence.face,
            &evidence.returned,
            &evidence.interaction,
        ) {
            return Ok(planned.clone());
        }
    }
    conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
        browser,
        owner,
        &evidence.face,
        &evidence.returned,
        &evidence.interaction,
        source,
        root,
    )
}
