use alloc::vec;
use conduit_body::{
    BodyBiographyEvidence, BodyBiographyRecordKind, BodyLifecycleSession, PurposeCompletionPolicy,
    PurposeObligation, PurposeObligationState, PurposeRefusal, PurposeState, WakeLifecycleEvent,
};

/// Derive the tutorial's optional purpose only from retained body evidence.
/// No chapter counter, Presenter output, or browser-local interaction can mark
/// an obligation complete.
pub fn purpose_state(body: &BodyLifecycleSession) -> Result<PurposeState, PurposeRefusal> {
    purpose_state_from_evidence(body.evidence())
}

pub fn purpose_state_from_evidence(
    evidence: &BodyBiographyEvidence,
) -> Result<PurposeState, PurposeRefusal> {
    let born = evidence.records.iter().find_map(|record| {
        matches!(record.kind, BodyBiographyRecordKind::Born { .. }).then(|| record.sign_id.clone())
    });
    let mut woke = None;
    let mut planned = None;
    let mut played = None;
    let mut repair = PurposeObligationState::Pending;
    for wake in &evidence.wakes {
        for event in &wake.events {
            match event {
                WakeLifecycleEvent::Woke { sign_id } => woke.get_or_insert_with(|| sign_id.clone()),
                WakeLifecycleEvent::PlanReady { sign_id, .. } => {
                    planned.get_or_insert_with(|| sign_id.clone())
                }
                WakeLifecycleEvent::PlayStarted { sign_id, .. } => {
                    played.get_or_insert_with(|| sign_id.clone());
                    if matches!(repair, PurposeObligationState::RepairRequired { .. }) {
                        repair = PurposeObligationState::Satisfied {
                            evidence_sign_ids: vec![sign_id.clone()],
                        };
                    }
                    continue;
                }
                WakeLifecycleEvent::Failed { sign_id } => {
                    repair = PurposeObligationState::RepairRequired {
                        failure_sign_id: sign_id.clone(),
                    };
                    continue;
                }
                _ => continue,
            };
        }
    }
    let joined = evidence
        .records
        .iter()
        .filter(|record| matches!(record.kind, BodyBiographyRecordKind::HostJoined { .. }))
        .map(|record| record.sign_id.clone())
        .nth(1);
    let revision = evidence
        .records
        .last()
        .map_or(evidence.body.birth_sequence, |record| record.sequence);
    let state = PurposeState {
        purpose_id: "purpose/orifina-tutorial@1".into(),
        revision,
        summary: "Teach one real Body lifecycle".into(),
        completion_policy: PurposeCompletionPolicy::ExplicitFulfillmentReadiness,
        obligations: vec![
            exact_obligation("born", "Be born as one retained body", born),
            exact_obligation("wake", "Wake through an admitted plan and Play", woke),
            exact_obligation("plan-ready", "Establish an exact current plan", planned),
            exact_obligation("play-started", "Start ordinary form work", played),
            PurposeObligation {
                obligation_id: "repair-fault".into(),
                summary: "Repair a real failed Wake".into(),
                state: repair,
            },
            exact_obligation("add-host", "Admit another host", joined),
        ],
    };
    state.validate()?;
    Ok(state)
}

fn exact_obligation(
    obligation_id: &str,
    summary: &str,
    evidence: Option<conduit_core::SignId>,
) -> PurposeObligation {
    PurposeObligation {
        obligation_id: obligation_id.into(),
        summary: summary.into(),
        state: evidence.map_or(PurposeObligationState::Pending, |sign_id| {
            PurposeObligationState::Satisfied {
                evidence_sign_ids: vec![sign_id],
            }
        }),
    }
}
