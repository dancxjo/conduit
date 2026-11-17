//! Causal startup acknowledgment after the existing owner retention transaction.
use super::*;
use conduit_std_host::body_execution::CommittedBodyStartup;

pub(super) fn needs_startup(plan: &conduit_body::BodyPlan) -> bool {
    plan.plots
        .iter()
        .flat_map(|part| &part.plan.fragments)
        .flat_map(|fragment| &fragment.placements)
        .any(|placement| placement.kind_id.as_str() == "body/wake")
}

pub(in crate::durable_host::owner::controller) fn retain_acknowledgment(
    owner: &mut Owner,
    root: &Path,
    play: BodyPlayIdentity,
    wake: Wake,
) -> Result<Option<CommittedBodyStartup>, String> {
    owner.retain_started(root, play, wake)?;
    let retained = owner
        .session
        .realization()
        .ok_or("retained Started has no realization")?;
    if !needs_startup(&retained.plan) {
        return Ok(None);
    }
    let play = retained
        .play
        .as_ref()
        .ok_or("retained Started has no Play")?;
    CommittedBodyStartup::from_retained(owner.session.evidence(), &retained.plan, play).map(Some)
}
