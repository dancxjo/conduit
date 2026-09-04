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
    CurrentBodyFrame, FormCandidate, PatchbayBodyApplicationEntrance, PatchbayBodyAttachment,
    PatchbayNavigationProjection, ReadableBodyHistory, RendererAdapterIdentity,
    RendererAdapterKind, RendererExecution,
};

use crate::{
    BrowserBodyWorkbench, BrowserBodyWorkbenchEntrance, BrowserReviewedForm, RendererSnapshot,
    SnapshotError,
};

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
    body_workbench_snapshot_with_reviewed(evidence_revision, encoded_evidence, entrance, &[])
}

pub fn body_workbench_snapshot_with_forms(
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
    forms: &[FormCandidate],
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let reviewed = crate::body_workbench_inventory::from_candidates(forms)
        .map_err(BodyWorkbenchError::Projection)?;
    body_workbench_snapshot_with_reviewed(evidence_revision, encoded_evidence, entrance, &reviewed)
}

pub(crate) fn body_workbench_snapshot_with_reviewed(
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
    reviewed_forms: &[BrowserReviewedForm],
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let attachment =
        PatchbayBodyAttachment::open_serialized(encoded_evidence, model_entrance(&entrance))
            .map_err(BodyWorkbenchError::Entrance)?;
    let presentation = workbench_presentation(evidence_revision, &attachment, reviewed_forms)?;
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
    attach_body_workbench_with_reviewed(
        snapshot,
        evidence_revision,
        encoded_evidence,
        entrance,
        reviewed_forms,
    )
}

impl core::fmt::Display for BodyWorkbenchError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "cannot attach Patchbay Body workbench: {self:?}")
    }
}

impl std::error::Error for BodyWorkbenchError {}

pub fn attach_body_workbench(
    snapshot: RendererSnapshot,
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    attach_body_workbench_with_reviewed(
        snapshot,
        evidence_revision,
        encoded_evidence,
        entrance,
        &[],
    )
}

fn attach_body_workbench_with_reviewed(
    mut snapshot: RendererSnapshot,
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
    reviewed_forms: &[BrowserReviewedForm],
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
        reviewed_forms: reviewed_forms.to_vec(),
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
    crate::body_workbench_inventory::validate(&workbench.reviewed_forms)
        .map_err(BodyWorkbenchError::Projection)?;
    let inventory = crate::body_workbench_inventory::project(
        &workbench.reviewed_forms,
        &attachment.evidence().body.workset,
        &attachment.evidence().body.state,
        attachment.evidence().body.workload_revision,
    )
    .map_err(BodyWorkbenchError::Projection)?;
    let presented_add_actions = presentation
        .actions
        .iter()
        .filter(|action| action.intent == "conduit.intent/add-form@1")
        .cloned()
        .collect::<Vec<_>>();
    let inventory_matches = presented_add_actions == inventory.actions
        && inventory
            .subjects
            .iter()
            .all(|subject| presentation.subjects.contains(subject));
    let body_id = attachment.evidence().body_id.as_str();
    let body_matches = presentation
        .basis
        .body_id
        .as_ref()
        .is_some_and(|identity| identity.as_str() == body_id);
    let basis_form = presentation
        .basis
        .source_document_id
        .as_ref()
        .zip(presentation.basis.checked_form_id.as_ref());
    let form_matches = basis_form.is_none_or(|(source, checked)| {
        attachment
            .evidence()
            .body
            .workset
            .contains(&conduit_body::ResidentForm::new(
                source.clone(),
                checked.clone(),
            ))
    });
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
        || !form_matches
        || !inventory_matches
        || workbench.current != expected_current
        || workbench.history != expected_history
    {
        return Err(BodyWorkbenchError::IdentityMismatch);
    }
    Ok(())
}

