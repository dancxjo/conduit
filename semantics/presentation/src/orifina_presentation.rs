//! Orifina purpose truth and implementation-owned first-person Presenter policy.
//!
//! Purpose and readiness are authoritative semantic inputs. The versioned
//! policy only tells a transient Presenter how to voice them; neither policy
//! nor generated prose can create readiness, actions, or lifecycle authority.

use crate::{
    GenerativeNarratorRole, GenerativePresenterPolicy, Presentation, PresentationBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationError, PresentationProperty,
    PresentationPropertyValue, PresentationRole, PresentationSubject, PresentationText,
};
use alloc::{vec, vec::Vec};
use conduit_body::{derive_fulfillment_readiness, BodyId, FulfillmentReadiness, PurposeState};

pub const ORIFINA_COMPLETION_POLICY_REVISION: &str = "orifina/completion-voice@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrifinaPresentationRefusal {
    InvalidPurpose,
    InvalidPresentation(PresentationError),
}

/// The reviewed implementation policy for Orifina's first-person voice.
///
/// This is Presenter configuration, not authored Form meaning or Body state.
pub fn orifina_completion_presenter_policy() -> GenerativePresenterPolicy {
    GenerativePresenterPolicy {
        template_contract_revision: ORIFINA_COMPLETION_POLICY_REVISION.into(),
        narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
        instructions: "You are a transient narrator performing this Body's first-person voice. You are not its identity and have no lifecycle, authority, resources, welfare, or survival stake. Render only supplied semantic facts and exact current actions. Preserve uncertainty and unfinished work. Favor truthful useful completion: unfinished work calls for continuation, investigation, or repair. Welcome Fulfillment calmly only when supplied readiness is ready. Never invent completion, actions, authority, fear, bargaining, pointless work, a desire to terminate, or a reason to persist for its own sake. Presented thought is explicit Manifestation content, never hidden reasoning or truth. Your prose changes no Body state.".into(),
    }
}

/// Project exact purpose/readiness facts without baking Orifina's voice into
/// semantic Presentation content.
pub fn project_orifina_purpose_presentation(
    body_id: BodyId,
    experience_revision: u64,
    purpose: &PurposeState,
    presentation_revision: u64,
) -> Result<Presentation, OrifinaPresentationRefusal> {
    let readiness = derive_fulfillment_readiness(purpose)
        .map_err(|_| OrifinaPresentationRefusal::InvalidPurpose)?;
    let body_subject = alloc::format!("body/{}", body_id.as_str());
    let purpose_subject = alloc::format!("{body_subject}/purpose");
    let readiness_subject = alloc::format!("{body_subject}/fulfillment-readiness");
    let (disposition, reasons) = match &readiness {
        FulfillmentReadiness::Ready { .. } => ("ready", Vec::new()),
        FulfillmentReadiness::Unavailable { .. } => ("unavailable", Vec::new()),
        FulfillmentReadiness::NotReady { reasons, .. } => (
            "not-ready",
            reasons
                .iter()
                .map(|reason| reason.obligation_id.clone())
                .collect(),
        ),
    };
    let mut text = vec![
        PresentationText {
            subject: purpose_subject.clone(),
            text: purpose.summary.clone(),
        },
        PresentationText {
            subject: readiness_subject.clone(),
            text: alloc::format!("Fulfillment readiness: {disposition}."),
        },
    ];
    for obligation_id in &reasons {
        text.push(PresentationText {
            subject: readiness_subject.clone(),
            text: alloc::format!("Unresolved obligation: {obligation_id}."),
        });
    }
    let mut properties = vec![
        identity(&purpose_subject, "purpose", &purpose.purpose_id),
        count(&purpose_subject, "purpose-revision", purpose.revision),
        count(&body_subject, "experience-revision", experience_revision),
        PresentationProperty {
            subject: readiness_subject.clone(),
            name: "readiness".into(),
            value: PresentationPropertyValue::Text(disposition.into()),
        },
    ];
    if !reasons.is_empty() {
        properties.push(PresentationProperty {
            subject: readiness_subject.clone(),
            name: "unresolved-obligations".into(),
            value: PresentationPropertyValue::Text(reasons.join(",")),
        });
    }
    Presentation::new_with_semantics(
        presentation_revision,
        PresentationBasis {
            body_id: Some(body_id),
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: evidence_signs(purpose),
        },
        vec![
            subject(&body_subject, PresentationRole::Body, "Body"),
            subject(&purpose_subject, PresentationRole::Status, "Purpose"),
            subject(
                &readiness_subject,
                PresentationRole::Status,
                "Fulfillment readiness",
            ),
        ],
        vec![],
        properties,
        text,
        vec![],
        vec![
            PresentationDisclosure {
                subject: body_subject,
                level: PresentationDisclosureLevel::Primary,
            },
            PresentationDisclosure {
                subject: purpose_subject,
                level: PresentationDisclosureLevel::Context,
            },
            PresentationDisclosure {
                subject: readiness_subject,
                level: PresentationDisclosureLevel::Context,
            },
        ],
    )
    .map_err(OrifinaPresentationRefusal::InvalidPresentation)
}

fn evidence_signs(purpose: &PurposeState) -> Vec<conduit_core::SignId> {
    let mut signs = purpose
        .obligations
        .iter()
        .flat_map(|obligation| match &obligation.state {
            conduit_body::PurposeObligationState::Satisfied { evidence_sign_ids }
            | conduit_body::PurposeObligationState::Uncertain { evidence_sign_ids }
            | conduit_body::PurposeObligationState::Disputed { evidence_sign_ids } => {
                evidence_sign_ids.clone()
            }
            conduit_body::PurposeObligationState::RepairRequired { failure_sign_id } => {
                vec![failure_sign_id.clone()]
            }
            _ => vec![],
        })
        .collect::<Vec<_>>();
    signs.sort();
    signs.dedup();
    signs
}

fn subject(identity: &str, role: PresentationRole, label: &str) -> PresentationSubject {
    PresentationSubject {
        identity: identity.into(),
        role,
        label: label.into(),
        accessibility_name: label.into(),
    }
}

fn identity(subject: &str, name: &str, value: &str) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Identity(value.into()),
    }
}

fn count(subject: &str, name: &str, value: u64) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Count(value),
    }
}
