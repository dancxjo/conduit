use super::{BodyCausalEvidenceRefusal, BodyPlan, BodyRunReport};
use conduit_body::{Wake, WakeLifecycle, WakeLifecycleEvent};

pub(super) fn validate_recovery_continuity(
    continuity: &Wake,
    prior_plan: &BodyPlan,
    prior_report: &BodyRunReport,
    replacement_plan: &BodyPlan,
    replacement_report: &BodyRunReport,
) -> Result<(), BodyCausalEvidenceRefusal> {
    if continuity.validate().is_err()
        || continuity.body_id != prior_plan.body_id
        || continuity.wake_id != prior_plan.wake_id
        || matches!(continuity.lifecycle, WakeLifecycle::Failed)
    {
        return Err(BodyCausalEvidenceRefusal::MismatchedContinuity);
    }
    let prior_started = continuity.events.iter().position(|event| {
        matches!(
            event,
            WakeLifecycleEvent::PlayStarted {
                plan_id,
                active_play_id,
                ..
            } if plan_id == &prior_plan.plan_id
                && active_play_id == &prior_report.play.active_play_id
        )
    });
    let unsatisfied = continuity.events.iter().position(|event| {
        matches!(
            event,
            WakeLifecycleEvent::BecameUnsatisfied { plan_id, .. }
                if plan_id == &prior_plan.plan_id
        )
    });
    let replanned = continuity.events.iter().position(|event| {
        matches!(
            event,
            WakeLifecycleEvent::Replanned {
                prior_plan_id,
                replacement_plan_id,
                ..
            } if prior_plan_id == &prior_plan.plan_id
                && replacement_plan_id == &replacement_plan.plan_id
        )
    });
    let replacement_started = continuity.events.iter().position(|event| {
        matches!(
            event,
            WakeLifecycleEvent::PlayStarted {
                plan_id,
                active_play_id,
                ..
            } if plan_id == &replacement_plan.plan_id
                && active_play_id == &replacement_report.play.active_play_id
        )
    });
    match (prior_started, unsatisfied, replanned, replacement_started) {
        (Some(start), Some(lost), Some(replan), Some(restart))
            if start < lost && lost < replan && replan < restart =>
        {
            Ok(())
        }
        _ => Err(BodyCausalEvidenceRefusal::MismatchedContinuity),
    }
}
