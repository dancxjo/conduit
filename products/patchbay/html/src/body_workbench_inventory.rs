//! Bounded reviewed-Form inventory projected beside one attached Body.

use crate::BrowserReviewedForm;
use conduit_body::{BodyState, BodyWorkset, ResidentForm, MAX_BODY_FORMS};
use conduit_core::{CheckedFormId, SourceDocumentId};
use conduit_presentation::{
    PresentationAction, PresentationActionAvailability, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationProperty, PresentationPropertyValue, PresentationRole,
    PresentationSubject,
};
use patchbay_model::{FormCandidate, MAX_FRONT_DOOR_FORMS};
use std::collections::BTreeSet;

pub(crate) struct ReviewedInventoryProjection {
    pub subjects: Vec<PresentationSubject>,
    pub properties: Vec<PresentationProperty>,
    pub actions: Vec<PresentationAction>,
    pub disclosures: Vec<PresentationDisclosure>,
}

pub(crate) fn from_candidates(
    candidates: &[FormCandidate],
) -> Result<Vec<BrowserReviewedForm>, String> {
    if candidates.len() > MAX_FRONT_DOOR_FORMS {
        return Err("reviewed Form inventory exceeds its finite bound".into());
    }
    let reviewed = candidates
        .iter()
        .map(|candidate| BrowserReviewedForm {
            label: candidate.label.clone(),
            source_document_id: candidate.source_document_id.as_str().into(),
            checked_form_id: candidate.checked_form_id.as_str().into(),
        })
        .collect::<Vec<_>>();
    validate(&reviewed)?;
    Ok(reviewed)
}

pub(crate) fn validate(reviewed: &[BrowserReviewedForm]) -> Result<(), String> {
    if reviewed.len() > MAX_FRONT_DOOR_FORMS {
        return Err("reviewed Form inventory exceeds its finite bound".into());
    }
    let mut identities = BTreeSet::new();
    for form in reviewed {
        if form.label.trim().is_empty()
            || form.label.len() > crate::MAX_FORM_LABEL_BYTES
            || form.source_document_id.is_empty()
            || form.checked_form_id.is_empty()
            || !identities.insert(form.checked_form_id.as_str())
        {
            return Err("reviewed Form inventory identity is invalid or duplicated".into());
        }
    }
    Ok(())
}

pub(crate) fn project(
    reviewed: &[BrowserReviewedForm],
    workset: &BodyWorkset,
    state: &BodyState,
    workload_revision: u64,
) -> Result<ReviewedInventoryProjection, String> {
    validate(reviewed)?;
    let mut projection = ReviewedInventoryProjection {
        subjects: Vec::new(),
        properties: Vec::new(),
        actions: Vec::new(),
        disclosures: Vec::new(),
    };
    for form in reviewed {
        let resident = ResidentForm::new(
            SourceDocumentId::from(form.source_document_id.as_str()),
            CheckedFormId::from(form.checked_form_id.as_str()),
        );
        if let Some(active) = workset
            .forms()
            .iter()
            .find(|active| active.checked_form_id == resident.checked_form_id)
        {
            if active != &resident {
                return Err("reviewed Form collides with an active checked identity".into());
            }
            continue;
        }
        let identity = format!("form/{}", form.checked_form_id);
        projection.subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Form,
            label: form.label.clone(),
            accessibility_name: format!("Available Form {}", form.label),
        });
        projection.properties.extend([
            identity_property(&identity, "source-document-id", &form.source_document_id),
            identity_property(&identity, "checked-form-id", &form.checked_form_id),
            PresentationProperty {
                subject: identity.clone(),
                name: "workload-membership".into(),
                value: PresentationPropertyValue::Text("available".into()),
            },
        ]);
        projection.actions.push(PresentationAction {
            identity: format!(
                "action/add-form/{}/{}",
                form.checked_form_id, workload_revision
            ),
            intent: "conduit.intent/add-form@1".into(),
            target: identity.clone(),
            label: "Add to Body".into(),
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: match state {
                BodyState::Awake { .. } => PresentationActionAvailability::Unavailable {
                    reason_code: "body-awake".into(),
                    explanation: "Lull the Body before changing its active Form workload.".into(),
                },
                BodyState::Lulled if workset.len() >= MAX_BODY_FORMS => {
                    PresentationActionAvailability::Unavailable {
                        reason_code: "workload-capacity".into(),
                        explanation: "The Body active Form workload is at capacity.".into(),
                    }
                }
                BodyState::Lulled => PresentationActionAvailability::Available,
            },
        });
        projection.disclosures.push(PresentationDisclosure {
            subject: identity,
            level: PresentationDisclosureLevel::Context,
        });
    }
    Ok(projection)
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

    fn reviewed(name: &str) -> BrowserReviewedForm {
        BrowserReviewedForm {
            label: name.into(),
            source_document_id: format!("source/{name}"),
            checked_form_id: format!("checked/{name}"),
        }
    }

    #[test]
    fn inventory_is_bounded_unique_and_excludes_exact_active_forms() {
        let hello = reviewed("hello");
        let workset = BodyWorkset::from_forms([ResidentForm::new(
            SourceDocumentId::from(hello.source_document_id.as_str()),
            CheckedFormId::from(hello.checked_form_id.as_str()),
        )])
        .unwrap();
        let projected = project(
            &[hello.clone(), reviewed("clock")],
            &workset,
            &BodyState::Lulled,
            0,
        )
        .unwrap();
        assert_eq!(projected.subjects.len(), 1);
        assert_eq!(projected.subjects[0].label, "clock");
        assert_eq!(projected.actions.len(), 1);
        assert_eq!(projected.actions[0].intent, "conduit.intent/add-form@1");
        assert!(matches!(
            projected.actions[0].availability,
            PresentationActionAvailability::Available
        ));

        assert!(validate(&[hello.clone(), hello]).is_err());
        assert!(validate(&vec![reviewed("form"); MAX_FRONT_DOOR_FORMS + 1]).is_err());
    }
}
