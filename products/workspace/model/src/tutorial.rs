//! Renderer-neutral guidance projected only from authoritative Body truth.
use crate::WorkspaceBody;
use alloc::{format, vec, vec::Vec};
use conduit_body::{
    BodyBiographyEvidence, BodyBiographyRecordKind, BodyState, FulfillmentReadiness,
    PurposeCompletionPolicy, PurposeObligation, PurposeObligationState, PurposeRefusal,
    PurposeState, WakeLifecycleEvent, derive_fulfillment_readiness,
};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, Face, FaceContext, FaceFocus, FaceRefusal,
    GenerativePresenterBounds, GenerativePresenterRefusal, GenerativePresenterRequest,
    OrifinaPresentationRefusal, Presentation, PresentationAction, PresentationActionAvailability,
    PresentationDisclosureLevel, PresentationError, PresentationMechanism, PresentationProperty,
    PresentationPropertyValue, PresentationText, SemanticAction, SemanticApplicationView,
    SemanticPresentationNode, StatusKind, orifina_completion_presenter_policy,
    project_orifina_purpose_presentation,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TutorialPresenterRefusal {
    InvalidPurpose(PurposeRefusal),
    InvalidFace(FaceRefusal),
    InvalidPurposePresentation(OrifinaPresentationRefusal),
    InvalidActionPresentation(PresentationError),
    InvalidRequest(GenerativePresenterRefusal),
}

