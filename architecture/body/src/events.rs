use conduit_core::{ActivePlayId, PlanId, SignId};
use serde::{Deserialize, Serialize};

use crate::{
    hold::validate_planning_basis_signs, BodyLifecycleError, BodyState, HoldPolicy, WakeId,
    WakeLifecycle, WakePlan,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyLifecycleEvent {
    Born { sign_id: SignId },
    Woke { wake_id: WakeId, sign_id: SignId },
    LullRetained { wake_id: WakeId, sign_id: SignId },
}

impl BodyLifecycleEvent {
    pub fn sign_id(&self) -> &SignId {
        match self {
            Self::Born { sign_id }
            | Self::Woke { sign_id, .. }
            | Self::LullRetained { sign_id, .. } => sign_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakeLifecycleEvent {
    Woke {
        sign_id: SignId,
    },
    PlanReady {
        plan_id: PlanId,
        sign_id: SignId,
    },
    PlanHeld {
        prior_plan_id: Option<PlanId>,
        plan_id: PlanId,
        basis_sign_ids: alloc::vec::Vec<SignId>,
        policy: HoldPolicy,
        sign_id: SignId,
    },
    HeldPlanReleased {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        sign_id: SignId,
    },
    HeldPlanInvalidated {
        plan_id: PlanId,
        current_basis_sign_ids: alloc::vec::Vec<SignId>,
        sign_id: SignId,
    },
    PlayStarted {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        sign_id: SignId,
    },
    BecameUnsatisfied {
        plan_id: PlanId,
        sign_id: SignId,
    },
    Replanned {
        prior_plan_id: PlanId,
        replacement_plan_id: PlanId,
        sign_id: SignId,
    },
    SamePlanObserved {
        plan_id: PlanId,
        sign_id: SignId,
    },
    Lulled {
        sign_id: SignId,
    },
    Failed {
        sign_id: SignId,
    },
}

impl WakeLifecycleEvent {
    pub fn sign_id(&self) -> &SignId {
        match self {
            Self::Woke { sign_id }
            | Self::PlanReady { sign_id, .. }
            | Self::PlanHeld { sign_id, .. }
            | Self::HeldPlanReleased { sign_id, .. }
            | Self::HeldPlanInvalidated { sign_id, .. }
            | Self::PlayStarted { sign_id, .. }
            | Self::BecameUnsatisfied { sign_id, .. }
            | Self::Replanned { sign_id, .. }
            | Self::SamePlanObserved { sign_id, .. }
            | Self::Lulled { sign_id }
            | Self::Failed { sign_id } => sign_id,
        }
    }
}

pub(crate) fn validate_body_events(
    events: &[BodyLifecycleEvent],
    sign: &[SignId],
    state: &BodyState,
) -> Result<(), BodyLifecycleError> {
    if events.len() != sign.len()
        || events
            .iter()
            .zip(sign)
            .any(|(event, id)| event.sign_id() != id)
        || !matches!(events.first(), Some(BodyLifecycleEvent::Born { .. }))
    {
        return Err(BodyLifecycleError::InvalidTransition);
    }
    let mut replayed = BodyState::Lulled;
    for event in events.iter().skip(1) {
        replayed = match (&replayed, event) {
            (BodyState::Lulled, BodyLifecycleEvent::Woke { wake_id, .. }) => BodyState::Awake {
                wake_id: wake_id.clone(),
            },
            (
                BodyState::Awake { wake_id },
                BodyLifecycleEvent::LullRetained {
                    wake_id: retained, ..
                },
            ) if wake_id == retained => BodyState::Lulled,
            _ => return Err(BodyLifecycleError::InvalidTransition),
        };
    }
    if &replayed == state {
        Ok(())
    } else {
        Err(BodyLifecycleError::InvalidTransition)
    }
}

pub(crate) fn validate_wake_events(
    events: &[WakeLifecycleEvent],
    sign: &[SignId],
    lifecycle: WakeLifecycle,
    plans: &[WakePlan],
) -> Result<(), BodyLifecycleError> {
    if events.len() != sign.len()
        || events
            .iter()
            .zip(sign)
            .any(|(event, id)| event.sign_id() != id)
        || !matches!(events.first(), Some(WakeLifecycleEvent::Woke { .. }))
    {
        return Err(BodyLifecycleError::InvalidTransition);
    }
    let mut replayed = WakeLifecycle::AwaitingPlan;
    let mut plan_index = 0usize;
    for event in events.iter().skip(1) {
        replayed = match (replayed, event) {
            (WakeLifecycle::AwaitingPlan, WakeLifecycleEvent::PlanReady { plan_id, .. })
                if plans.first().is_some_and(|plan| &plan.plan_id == plan_id) =>
            {
                plan_index = 1;
                WakeLifecycle::AwaitingPlay
            }
            (
                WakeLifecycle::AwaitingPlan,
                WakeLifecycleEvent::PlanHeld {
                    prior_plan_id: None,
                    plan_id,
                    basis_sign_ids,
                    policy,
                    ..
                },
            ) if plans.first().is_some_and(|plan| {
                &plan.plan_id == plan_id
                    && plan.hold.as_ref().is_some_and(|hold| {
                        hold.basis.sign_ids() == basis_sign_ids && &hold.policy == policy
                    })
            }) =>
            {
                plan_index = 1;
                WakeLifecycle::Held
            }
            (
                WakeLifecycle::AwaitingPlay,
                WakeLifecycleEvent::PlayStarted {
                    plan_id,
                    active_play_id,
                    ..
                },
            ) if plans.get(plan_index - 1).is_some_and(|plan| {
                &plan.plan_id == plan_id && plan.active_play_id.as_ref() == Some(active_play_id)
            }) =>
            {
                WakeLifecycle::Playing
            }
            (
                WakeLifecycle::Held,
                WakeLifecycleEvent::HeldPlanReleased {
                    plan_id,
                    active_play_id,
                    ..
                },
            ) if plans.get(plan_index - 1).is_some_and(|plan| {
                &plan.plan_id == plan_id && plan.active_play_id.as_ref() == Some(active_play_id)
            }) =>
            {
                WakeLifecycle::Playing
            }
            (
                WakeLifecycle::Held,
                WakeLifecycleEvent::HeldPlanInvalidated {
                    plan_id,
                    current_basis_sign_ids,
                    ..
                },
            ) if validate_planning_basis_signs(current_basis_sign_ids).is_ok()
                && plans.get(plan_index - 1).is_some_and(|plan| {
                    &plan.plan_id == plan_id
                        && plan
                            .hold
                            .as_ref()
                            .is_some_and(|hold| hold.basis.sign_ids() != current_basis_sign_ids)
                }) =>
            {
                WakeLifecycle::AwaitingReplacement
            }
            (WakeLifecycle::Playing, WakeLifecycleEvent::SamePlanObserved { plan_id, .. })
                if plans
                    .get(plan_index - 1)
                    .is_some_and(|plan| &plan.plan_id == plan_id) =>
            {
                WakeLifecycle::Playing
            }
            (WakeLifecycle::Playing, WakeLifecycleEvent::BecameUnsatisfied { plan_id, .. })
                if plans
                    .get(plan_index - 1)
                    .is_some_and(|plan| &plan.plan_id == plan_id) =>
            {
                WakeLifecycle::Unsatisfied
            }
            (
                WakeLifecycle::Unsatisfied,
                WakeLifecycleEvent::Replanned {
                    prior_plan_id,
                    replacement_plan_id,
                    ..
                },
            ) if plans
                .get(plan_index - 1)
                .is_some_and(|plan| &plan.plan_id == prior_plan_id)
                && plans
                    .get(plan_index)
                    .is_some_and(|plan| &plan.plan_id == replacement_plan_id) =>
            {
                plan_index += 1;
                WakeLifecycle::AwaitingPlay
            }
            (
                WakeLifecycle::AwaitingReplacement,
                WakeLifecycleEvent::Replanned {
                    prior_plan_id,
                    replacement_plan_id,
                    ..
                },
            ) if plans
                .get(plan_index - 1)
                .is_some_and(|plan| &plan.plan_id == prior_plan_id)
                && plans
                    .get(plan_index)
                    .is_some_and(|plan| &plan.plan_id == replacement_plan_id) =>
            {
                plan_index += 1;
                WakeLifecycle::AwaitingPlay
            }
            (
                WakeLifecycle::Unsatisfied | WakeLifecycle::AwaitingReplacement,
                WakeLifecycleEvent::PlanHeld {
                    prior_plan_id: Some(prior_plan_id),
                    plan_id,
                    basis_sign_ids,
                    policy,
                    ..
                },
            ) if plans
                .get(plan_index - 1)
                .is_some_and(|plan| &plan.plan_id == prior_plan_id)
                && plans.get(plan_index).is_some_and(|plan| {
                    &plan.plan_id == plan_id
                        && plan.hold.as_ref().is_some_and(|hold| {
                            hold.basis.sign_ids() == basis_sign_ids && &hold.policy == policy
                        })
                }) =>
            {
                plan_index += 1;
                WakeLifecycle::Held
            }
            (state, WakeLifecycleEvent::Lulled { .. })
                if !matches!(state, WakeLifecycle::Lulled | WakeLifecycle::Failed) =>
            {
                WakeLifecycle::Lulled
            }
            (state, WakeLifecycleEvent::Failed { .. })
                if !matches!(state, WakeLifecycle::Lulled | WakeLifecycle::Failed) =>
            {
                WakeLifecycle::Failed
            }
            _ => return Err(BodyLifecycleError::InvalidTransition),
        };
    }
    if replayed == lifecycle && plan_index == plans.len() {
        Ok(())
    } else {
        Err(BodyLifecycleError::InvalidTransition)
    }
}
