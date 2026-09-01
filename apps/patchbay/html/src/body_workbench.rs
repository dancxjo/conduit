//! Browser composition of the accepted Body attachment, current frame, and
//! readable `BODY / SIGNS` history. This module adds no Body truth: it retains
//! the validated evidence bytes and serializes the shared model projections.

use conduit_core::{BootId, HostId, ImplementationId, PlanId, SignId};
use conduit_presentation::{
    Presentation, PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};
use patchbay_model::{
    CurrentBodyFrame, PatchbayBodyApplicationEntrance, PatchbayBodyAttachment,
    PatchbayNavigationProjection, ReadableBodyHistory, RendererAdapterIdentity,
    RendererAdapterKind, RendererExecution,
};

use crate::{BrowserBodyWorkbench, BrowserBodyWorkbenchEntrance, RendererSnapshot, SnapshotError};

pub const BODY_WORKBENCH_SCHEMA: &str = "conduit.patchbay/browser-body-workbench@1";

#[derive(Debug)]
pub enum BodyWorkbenchError {
    Entrance(patchbay_model::PatchbayBodyEntranceError),
    History(patchbay_model::ReadableBodyHistoryError),
    Encode(serde_json::Error),
    IdentityMismatch,
    Snapshot(SnapshotError),
    Projection(String),
}

pub fn body_workbench_snapshot(
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let attachment =
        PatchbayBodyAttachment::open_serialized(encoded_evidence, model_entrance(&entrance))
            .map_err(BodyWorkbenchError::Entrance)?;
    let presentation = workbench_presentation(evidence_revision, &attachment)?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/body-workbench"),
            boot_id: BootId::from("patchbay-html/body-workbench/boot"),
            target_subject: "patchbay-html/body-workbench/document".into(),
        },
        SignId::from("patchbay-html/body-workbench/prepared"),
    )
    .map_err(|error| BodyWorkbenchError::Projection(error.to_string()))?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(BodyWorkbenchError::Snapshot)?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)
        .map_err(BodyWorkbenchError::Projection)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(BodyWorkbenchError::Snapshot)?;
    attach_body_workbench(snapshot, evidence_revision, encoded_evidence, entrance)
}

/// Deterministic browser-only documentary fixture. Product entrances use
/// `body_workbench_snapshot` with caller-supplied serialized evidence.
pub fn body_workbench_fixture_snapshot(
    hosted: bool,
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    use conduit_body::{
        AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationChoice,
        BodyGraduationEvidence, BodyMembership, MembershipProofId, PartId,
    };
    use conduit_core::{bind_sign, CheckedFormId, OfferGeneration, SourceDocumentId};

    const PLAN: &str = "plan/roseau-hosted-patchbay";
    const IMPLEMENTATION: &str = "browser/patchbay-surface@1";
    let host = HostId::from("host/roseau");
    let boot = BootId::from("boot/roseau/1");
    let body = Body::born(
        SourceDocumentId::from("source/roseau-program"),
        CheckedFormId::from("checked/roseau-program"),
        1,
        bind_sign(&host, &boot, None, 1).sign_id,
    )
    .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let mut membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let part = PartId::bind(&body.body_id, "roseau/here", 1)
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let proof = MembershipProofId::bind("proof/roseau/here")
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            bind_sign(&host, &boot, None, 2).sign_id,
        )
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let joined = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            bind_sign(&host, &boot, None, 3).sign_id,
        )
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone())
            .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?,
        "Roseau".into(),
        "Morse relay".into(),
    )
    .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    evidence
        .append_membership_events(membership, &[(admitted, 2), (joined, 3)])
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let choice = if hosted {
        BodyGraduationChoice::HostedPatchbay
    } else {
        BodyGraduationChoice::ExternalReader
    };
    evidence
        .graduate(BodyGraduationEvidence {
            body_id: evidence.body_id.clone(),
            sequence: 4,
            sign_id: SignId::from("sign/roseau/graduated"),
            choice,
            patchbay_plan_id: hosted.then(|| PlanId::from(PLAN)),
            patchbay_implementation_id: hosted.then(|| ImplementationId::from(IMPLEMENTATION)),
        })
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let encoded = serde_json::to_vec(&evidence).map_err(BodyWorkbenchError::Encode)?;
    let entrance = if hosted {
        BrowserBodyWorkbenchEntrance::Hosted {
            plan_id: PLAN.into(),
            implementation_id: IMPLEMENTATION.into(),
        }
    } else {
        BrowserBodyWorkbenchEntrance::ExternalReader
    };
    body_workbench_snapshot(1, &encoded, entrance)
}

