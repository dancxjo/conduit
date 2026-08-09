//! Structural validation for bounded Realm lifecycle histories.

use alloc::vec::Vec;
use conduit_core::EvidenceId;

use crate::lifecycle_identity::validate_lifecycle_ids;
use crate::{
    ActivationLifecycle, ActivationPlan, ActivationPlanState, RealmLifecycleError,
    MAX_LIFECYCLE_ID_BYTES,
};

pub(crate) fn validate_plan_history(
    lifecycle: ActivationLifecycle,
    plans: &[ActivationPlan],
) -> Result<(), RealmLifecycleError> {
    if plans.iter().any(|item| {
        item.plan_id.as_str().is_empty()
            || item.plan_id.as_str().len() > MAX_LIFECYCLE_ID_BYTES
            || item.active_play_id.as_ref().is_some_and(|play| {
                play.as_str().is_empty() || play.as_str().len() > MAX_LIFECYCLE_ID_BYTES
            })
    }) || plans.iter().enumerate().any(|(index, item)| {
        plans[..index]
            .iter()
            .any(|prior| prior.plan_id == item.plan_id)
    }) || plans.iter().enumerate().any(|(index, item)| {
        index + 1 < plans.len() && item.state != ActivationPlanState::Superseded
    }) {
        return Err(RealmLifecycleError::InvalidTransition);
    }
    let current = plans.last();
    let valid = match lifecycle {
        ActivationLifecycle::AwaitingPlan => plans.is_empty(),
        ActivationLifecycle::AwaitingPlay => current.is_some_and(|item| {
            item.state == ActivationPlanState::AwaitingPlay && item.active_play_id.is_none()
        }),
        ActivationLifecycle::Active => current.is_some_and(|item| {
            item.state == ActivationPlanState::Playing && item.active_play_id.is_some()
        }),
        ActivationLifecycle::Unsatisfied => current.is_some_and(|item| {
            item.state == ActivationPlanState::Unsatisfied && item.active_play_id.is_some()
        }),
        ActivationLifecycle::Deactivated | ActivationLifecycle::Failed => true,
    };
    valid
        .then_some(())
        .ok_or(RealmLifecycleError::InvalidTransition)
}

pub(crate) fn push_evidence(
    evidence: &mut Vec<EvidenceId>,
    evidence_id: EvidenceId,
    capacity: usize,
) -> Result<(), RealmLifecycleError> {
    validate_lifecycle_ids(&[evidence_id.as_str()])?;
    if evidence.contains(&evidence_id) {
        return Err(RealmLifecycleError::DuplicateEvidence);
    }
    if evidence.len() >= capacity {
        return Err(RealmLifecycleError::EvidenceCapacityExhausted);
    }
    evidence.push(evidence_id);
    Ok(())
}

pub(crate) fn validate_evidence(
    evidence: &[EvidenceId],
    capacity: usize,
) -> Result<(), RealmLifecycleError> {
    if evidence.is_empty() || evidence.len() > capacity {
        return Err(RealmLifecycleError::EvidenceCapacityExhausted);
    }
    for (index, item) in evidence.iter().enumerate() {
        validate_lifecycle_ids(&[item.as_str()])?;
        if evidence[..index].contains(item) {
            return Err(RealmLifecycleError::DuplicateEvidence);
        }
    }
    Ok(())
}
