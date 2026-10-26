//! Publication provenance from the installed Body proposal, not an owner command.
//! This creates no resource possession, startup evidence or permission to execute.
use conduit_body::{BodyPlanEvidence, SealedBodyLifecycleSession};
use conduit_core::{ActivePlayIdentity, FragmentId, VerifiedPlanMetadata};
pub fn check<'a>(
    owner: &'a SealedBodyLifecycleSession,
    plan: &VerifiedPlanMetadata,
    fragment_id: &FragmentId,
    active: &ActivePlayIdentity,
) -> Result<(&'a conduit_body::BodyId, &'a conduit_body::WakeId), &'static str> {
    let current = owner.realization().ok_or("local-face-no-proposal")?;
    current
        .plan
        .validate_for(&current.wake)
        .map_err(|_| "local-face-body-plan")?;
    let fragment = plan.fragment(fragment_id).ok_or("local-face-fragment")?;
    let metadata = fragment.lowering_metadata();
    let expected = conduit_core::bind_active_play(
        &active.plan_id,
        &active.host_id,
        &active.boot_id,
        active.play_sequence,
    );
    if active.play_sequence == 0
        || &expected != active
        || plan.plan_id() != &active.plan_id
        || metadata.host_id != active.host_id
        || metadata.boot_id != active.boot_id
        || !owner
            .evidence()
            .membership
            .parts
            .iter()
            .filter(|part| part.state == conduit_body::MembershipState::Admitted)
            .filter_map(|part| part.current.as_ref())
            .any(|host| host.host_id == active.host_id && host.boot_id == active.boot_id)
    {
        return Err("local-face-current-member");
    }
    let source = plan.preparation_metadata();
    let matches = current
        .plan
        .plots()
        .iter()
        .filter(|plot| {
            plot.plan_id() == plan.plan_id()
                && plot.identity().source_document_id == source.source_document_id
                && plot.identity().checked_plot_id == source.checked_plot_id
                && plot.identity().expanded_plot_id == source.expanded_plot_id
                && plot.fragments().iter().any(|fragment| {
                    fragment.fragment_id() == fragment_id
                        && fragment.host_id() == &active.host_id
                        && fragment.boot_id() == &active.boot_id
                        && fragment.offer_generation() == metadata.offer_generation
                })
        })
        .count();
    if matches != 1 {
        return Err("local-face-unproposed-plan");
    }
    Ok((current.plan.body_id(), current.plan.wake_id()))
}
