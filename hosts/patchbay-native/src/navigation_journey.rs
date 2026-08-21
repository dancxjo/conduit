//! Native consumption of a shared portable navigation journey.

use conduit_presentation::{NavigationJourneyDisposition, NavigationJourneyReceipt, Presentation};
use patchbay_model::PatchbayNavigationProjection;

/// Manifest successful receipt cursors without replaying them or acquiring
/// mutation authority. Refusals remain visible in the receipt itself.
pub(super) fn portable_navigation_journey_lines(
    presentation: &Presentation,
    navigation: &PatchbayNavigationProjection,
    journey: &NavigationJourneyReceipt,
) -> Result<Vec<Vec<String>>, String> {
    journey
        .validate(presentation, &navigation.navigation)
        .map_err(|error| format!("portable navigation journey is invalid: {error:?}"))?;
    journey
        .steps
        .iter()
        .filter(|step| step.disposition == NavigationJourneyDisposition::Advanced)
        .map(|step| {
            let mut at_step = navigation.clone();
            at_step.cursor = step.after_cursor.clone();
            crate::presentation::ordinary_front_door_lines(presentation, &at_step, None)
        })
        .collect()
}