impl core::fmt::Display for BodyWorkbenchError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "cannot attach Patchbay Body workbench: {self:?}")
    }
}

impl std::error::Error for BodyWorkbenchError {}

pub fn attach_body_workbench(
    mut snapshot: RendererSnapshot,
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let model_entrance = model_entrance(&entrance);
    let attachment = PatchbayBodyAttachment::open_serialized(encoded_evidence, model_entrance)
        .map_err(BodyWorkbenchError::Entrance)?;
    let current = CurrentBodyFrame::from_attachment(evidence_revision, &attachment);
    let history = ReadableBodyHistory::from_attachment(evidence_revision, &attachment)
        .map_err(BodyWorkbenchError::History)?;
    let workbench = BrowserBodyWorkbench {
        schema: BODY_WORKBENCH_SCHEMA.into(),
        evidence_revision,
        encoded_evidence: encoded_evidence.to_vec(),
        entrance,
        body_id: attachment.evidence().body_id.as_str().into(),
        current: serde_json::to_value(current).map_err(BodyWorkbenchError::Encode)?,
        history: serde_json::to_value(history).map_err(BodyWorkbenchError::Encode)?,
    };
    validate_body_workbench(&workbench, &snapshot.presentation)?;
    snapshot
        .attach_body_workbench(workbench)
        .map_err(BodyWorkbenchError::Snapshot)?;
    Ok(snapshot)
}

pub(crate) fn validate_body_workbench(
    workbench: &BrowserBodyWorkbench,
    presentation: &Presentation,
) -> Result<(), BodyWorkbenchError> {
    if workbench.schema != BODY_WORKBENCH_SCHEMA || workbench.evidence_revision == 0 {
        return Err(BodyWorkbenchError::IdentityMismatch);
    }
    let attachment = PatchbayBodyAttachment::open_serialized(
        &workbench.encoded_evidence,
        model_entrance(&workbench.entrance),
    )
    .map_err(BodyWorkbenchError::Entrance)?;
    let body_id = attachment.evidence().body_id.as_str();
    let body_matches = presentation
        .basis
        .body_id
        .as_ref()
        .is_some_and(|identity| identity.as_str() == body_id);
    let source_matches = presentation.basis.source_document_id.as_ref()
        == Some(&attachment.evidence().body.source_document_id);
    let checked_matches = presentation.basis.checked_form_id.as_ref()
        == Some(&attachment.evidence().body.checked_form_id);
    let expected_current = serde_json::to_value(CurrentBodyFrame::from_attachment(
        workbench.evidence_revision,
        &attachment,
    ))
    .map_err(BodyWorkbenchError::Encode)?;
    let expected_history = serde_json::to_value(
        ReadableBodyHistory::from_attachment(workbench.evidence_revision, &attachment)
            .map_err(BodyWorkbenchError::History)?,
    )
    .map_err(BodyWorkbenchError::Encode)?;
    if workbench.body_id != body_id
        || !body_matches
        || !source_matches
        || !checked_matches
        || workbench.current != expected_current
        || workbench.history != expected_history
    {
        return Err(BodyWorkbenchError::IdentityMismatch);
    }
    Ok(())
}

fn model_entrance(entrance: &BrowserBodyWorkbenchEntrance) -> PatchbayBodyApplicationEntrance {
    match entrance {
        BrowserBodyWorkbenchEntrance::Hosted {
            plan_id,
            implementation_id,
        } => PatchbayBodyApplicationEntrance::Hosted {
            plan_id: PlanId::from(plan_id.as_str()),
            implementation_id: ImplementationId::from(implementation_id.as_str()),
        },
        BrowserBodyWorkbenchEntrance::ExternalReader => {
            PatchbayBodyApplicationEntrance::ExternalReader
        }
    }
}

