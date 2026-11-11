//! Generic Experience integration as an ordinary finite pure Back.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES;
use conduit_core::PlannedGear;
use conduit_plot::rust_binding::NativeRustBinding;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EPISODE_EXPERIENCE_INTEGRATION_IMPLEMENTATION,
    budget,
    prepare,
};

fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    super::episode_loop_back::exact_pure_gear(
        gear,
        &conduit_std_offers::episode_experience_integration_offer(),
    )?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (2 * MAXIMUM_EPISODE_EXPERIENCE_BYTES) as u32,
        maximum_value_bytes: MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::HostedFrame(
        super::hosted_frame_back::FrameStepBack::for_transform(
            MAXIMUM_EPISODE_EXPERIENCE_BYTES,
            MAXIMUM_EPISODE_EXPERIENCE_BYTES,
            integrate,
        ),
    ))
}
fn integrate(bytes: &[u8]) -> Result<Vec<u8>, u16> {
    let input = conduit_chat::EpisodeExperienceInput::decode(bytes).map_err(|_| 1_u16)?;
    let experience = conduit_chat::integrate_episode_experience(
        &input,
        conduit_semantic_catalog::experience_evidence_limits(),
    )
    .map_err(|error| match error {
        conduit_human::ExperienceIntegrationError::Evidence(_) => 2_u16,
        conduit_human::ExperienceIntegrationError::Admission(_) => 3_u16,
    })?;
    experience.encode().map_err(|_| 4)
}