pub(crate) fn model_entrance(
    entrance: &BrowserBodyWorkbenchEntrance,
) -> PatchbayBodyApplicationEntrance {
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
    reviewed_forms: &[BrowserReviewedForm],
) -> Result<Presentation, BodyWorkbenchError> {
    let evidence = attachment.evidence();
    let body_identity = format!("body/{}", evidence.body_id.as_str());
    let workset = evidence
        .body
        .effective_workset()
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let mut subjects = vec![PresentationSubject {
        identity: body_identity.clone(),
        role: PresentationRole::Body,
        label: evidence.friendly_name.clone(),
        accessibility_name: format!("Body {}", evidence.friendly_name),
    }];
    let mut relationships = Vec::new();
    let mut properties = vec![
        identity_property(&body_identity, "body-id", evidence.body_id.as_str()),
        PresentationProperty {
            subject: body_identity.clone(),
            name: "workload-revision".into(),
            value: PresentationPropertyValue::Count(evidence.body.workload_revision),
        },
    ];
    let mut form_identities = Vec::with_capacity(workset.len());
    let (action_name, action_label, action_intent) = match evidence.body.state {
        conduit_body::BodyState::Lulled => ("wake", "Wake", "conduit.intent/wake@1"),
        conduit_body::BodyState::Awake { .. } => ("lull", "Lull", "conduit.intent/lull@1"),
    };
    let mut actions = vec![PresentationAction {
        identity: format!("action/{action_name}/{body_identity}"),
        intent: action_intent.into(),
        target: body_identity.clone(),
        label: action_label.into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability: PresentationActionAvailability::Available,
    }];
    for form in workset.forms() {
        let form_identity = format!("form/{}", form.checked_form_id.as_str());
        let label = form.checked_form_id.as_str().to_owned();
        subjects.push(PresentationSubject {
            identity: form_identity.clone(),
            role: PresentationRole::Form,
            label: label.clone(),
            accessibility_name: format!("Form {label}"),
        });
        relationships.push(PresentationRelationship {
            source: body_identity.clone(),
            target: form_identity.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        properties.push(identity_property(
            &form_identity,
            "source-document-id",
            form.source_document_id.as_str(),
        ));
        properties.push(identity_property(
            &form_identity,
            "checked-form-id",
            form.checked_form_id.as_str(),
        ));
        actions.push(PresentationAction {
            identity: format!(
                "action/remove-form/{}/{}",
                form.checked_form_id.as_str(),
                evidence.body.workload_revision
            ),
            intent: "conduit.intent/remove-form@1".into(),
            target: form_identity.clone(),
            label: "Remove from Body".into(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: match &evidence.body.state {
                conduit_body::BodyState::Lulled => PresentationActionAvailability::Available,
                conduit_body::BodyState::Awake { .. } => {
                    PresentationActionAvailability::Unavailable {
                        reason_code: "body-awake".into(),
                        explanation: "Lull the Body before changing its active Form workload."
                            .into(),
                    }
                }
            },
        });
        form_identities.push(form_identity);
    }
    let reviewed = crate::body_workbench_inventory::project(
        reviewed_forms,
        &workset,
        &evidence.body.state,
        evidence.body.workload_revision,
    )
    .map_err(BodyWorkbenchError::Projection)?;
    subjects.extend(reviewed.subjects);
    properties.extend(reviewed.properties);
    actions.extend(reviewed.actions);
    form_identities.extend(reviewed.disclosures.iter().map(|item| item.subject.clone()));
    let mut text = vec![PresentationText {
        subject: body_identity.clone(),
        text: format!(
            "{} is a durable Body running {} current Form(s) at workload revision {}.",
            evidence.friendly_name,
            workset.len(),
            evidence.body.workload_revision,
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
    Presentation::new_with_semantics(
        revision,
        PresentationBasis {
            body_id: Some(evidence.body_id.clone()),
            wake_id: match &evidence.body.state {
                conduit_body::BodyState::Awake { wake_id } => Some(wake_id.clone()),
                conduit_body::BodyState::Lulled => None,
            },
            source_document_id: None,
            checked_form_id: None,
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
        actions,
        vec![PresentationDisclosure {
            subject: body_identity,
            level: PresentationDisclosureLevel::Primary,
        }]
        .into_iter()
        .chain(
            form_identities
                .into_iter()
                .map(|subject| PresentationDisclosure {
                    subject,
                    level: PresentationDisclosureLevel::Context,
                }),
        )
        .collect(),
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
mod tests;
