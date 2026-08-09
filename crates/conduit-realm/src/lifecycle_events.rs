//! Machine-readable evidence events for Realm deployment continuity.

use conduit_core::{ActivePlayId, EvidenceId, PlanId};
use serde::{Deserialize, Serialize};

use crate::lifecycle_identity::validate_lifecycle_ids;
use crate::{
    ActivationId, ActivationLifecycle, ActivationPlan, DeploymentState, RealmLifecycleError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentLifecycleEvent {
    Installed {
        evidence_id: EvidenceId,
    },
    Activated {
        activation_id: ActivationId,
        evidence_id: EvidenceId,
    },
    ActivationRetained {
        activation_id: ActivationId,
        evidence_id: EvidenceId,
    },
    Undeployed {
        evidence_id: EvidenceId,
    },
}

impl DeploymentLifecycleEvent {
    pub fn evidence_id(&self) -> &EvidenceId {
        match self {
            Self::Installed { evidence_id }
            | Self::Activated { evidence_id, .. }
            | Self::ActivationRetained { evidence_id, .. }
            | Self::Undeployed { evidence_id } => evidence_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationLifecycleEvent {
    Activated {
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
    Deactivated {
        evidence_id: EvidenceId,
    },
    Failed {
        evidence_id: EvidenceId,
    },
}

impl ActivationLifecycleEvent {
    pub fn evidence_id(&self) -> &EvidenceId {
        match self {
            Self::Activated { evidence_id }
            | Self::PlanReady { evidence_id, .. }
            | Self::PlayStarted { evidence_id, .. }
            | Self::BecameUnsatisfied { evidence_id, .. }
            | Self::Replanned { evidence_id, .. }
            | Self::SamePlanObserved { evidence_id, .. }
            | Self::Deactivated { evidence_id }
            | Self::Failed { evidence_id } => evidence_id,
        }
    }
}

pub(crate) fn validate_deployment_events(
    events: &[DeploymentLifecycleEvent],
    evidence: &[EvidenceId],
) -> Result<(), RealmLifecycleError> {
    if events.len() != evidence.len()
        || events
            .iter()
            .zip(evidence)
            .any(|(event, identity)| event.evidence_id() != identity)
        || !matches!(
            events.first(),
            Some(DeploymentLifecycleEvent::Installed { .. })
        )
    {
        return Err(RealmLifecycleError::InvalidTransition);
    }
    for event in events {
        validate_lifecycle_ids(&[event.evidence_id().as_str()])?;
        if let DeploymentLifecycleEvent::Activated { activation_id, .. }
        | DeploymentLifecycleEvent::ActivationRetained { activation_id, .. } = event
        {
            validate_lifecycle_ids(&[activation_id.as_str()])?;
        }
    }
    Ok(())
}

pub(crate) fn validate_deployment_event_state(
    events: &[DeploymentLifecycleEvent],
    state: &DeploymentState,
) -> Result<(), RealmLifecycleError> {
    let mut replayed = DeploymentState::Inactive;
    for event in events.iter().skip(1) {
        replayed = match (&replayed, event) {
            (
                DeploymentState::Inactive,
                DeploymentLifecycleEvent::Activated { activation_id, .. },
            ) => DeploymentState::Active {
                activation_id: activation_id.clone(),
            },
            (
                DeploymentState::Active { activation_id },
                DeploymentLifecycleEvent::ActivationRetained {
                    activation_id: retained,
                    ..
                },
            ) if activation_id == retained => DeploymentState::Inactive,
            (DeploymentState::Inactive, DeploymentLifecycleEvent::Undeployed { .. }) => {
                DeploymentState::Undeployed
            }
            _ => return Err(RealmLifecycleError::InvalidTransition),
        };
    }
    if &replayed == state {
        Ok(())
    } else {
        Err(RealmLifecycleError::InvalidTransition)
    }
}

pub(crate) fn validate_activation_events(
    events: &[ActivationLifecycleEvent],
    evidence: &[EvidenceId],
    lifecycle: ActivationLifecycle,
    plans: &[ActivationPlan],
) -> Result<(), RealmLifecycleError> {
    if events.len() != evidence.len()
        || events
            .iter()
            .zip(evidence)
            .any(|(event, identity)| event.evidence_id() != identity)
        || !matches!(
            events.first(),
            Some(ActivationLifecycleEvent::Activated { .. })
        )
    {
        return Err(RealmLifecycleError::InvalidTransition);
    }
    for event in events {
        validate_activation_event_ids(event)?;
    }
    let (replayed, consumed_plans) = replay_activation_events(events, plans)?;
    if replayed == lifecycle && consumed_plans == plans.len() {
        Ok(())
    } else {
        Err(RealmLifecycleError::InvalidTransition)
    }
}

fn replay_activation_events(
    events: &[ActivationLifecycleEvent],
    plans: &[ActivationPlan],
) -> Result<(ActivationLifecycle, usize), RealmLifecycleError> {
    let mut lifecycle = ActivationLifecycle::AwaitingPlan;
    let mut plan_index = 0usize;
    for event in events.iter().skip(1) {
        match (lifecycle, event) {
            (
                ActivationLifecycle::AwaitingPlan,
                ActivationLifecycleEvent::PlanReady { plan_id, .. },
            ) if plans
                .get(plan_index)
                .is_some_and(|plan| plan.plan_id == *plan_id && plan_index == 0) =>
            {
                plan_index += 1;
                lifecycle = ActivationLifecycle::AwaitingPlay;
            }
            (
                ActivationLifecycle::AwaitingPlay,
                ActivationLifecycleEvent::PlayStarted {
                    plan_id,
                    active_play_id,
                    ..
                },
            ) if plans.get(plan_index.saturating_sub(1)).is_some_and(|plan| {
                plan.plan_id == *plan_id && plan.active_play_id.as_ref() == Some(active_play_id)
            }) =>
            {
                lifecycle = ActivationLifecycle::Active;
            }
            (
                ActivationLifecycle::Active,
                ActivationLifecycleEvent::SamePlanObserved { plan_id, .. },
            ) if current_plan(plans, plan_index) == Some(plan_id) => {}
            (
                ActivationLifecycle::Active,
                ActivationLifecycleEvent::BecameUnsatisfied { plan_id, .. },
            ) if current_plan(plans, plan_index) == Some(plan_id) => {
                lifecycle = ActivationLifecycle::Unsatisfied;
            }
            (
                ActivationLifecycle::Unsatisfied,
                ActivationLifecycleEvent::Replanned {
                    prior_plan_id,
                    replacement_plan_id,
                    ..
                },
            ) if current_plan(plans, plan_index) == Some(prior_plan_id)
                && plans
                    .get(plan_index)
                    .is_some_and(|plan| plan.plan_id == *replacement_plan_id) =>
            {
                plan_index += 1;
                lifecycle = ActivationLifecycle::AwaitingPlay;
            }
            (_, ActivationLifecycleEvent::Deactivated { .. })
                if !matches!(
                    lifecycle,
                    ActivationLifecycle::Deactivated | ActivationLifecycle::Failed
                ) =>
            {
                lifecycle = ActivationLifecycle::Deactivated;
            }
            (_, ActivationLifecycleEvent::Failed { .. })
                if !matches!(
                    lifecycle,
                    ActivationLifecycle::Deactivated | ActivationLifecycle::Failed
                ) =>
            {
                lifecycle = ActivationLifecycle::Failed;
            }
            _ => return Err(RealmLifecycleError::InvalidTransition),
        }
    }
    Ok((lifecycle, plan_index))
}

fn current_plan(plans: &[ActivationPlan], consumed_plans: usize) -> Option<&PlanId> {
    plans
        .get(consumed_plans.checked_sub(1)?)
        .map(|plan| &plan.plan_id)
}

fn validate_activation_event_ids(
    event: &ActivationLifecycleEvent,
) -> Result<(), RealmLifecycleError> {
    validate_lifecycle_ids(&[event.evidence_id().as_str()])?;
    match event {
        ActivationLifecycleEvent::PlanReady { plan_id, .. }
        | ActivationLifecycleEvent::BecameUnsatisfied { plan_id, .. }
        | ActivationLifecycleEvent::SamePlanObserved { plan_id, .. } => {
            validate_lifecycle_ids(&[plan_id.as_str()])
        }
        ActivationLifecycleEvent::PlayStarted {
            plan_id,
            active_play_id,
            ..
        } => validate_lifecycle_ids(&[plan_id.as_str(), active_play_id.as_str()]),
        ActivationLifecycleEvent::Replanned {
            prior_plan_id,
            replacement_plan_id,
            ..
        } => {
            validate_lifecycle_ids(&[prior_plan_id.as_str(), replacement_plan_id.as_str()])?;
            if prior_plan_id == replacement_plan_id {
                Err(RealmLifecycleError::StalePlan)
            } else {
                Ok(())
            }
        }
        ActivationLifecycleEvent::Activated { .. }
        | ActivationLifecycleEvent::Deactivated { .. }
        | ActivationLifecycleEvent::Failed { .. } => Ok(()),
    }
}
