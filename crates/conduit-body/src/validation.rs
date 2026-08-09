use conduit_core::ClueId;

use crate::identity::validate_ids;
use crate::{BodyLifecycleError, WakeLifecycle, WakePlan, WakePlanState};

pub(crate) fn validate_clue(values: &[ClueId], capacity: usize) -> Result<(), BodyLifecycleError> {
    if values.is_empty() || values.len() > capacity {
        return Err(BodyLifecycleError::ClueCapacityExhausted);
    }
    for (index, value) in values.iter().enumerate() {
        validate_ids(&[value.as_str()])?;
        if values[..index].contains(value) {
            return Err(BodyLifecycleError::DuplicateClue);
        }
    }
    Ok(())
}

pub(crate) fn validate_new_clue(
    values: &[ClueId],
    value: &ClueId,
    capacity: usize,
) -> Result<(), BodyLifecycleError> {
    validate_ids(&[value.as_str()])?;
    if values.contains(value) {
        return Err(BodyLifecycleError::DuplicateClue);
    }
    if values.len() >= capacity {
        return Err(BodyLifecycleError::ClueCapacityExhausted);
    }
    Ok(())
}

pub(crate) fn validate_plan_history(
    lifecycle: WakeLifecycle,
    plans: &[WakePlan],
) -> Result<(), BodyLifecycleError> {
    for plan in plans {
        validate_ids(&[plan.plan_id.as_str()])?;
        if let Some(play) = &plan.active_play_id {
            validate_ids(&[play.as_str()])?;
        }
        match (&plan.state, &plan.hold) {
            (WakePlanState::Held | WakePlanState::Invalidated, Some(hold)) => {
                hold.validate_for_plan(&plan.plan_id)?;
            }
            (WakePlanState::AwaitingPlay, None)
            | (WakePlanState::Playing, None | Some(_))
            | (WakePlanState::Unsatisfied, None | Some(_))
            | (WakePlanState::Superseded, None | Some(_)) => {}
            _ => return Err(BodyLifecycleError::InvalidTransition),
        }
    }
    if plans
        .iter()
        .enumerate()
        .any(|(i, p)| plans[..i].iter().any(|q| q.plan_id == p.plan_id))
        || plans
            .iter()
            .enumerate()
            .any(|(i, p)| i + 1 < plans.len() && p.state != WakePlanState::Superseded)
    {
        return Err(BodyLifecycleError::InvalidTransition);
    }
    let current = plans.last();
    let valid = match lifecycle {
        WakeLifecycle::AwaitingPlan => plans.is_empty(),
        WakeLifecycle::AwaitingPlay => current
            .is_some_and(|p| p.state == WakePlanState::AwaitingPlay && p.active_play_id.is_none()),
        WakeLifecycle::Held => current.is_some_and(|p| {
            p.state == WakePlanState::Held && p.active_play_id.is_none() && p.hold.is_some()
        }),
        WakeLifecycle::AwaitingReplacement => current.is_some_and(|p| {
            p.state == WakePlanState::Invalidated && p.active_play_id.is_none() && p.hold.is_some()
        }),
        WakeLifecycle::Playing => {
            current.is_some_and(|p| p.state == WakePlanState::Playing && p.active_play_id.is_some())
        }
        WakeLifecycle::Unsatisfied => current
            .is_some_and(|p| p.state == WakePlanState::Unsatisfied && p.active_play_id.is_some()),
        WakeLifecycle::Lulled | WakeLifecycle::Failed => true,
    };
    valid
        .then_some(())
        .ok_or(BodyLifecycleError::InvalidTransition)
}
