//! Checked Make display graph/Fore. Neither getter claims readiness or authority.
use super::software;
pub fn planning_source(
) -> Result<conduit_plot::flat_image::indexed::IndexedSourceImage<'static>, &'static str> {
    let bytes = software::planning_source_image();
    if conduit_core::assigned_plan_payload_digest(bytes) != software::planning_source_sha256() {
        return Err("display-source-image-digest");
    }
    // SAFETY: same-recipe bytes and expected digest are linked trusted Make
    // output. No network input or claimed Source identity enters this boundary.
    unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(bytes).map(|image| {
            image
                .with_make_gear_decoder(software::decode_planning_gear)
                .with_make_configuration_decoder(software::decode_planning_configuration)
        })
    }
    .map_err(|_| "display-source-image-representation")
}
pub fn planning_boundary(
) -> Result<conduit_planner::indexed_source::IndexedAuthoringBoundary, &'static str> {
    software::planning_boundary().map_err(|_| "display-source-boundary")
}
