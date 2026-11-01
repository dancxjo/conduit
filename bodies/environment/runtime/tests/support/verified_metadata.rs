//! Test-only retirement of a rich ordinary Plan's exact configurations.
use conduit_core as c;
pub fn from_plan(plan: &c::Plan) -> c::VerifiedPlanMetadata {
    let mut metadata = plan.clone();
    for gear in metadata
        .fragments
        .iter_mut()
        .flat_map(|f| &mut f.placements)
    {
        gear.configuration.clear();
    }
    c::verify_plan_metadata_with_configurations(metadata, |gear| {
        Ok::<_, ()>(
            plan.fragments
                .iter()
                .flat_map(|f| &f.placements)
                .find(|original| original.placement_id == gear.placement_id)
                .expect("same exact local Plan")
                .configuration
                .clone(),
        )
    })
    .unwrap()
}
