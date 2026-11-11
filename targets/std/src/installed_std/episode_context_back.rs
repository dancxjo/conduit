//! Pure typed context rendering after the explicit episode retrieval Gear.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_chat::{MAXIMUM_EPISODE_PROMPT_BYTES, MAXIMUM_EPISODE_RECOLLECTION_BYTES};
use conduit_core::PlannedGear;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EPISODE_MODEL_CONTEXT_IMPLEMENTATION,
    budget,
    prepare,
};

fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    super::episode_loop_back::exact_pure_gear(
        gear,
        &conduit_std_offers::episode_model_context_offer(),
    )?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (MAXIMUM_EPISODE_RECOLLECTION_BYTES + MAXIMUM_EPISODE_PROMPT_BYTES) as u32,
        maximum_value_bytes: MAXIMUM_EPISODE_RECOLLECTION_BYTES as u32,
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
            MAXIMUM_EPISODE_RECOLLECTION_BYTES,
            MAXIMUM_EPISODE_PROMPT_BYTES,
            |bytes| {
                conduit_chat::episode_recollection_request(bytes)
                    .map_err(super::episode_loop_back::refusal_detail)
            },
        ),
    ))
}