fn workbench_presentation(
    revision: u64,
    attachment: &PatchbayBodyAttachment,
) -> Result<Presentation, BodyWorkbenchError> {
    let evidence = attachment.evidence();
    let body_identity = format!("body/{}", evidence.body_id.as_str());
    let program_identity = format!("form/{}", evidence.body.checked_form_id.as_str());
    let mut subjects = vec![
        PresentationSubject {
            identity: program_identity.clone(),
            role: PresentationRole::Form,
            label: evidence.initial_program.clone(),
            accessibility_name: format!("Program {}", evidence.initial_program),
        },
        PresentationSubject {
            identity: body_identity.clone(),
            role: PresentationRole::Body,
            label: evidence.friendly_name.clone(),
            accessibility_name: format!("Body {}", evidence.friendly_name),
        },
    ];
    let mut relationships = vec![PresentationRelationship {
        source: body_identity.clone(),
        target: program_identity.clone(),
        kind: PresentationRelationshipKind::Realizes,
    }];
    let mut properties = vec![
        identity_property(&body_identity, "body-id", evidence.body_id.as_str()),
        identity_property(
            &program_identity,
            "source-document-id",
            evidence.body.source_document_id.as_str(),
        ),
        identity_property(
            &program_identity,
            "checked-form-id",
            evidence.body.checked_form_id.as_str(),
        ),
    ];
    let mut text = vec![PresentationText {
        subject: body_identity.clone(),
        text: format!(
            "{} is a durable Body whose initial Program is {}.",
            evidence.friendly_name, evidence.initial_program
        ),
    }];
    for part in &evidence.membership.parts {
        let part_identity = format!("part/{}", part.part_id.as_str());
        subjects.push(PresentationSubject {
            identity: part_identity.clone(),
            role: PresentationRole::Part,
            label: part.part_id.as_str().into(),
            accessibility_name: format!("Body Part {}", part.part_id.as_str()),
        });
        relationships.push(PresentationRelationship {
            source: body_identity.clone(),
            target: part_identity.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        if let Some(current) = &part.current {
            let host_identity = format!("host/{}", current.host_id.as_str());
            subjects.push(PresentationSubject {
                identity: host_identity.clone(),
                role: PresentationRole::Host,
                label: current.host_id.as_str().into(),
                accessibility_name: format!("Current Host {}", current.host_id.as_str()),
            });
            relationships.push(PresentationRelationship {
                source: part_identity.clone(),
                target: host_identity.clone(),
                kind: PresentationRelationshipKind::Realizes,
            });
            properties.push(identity_property(
                &host_identity,
                "boot-id",
                current.boot_id.as_str(),
            ));
        }
    }
    for record in &evidence.records {
        let sign_identity = format!("sign/{}", record.sign_id.as_str());
        subjects.push(PresentationSubject {
            identity: sign_identity.clone(),
            role: PresentationRole::Sign,
            label: format!("Evidence {}", record.sequence),
            accessibility_name: format!("Body biography evidence sequence {}", record.sequence),
        });
        relationships.push(PresentationRelationship {
            source: body_identity.clone(),
            target: sign_identity.clone(),
            kind: PresentationRelationshipKind::Observes,
        });
        properties.push(PresentationProperty {
            subject: sign_identity.clone(),
            name: "evidence-sequence".into(),
            value: PresentationPropertyValue::Count(record.sequence),
        });
        text.push(PresentationText {
            subject: sign_identity,
            text: format!("{:?}", record.kind),
        });
    }
    let (action_name, action_label, action_intent) = match evidence.body.state {
        conduit_body::BodyState::Lulled => ("wake", "Wake", "conduit.intent/wake@1"),
        conduit_body::BodyState::Awake { .. } => ("lull", "Lull", "conduit.intent/lull@1"),
    };
    Presentation::new_with_semantics(
        revision,
        PresentationBasis {
            seed_id: Some(evidence.body.seed_id.clone()),
            body_id: Some(evidence.body_id.clone()),
            wake_id: match &evidence.body.state {
                conduit_body::BodyState::Awake { wake_id } => Some(wake_id.clone()),
                conduit_body::BodyState::Lulled => None,
            },
            source_document_id: Some(evidence.body.source_document_id.clone()),
            checked_form_id: Some(evidence.body.checked_form_id.clone()),
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: evidence
                .records
                .iter()
                .map(|record| record.sign_id.clone())
                .collect(),
        },
        subjects,
        relationships,
        properties,
        text,
        vec![PresentationAction {
            identity: format!("action/{action_name}/{body_identity}"),
            intent: action_intent.into(),
            target: body_identity.clone(),
            label: action_label.into(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        }],
        vec![
            PresentationDisclosure {
                subject: body_identity,
                level: PresentationDisclosureLevel::Primary,
            },
            PresentationDisclosure {
                subject: program_identity,
                level: PresentationDisclosureLevel::Context,
            },
        ],
    )
    .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))
}

fn identity_property(subject: &str, name: &str, identity: &str) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Identity(identity.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        Body, BodyBiographyEvidence, BodyGraduationChoice, BodyGraduationEvidence, BodyMembership,
    };
    use conduit_core::SignId;

    fn evidence(snapshot: &RendererSnapshot) -> Vec<u8> {
        let basis = &snapshot.presentation.basis;
        let body = Body::born(
            basis.source_document_id.clone().unwrap(),
            basis.checked_form_id.clone().unwrap(),
            1,
            SignId::from("patchbay/bornd"),
        )
        .unwrap()
        .wake(1, SignId::from("patchbay/woke"))
        .unwrap()
        .0;
        assert_eq!(basis.body_id.as_ref(), Some(&body.body_id));
        let mut evidence = BodyBiographyEvidence::born(
            body.clone(),
            BodyMembership::new(body.body_id.clone()).unwrap(),
            "Roseau".into(),
            "hello@1".into(),
        )
        .unwrap();
        evidence
            .graduate(BodyGraduationEvidence {
                body_id: body.body_id,
                sequence: 2,
                sign_id: SignId::from("sign/roseau/graduated"),
                choice: BodyGraduationChoice::ExternalReader,
                patchbay_plan_id: None,
                patchbay_implementation_id: None,
            })
            .unwrap();
        serde_json::to_vec(&evidence).unwrap()
    }

    #[test]
    fn attached_workbench_retains_exact_evidence_and_refuses_identity_drift() {
        let snapshot = crate::demonstration_snapshot().unwrap();
        let bytes = evidence(&snapshot);
        let attached = attach_body_workbench(
            snapshot.clone(),
            1,
            &bytes,
            BrowserBodyWorkbenchEntrance::ExternalReader,
        )
        .unwrap();
        let workbench = attached.body_workbench.as_ref().unwrap();
        assert_eq!(workbench.encoded_evidence, bytes);
        assert_eq!(workbench.current["friendly_name"], "Roseau");
        assert_eq!(workbench.history["entries"].as_array().unwrap().len(), 2);

        let mut stale = workbench.clone();
        stale.body_id.push_str("-stale");
        assert!(validate_body_workbench(&stale, &snapshot.presentation).is_err());

        let entrance =
            body_workbench_snapshot(1, &bytes, BrowserBodyWorkbenchEntrance::ExternalReader)
                .unwrap();
        assert_eq!(
            entrance.presentation.basis.body_id,
            snapshot.presentation.basis.body_id
        );
        let navigation = entrance.navigation.unwrap();
        assert!(navigation
            .navigation
            .places
            .iter()
            .any(|place| place.place == conduit_presentation::PresentationPlace::Program));
        assert!(navigation
            .navigation
            .places
            .iter()
            .any(|place| place.place == conduit_presentation::PresentationPlace::Body));
    }
}