/// Build the exact bounded request used to voice the current tutorial state.
///
/// Purpose and readiness come from retained body evidence. The reviewed policy
/// remains separate implementation input, and the returned action is only a
/// description: an operator must still select and authorize Fulfillment.
pub fn generative_request(
    body: &WorkspaceBody,
    request_identity: alloc::string::String,
    presentation_revision: u64,
    playback: TutorialPlayback,
) -> Result<GenerativePresenterRequest, TutorialPresenterRefusal> {
    let purpose = purpose_state(body).map_err(TutorialPresenterRefusal::InvalidPurpose)?;
    let readiness =
        derive_fulfillment_readiness(&purpose).map_err(TutorialPresenterRefusal::InvalidPurpose)?;
    let guidance = guidance(body.evidence(), playback, &purpose);
    let purpose_projection = project_orifina_purpose_presentation(
        body.evidence().body.body_id.clone(),
        purpose.revision,
        &purpose,
        presentation_revision,
    )
    .map_err(TutorialPresenterRefusal::InvalidPurposePresentation)?;
    let mut face = Face::project(
        &body.evidence().body,
        body.realization().map(|realization| &realization.wake),
        presentation_revision,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .map_err(TutorialPresenterRefusal::InvalidFace)?;
    let body_subject = format!("body/{}", body.evidence().body.body_id.as_str());
    // The conversational Presenter receives the same tutorial meaning as the
    // graphical view, not merely the generic Face's identifier-heavy summary.
    // Exact identities and lifecycle facts remain structured properties.
    face.presentation
        .text
        .retain(|text| text.subject != body_subject);
    face.presentation.text.push(PresentationText {
        subject: body_subject.clone(),
        text: format!("{}: {}", guidance.title, guidance.detail),
    });
    face.presentation.properties.push(PresentationProperty {
        subject: body_subject.clone(),
        name: "tutorial-phase".into(),
        value: PresentationPropertyValue::Text(guidance.phase.into()),
    });
    let (identity, intent, label) = if matches!(readiness, FulfillmentReadiness::Ready { .. })
        && !matches!(body.evidence().body.state, BodyState::Fulfilled { .. })
    {
        (
            "body.fulfill",
            "conduit.intent/fulfill@1",
            "Fulfill this body",
        )
    } else {
        (
            guidance.action,
            "conduit.intent/tutorial-next@1",
            guidance.label,
        )
    };
    let purpose_subjects = purpose_projection
        .subjects
        .into_iter()
        .filter(|subject| subject.identity != body_subject);
    face.presentation.subjects.extend(purpose_subjects);
    face.presentation
        .relationships
        .extend(purpose_projection.relationships);
    face.presentation
        .properties
        .extend(purpose_projection.properties);
    face.presentation.text.extend(purpose_projection.text);
    face.presentation.disclosures.extend(
        purpose_projection
            .disclosures
            .into_iter()
            .filter(|disclosure| disclosure.subject != body_subject),
    );
    face.presentation
        .basis
        .sign_ids
        .extend(purpose_projection.basis.sign_ids);
    face.presentation.basis.sign_ids.sort();
    face.presentation.basis.sign_ids.dedup();
    face.presentation.actions.push(PresentationAction {
        identity: identity.into(),
        intent: intent.into(),
        target: body_subject,
        label: label.into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: PresentationActionAvailability::Available,
    });
    face.presentation = Presentation::new_with_interactions(
        face.presentation.revision,
        face.presentation.basis,
        face.presentation.subjects,
        face.presentation.relationships,
        face.presentation.properties,
        face.presentation.text,
        face.presentation.actions,
        face.presentation.inputs,
        face.presentation.disclosures,
    )
    .map_err(TutorialPresenterRefusal::InvalidActionPresentation)?;
    GenerativePresenterRequest::from_face(
        request_identity,
        orifina_completion_presenter_policy(),
        &face,
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .map_err(TutorialPresenterRefusal::InvalidRequest)
}

pub fn presentation(
    body: &WorkspaceBody,
    revision: u32,
    playback: TutorialPlayback,
) -> Result<SemanticApplicationView, conduit_presentation::SemanticPresentationRefusal> {
    presentation_from_evidence(body.evidence(), revision, playback)
}

/// Project tutorial guidance from an exact retained body biography at a host
/// boundary. This lets the ordinary resident Tutorial Form consume the same
/// semantic truth without reaching through a product-owned `WorkspaceBody`.
pub fn presentation_from_evidence(
    evidence: &BodyBiographyEvidence,
    revision: u32,
    playback: TutorialPlayback,
) -> Result<SemanticApplicationView, conduit_presentation::SemanticPresentationRefusal> {
    let purpose = purpose_state_from_evidence(evidence)
        .expect("validated Body evidence must project valid purpose");
    let guidance = guidance(evidence, playback, &purpose);
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

/// Derive the tutorial's optional purpose only from retained body evidence.
/// No chapter counter, Presenter output, or browser-local interaction can mark
/// an obligation complete.
pub fn purpose_state(body: &WorkspaceBody) -> Result<PurposeState, PurposeRefusal> {
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

fn guidance(
    evidence: &BodyBiographyEvidence,
    playback: TutorialPlayback,
    purpose: &PurposeState,
) -> Guidance {
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
            detail: "This biography contains a failed Wake. Inspect its evidence, then change the actual workset or available hosts before waking again.",
            action: "body.inspect-lifecycle",
            label: "Inspect lifecycle evidence",
        };
    }
    if matches!(evidence.body.state, BodyState::Lulled) && evidence.wakes.is_empty() {
        return Guidance {
            phase: "wake",
            title: "Wake this body",
            detail: "Birth made one retained body. Wake admits its first exact plan and Play without creating another body.",
            action: "body.wake",
            label: "Wake the retained body",
        };
    }
    if matches!(evidence.body.state, BodyState::Lulled) {
        return Guidance {
            phase: "lull",
            title: "Retained rest is not completion",
            detail: "The body is lulled: its identity, Forms, and biography remain.",
            action: "body.wake",
            label: "Wake the retained body",
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
            title: "Invite another host",
            detail: "The workset changed without rebirth. Invite another host through the same finite Body admission path, then inspect its exact membership and offers.",
            action: "body.invite-host",
            label: "Invite another host",
        };
    }
    if evidence.body.workload_revision > 0 {
        return Guidance {
            phase: "revised",
            title: "One body, a changed workset",
            detail: "The workload revision changed without rebirth. The current plan realizes revised Forms for this same body.",
            action: "body.inspect-lifecycle",
            label: "Inspect the current realization",
        };
    }
    if evidence.wakes.len() > 1 {
        return Guidance {
            phase: "continuity",
            title: "The same body woke again",
            detail: "A fresh Wake, Plan, and Play continue one retained biography.",
            action: "body.open-library",
            label: "Browse this body's Forms",
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
        label: "Use the current form",
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
