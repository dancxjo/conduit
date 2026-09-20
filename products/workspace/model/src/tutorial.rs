//! Renderer-neutral guidance projected only from authoritative Body truth.
use crate::WorkspaceBody;
use alloc::{format, vec, vec::Vec};
use conduit_body::{
    BodyBiographyRecordKind, BodyState, FulfillmentReadiness, PurposeCompletionPolicy,
    PurposeObligation, PurposeObligationState, PurposeRefusal, PurposeState, WakeLifecycleEvent,
    derive_fulfillment_readiness,
};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, PresentationMechanism, SemanticAction,
    SemanticApplicationView, SemanticPresentationNode, StatusKind,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TutorialPlayback {
    Lulled,
    Preparing,
    Playing,
    Idle,
    Completed,
    Cancelled,
    Failed,
    Refused,
    Stopped,
    Fulfilled,
}

struct Guidance {
    phase: &'static str,
    title: &'static str,
    detail: &'static str,
    action: &'static str,
    label: &'static str,
}

pub fn presentation(
    body: &WorkspaceBody,
    revision: u32,
    playback: TutorialPlayback,
) -> Result<SemanticApplicationView, conduit_presentation::SemanticPresentationRefusal> {
    let purpose = purpose_state(body).expect("validated Body evidence must project valid purpose");
    let guidance = guidance(body, playback, &purpose);
    let readiness = derive_fulfillment_readiness(&purpose)
        .expect("validated tutorial purpose must derive readiness");
    let readiness_text = match &readiness {
        FulfillmentReadiness::Ready { .. } => "ready".into(),
        FulfillmentReadiness::Unavailable { .. } => "unavailable".into(),
        FulfillmentReadiness::NotReady { reasons, .. } => {
            format!("not ready · {} exact obligation(s) remain", reasons.len())
        }
    };
    let view = SemanticApplicationView {
        revision,
        root: node(
            "body-tutorial",
            PresentationMechanism::Panel {
                title: guidance.title.into(),
            },
            vec![
                node(
                    "tutorial-guidance",
                    PresentationMechanism::Status {
                        kind: if guidance.phase == "repair" {
                            StatusKind::Warning
                        } else {
                            StatusKind::Ordinary
                        },
                        title: format!("Tutorial · {}", guidance.phase),
                        detail: guidance.detail.into(),
                    },
                    vec![],
                ),
                node(
                    "tutorial-next-action",
                    PresentationMechanism::Action(SemanticAction {
                        identity: guidance.action.into(),
                        event: ApplicationEventKind::Activate,
                        label: guidance.label.into(),
                        availability: ActionAvailability::Available,
                    }),
                    vec![],
                ),
                node(
                    "tutorial-purpose",
                    PresentationMechanism::Status {
                        kind: StatusKind::Ordinary,
                        title: "Purpose · exact readiness".into(),
                        detail: readiness_text,
                    },
                    vec![],
                ),
            ],
        ),
    };
    view.lower()?;
    Ok(view)
}

/// Derive the tutorial's optional purpose only from retained Body evidence.
/// No chapter counter, Presenter output, or browser-local interaction can mark
/// an obligation complete.
pub fn purpose_state(body: &WorkspaceBody) -> Result<PurposeState, PurposeRefusal> {
    let evidence = body.evidence();
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
            exact_obligation("born", "Be born as one retained Body", born),
            exact_obligation("wake", "Wake through an admitted Plan and Play", woke),
            exact_obligation("plan-ready", "Establish an exact current Plan", planned),
            exact_obligation("play-started", "Start ordinary Form work", played),
            PurposeObligation {
                obligation_id: "repair-fault".into(),
                summary: "Repair a real failed Wake".into(),
                state: repair,
            },
            exact_obligation("add-host", "Admit another Host", joined),
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

fn guidance(body: &WorkspaceBody, playback: TutorialPlayback, purpose: &PurposeState) -> Guidance {
    let evidence = body.evidence();
    if matches!(evidence.body.state, BodyState::Fulfilled { .. }) {
        return Guidance {
            phase: "fulfilled",
            title: "Its useful life is complete",
            detail: "Fulfilled is terminal, not deletion. Inspect the closed biography.",
            action: "body.inspect-lifecycle",
            label: "Inspect lifecycle evidence",
        };
    }
    if purpose.obligations.iter().any(|obligation| {
        obligation.obligation_id == "repair-fault"
            && matches!(
                obligation.state,
                PurposeObligationState::RepairRequired { .. }
            )
    }) {
        return Guidance {
            phase: "repair",
            title: "Inspect the real fault",
            detail: "This biography contains a failed Wake. Inspect its evidence, then change the actual workset or available Hosts before waking again.",
            action: "body.inspect-lifecycle",
            label: "Inspect lifecycle evidence",
        };
    }
    if matches!(evidence.body.state, BodyState::Lulled) && evidence.wakes.is_empty() {
        return Guidance {
            phase: "wake",
            title: "Wake this Body",
            detail: "Birth made one retained Body. Wake admits its first exact Plan and Play without creating another Body.",
            action: "body.wake",
            label: "Wake the retained Body",
        };
    }
    if matches!(evidence.body.state, BodyState::Lulled) {
        return Guidance {
            phase: "lull",
            title: "Retained rest is not completion",
            detail: "The Body is lulled: its identity, Forms, and biography remain.",
            action: "body.wake",
            label: "Wake the retained Body",
        };
    }
    let another_host_joined = evidence
        .records
        .iter()
        .filter(|record| matches!(record.kind, BodyBiographyRecordKind::HostJoined { .. }))
        .count()
        > 1;
    if evidence.body.workload_revision > 0 && !another_host_joined {
        return Guidance {
            phase: "add-host",
            title: "Invite another Host",
            detail: "The workset changed without rebirth. Invite another Host through the same finite Body admission path, then inspect its exact membership and offers.",
            action: "body.invite-host",
            label: "Invite another Host",
        };
    }
    if evidence.body.workload_revision > 0 {
        return Guidance {
            phase: "revised",
            title: "One Body, a changed workset",
            detail: "The workload revision changed without rebirth. The current Plan realizes revised Forms for this same Body.",
            action: "body.inspect-lifecycle",
            label: "Inspect the current realization",
        };
    }
    if evidence.wakes.len() > 1 {
        return Guidance {
            phase: "continuity",
            title: "The same Body woke again",
            detail: "A fresh Wake, Plan, and Play continue one retained biography.",
            action: "body.open-library",
            label: "Browse this Body's Forms",
        };
    }
    Guidance {
        phase: "living",
        title: "Use it, then leave it useful",
        detail: if playback == TutorialPlayback::Idle {
            "Idle means admitted work awaits future input. Finite means bounded state, queues, authority, and obligations—not a short lifetime."
        } else {
            "Interact more than once. A finite Body may remain awake indefinitely because its instantaneous and retained bounds stay finite."
        },
        action: "body.use-current",
        label: "Use the current Form",
    }
}

fn node(
    key: &str,
    mechanism: PresentationMechanism,
    children: Vec<SemanticPresentationNode>,
) -> SemanticPresentationNode {
    SemanticPresentationNode {
        key: key.into(),
        mechanism,
        children,
    }
}
