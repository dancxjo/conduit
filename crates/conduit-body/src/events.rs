use conduit_core::{ActivePlayId, EvidenceId, PlanId};
use serde::{Deserialize, Serialize};

use crate::{BodyLifecycleError, BodyState, WakeId, WakeLifecycle, WakePlan};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyLifecycleEvent {
    Born {
        evidence_id: EvidenceId,
    },
    Woke {
        wake_id: WakeId,
        evidence_id: EvidenceId,
    },
    LullRetained {
        wake_id: WakeId,
        evidence_id: EvidenceId,
    },
}

impl BodyLifecycleEvent {
    pub fn evidence_id(&self) -> &EvidenceId {
        match self {
            Self::Born { evidence_id }
            | Self::Woke { evidence_id, .. }
            | Self::LullRetained { evidence_id, .. } => evidence_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakeLifecycleEvent {
    Woke {
        evidence_id: EvidenceId,
    },
    PlanReady {
        plan_id: PlanId,
        evidence_id: EvidenceId,
    },
    PlayStarted {
        plan_id: PlanId,
        active_play_id: ActivePlayId,
        evidence_id: EvidenceId,
    },
    BecameUnsatisfied {
        plan_id: PlanId,
        evidence_id: EvidenceId,
    },
    Replanned {
        prior_plan_id: PlanId,
        replacement_plan_id: PlanId,
        evidence_id: EvidenceId,
    },
    SamePlanObserved {
        plan_id: PlanId,
        evidence_id: EvidenceId,
    },
    Lulled {
        evidence_id: EvidenceId,
    },
    Failed {
        evidence_id: EvidenceId,
    },
}

impl WakeLifecycleEvent {
    pub fn evidence_id(&self) -> &EvidenceId {
        match self {
            Self::Woke { evidence_id }
            | Self::PlanReady { evidence_id, .. }
            | Self::PlayStarted { evidence_id, .. }
            | Self::BecameUnsatisfied { evidence_id, .. }
            | Self::Replanned { evidence_id, .. }
            | Self::SamePlanObserved { evidence_id, .. }
            | Self::Lulled { evidence_id }
            | Self::Failed { evidence_id } => evidence_id,
        }
    }
}

pub(crate) fn validate_body_events(
    events: &[BodyLifecycleEvent],
    evidence: &[EvidenceId],
    state: &BodyState,
) -> Result<(), BodyLifecycleError> {
    if events.len() != evidence.len()
        || events
            .iter()
            .zip(evidence)
            .any(|(event, id)| event.evidence_id() != id)
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
    evidence: &[EvidenceId],
    lifecycle: WakeLifecycle,
    plans: &[WakePlan],
) -> Result<(), BodyLifecycleError> {
    if events.len() != evidence.len()
        || events
            .iter()
            .zip(evidence)
            .any(|(event, id)| event.evidence_id() != id)
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
