//! Portable WORLD projection for the bounded zero-Body entrance state.

use conduit_presentation::{
    Presentation, PresentationAction, PresentationActionAvailability, PresentationBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};

use crate::portable_projection::ContentBuilder;
use crate::{OpenedFrontDoorSubject, ZeroBodyFrontDoor, ZeroBodyFrontDoorProjection};

impl ZeroBodyFrontDoor {
    pub fn project(&self) -> Result<ZeroBodyFrontDoorProjection, String> {
        let host = self.model.advertisement();
        let host_subject = format!("host/{}/{}", host.host_id.as_str(), host.boot_id.as_str());
        let mut subjects = vec![PresentationSubject {
            identity: host_subject.clone(),
            role: PresentationRole::Host,
            label: "This computer".into(),
            accessibility_name: format!(
                "This Host {} boot {}; current Body none",
                host.host_id.as_str(),
                host.boot_id.as_str()
            ),
        }];
        let mut relationships = Vec::new();
        let mut properties = vec![
            identity(&host_subject, "host-id", host.host_id.as_str()),
            identity(&host_subject, "boot-id", host.boot_id.as_str()),
            PresentationProperty {
                subject: host_subject.clone(),
                name: "this-host".into(),
                value: PresentationPropertyValue::Flag(true),
            },
            PresentationProperty {
                subject: host_subject.clone(),
                name: "current-body".into(),
                value: PresentationPropertyValue::Text("none".into()),
            },
        ];
        let mut text = vec![PresentationText {
            subject: host_subject.clone(),
            text: "HOST body=none; OPEN is inert; only JOIN or BIRTH can embody this Host".into(),
        }];
        let mut actions = Vec::new();
        let mut disclosures = vec![PresentationDisclosure {
            subject: host_subject.clone(),
            level: PresentationDisclosureLevel::Context,
        }];
        let selected_form = match &self.opened {
            Some(OpenedFrontDoorSubject::Form {
                checked_form_id, ..
            }) => Some(checked_form_id),
            _ => None,
        };
        for offer in &host.capabilities {
            let subject = format!(
                "capability/{}/{}/{}",
                host.host_id.as_str(),
                host.boot_id.as_str(),
                offer.capability_id.as_str()
            );
            subjects.push(PresentationSubject {
                identity: subject.clone(),
                role: PresentationRole::Capability,
                label: offer.kind_id.as_str().into(),
                accessibility_name: format!("Available capability {}", offer.kind_id.as_str()),
            });
            relationships.push(PresentationRelationship {
                source: host_subject.clone(),
                target: subject.clone(),
                kind: PresentationRelationshipKind::Contains,
            });
            properties.push(identity(
                &subject,
                "capability-id",
                offer.capability_id.as_str(),
            ));
        }
        for candidate in &self.body_candidates {
            let subject = format!("body/{}", candidate.body.body_id.as_str());
            subjects.push(PresentationSubject {
                identity: subject.clone(),
                role: PresentationRole::Body,
                label: candidate.label.clone(),
                accessibility_name: format!("Discoverable Body {}; not joined", candidate.label),
            });
            relationships.push(PresentationRelationship {
                source: host_subject.clone(),
                target: subject.clone(),
                kind: PresentationRelationshipKind::Observes,
            });
            properties.extend([
                identity(&subject, "body-id", candidate.body.body_id.as_str()),
                identity(&subject, "membership-proof", candidate.proof_id.as_str()),
                PresentationProperty {
                    subject: subject.clone(),
                    name: "current".into(),
                    value: PresentationPropertyValue::Flag(false),
                },
                PresentationProperty {
                    subject: subject.clone(),
                    name: "freshness-sequence".into(),
                    value: PresentationPropertyValue::Count(candidate.freshness_sequence),
                },
            ]);
            text.push(PresentationText {
                subject,
                text: "BODY candidate; OPEN permits inspection and does not grant membership"
                    .into(),
            });
        }
        for form in &self.forms {
            let subject = format!("form/{}", form.checked_form_id.as_str());
            let is_opened = matches!(
                &self.opened,
                Some(OpenedFrontDoorSubject::Form { checked_form_id, .. }) if checked_form_id == &form.checked_form_id
            );
            subjects.push(PresentationSubject {
                identity: subject.clone(),
                role: PresentationRole::Form,
                label: form.label.clone(),
                accessibility_name: format!("Openable Form {}; not born", form.label),
            });
            relationships.push(PresentationRelationship {
                source: host_subject.clone(),
                target: subject.clone(),
                kind: PresentationRelationshipKind::Observes,
            });
            properties.extend([
                identity(&subject, "form-id", form.checked_form_id.as_str()),
                identity(
                    &subject,
                    "source-document-id",
                    form.source_document_id.as_str(),
                ),
                identity(&subject, "checked-form-id", form.checked_form_id.as_str()),
                PresentationProperty {
                    subject: subject.clone(),
                    name: "freshness-sequence".into(),
                    value: PresentationPropertyValue::Count(form.freshness_sequence),
                },
            ]);
            text.push(PresentationText {
                subject: subject.clone(),
                text: format!(
                    "FORM provenance={}; OPEN permits inspection and does not create a Body",
                    form.provenance
                ),
            });
            actions.extend([
                PresentationAction {
                    identity: format!("action/open/{}", form.checked_form_id.as_str()),
                    intent: "conduit.intent/open@1".into(),
                    target: subject.clone(),
                    label: "Open".into(),
                    disclosure: PresentationDisclosureLevel::CurrentAction,
                    availability: PresentationActionAvailability::Available,
                },
                PresentationAction {
                    identity: format!("action/birth/{}", form.checked_form_id.as_str()),
                    intent: "conduit.intent/birth@1".into(),
                    target: subject.clone(),
                    label: "Birth".into(),
                    disclosure: PresentationDisclosureLevel::CurrentAction,
                    availability: if is_opened {
                        PresentationActionAvailability::Available
                    } else {
                        PresentationActionAvailability::Unavailable {
                            reason_code: "authority/not-admitted".into(),
                            explanation:
                                "No admitted authority can create a Body from this entrance."
                                    .into(),
                        }
                    },
                },
            ]);
            disclosures.push(PresentationDisclosure {
                subject,
                level: if selected_form.is_none() || is_opened {
                    PresentationDisclosureLevel::Primary
                } else {
                    PresentationDisclosureLevel::ExactProvenance
                },
            });
        }
        if let Some(opened) = &self.opened {
            let subject = match opened {
                OpenedFrontDoorSubject::Body { body_id, .. } => {
                    format!("body/{}", body_id.as_str())
                }
                OpenedFrontDoorSubject::Form {
                    checked_form_id, ..
                } => {
                    format!("form/{}", checked_form_id.as_str())
                }
            };
            properties.push(PresentationProperty {
                subject: subject.clone(),
                name: "opened".into(),
                value: PresentationPropertyValue::Flag(true),
            });
            if let OpenedFrontDoorSubject::Form {
                checked_form_id, ..
            } = opened
            {
                let form = self
                    .forms
                    .iter()
                    .find(|candidate| &candidate.checked_form_id == checked_form_id)
                    .ok_or_else(|| "opened Form is absent from the bounded entrance".to_owned())?;
                let editor = form.editor()?;
                let open_form = editor.view().open_form.clone();
                let graph = editor
                    .patchbay_graph_for_authoring(&open_form)
                    .map_err(|error| error.to_string())?;
                let form_subject = format!("form/{}", form.checked_form_id.as_str());
                let mut content =
                    ContentBuilder::from_parts(subjects, relationships, properties, text);
                content.subject_with_identity(
                    &form_subject,
                    PresentationRole::Form,
                    &open_form,
                    format!("Checked Form {} opened from Form {}", open_form, form.label),
                );
                content.contains(&subject, &form_subject);
                content.property(
                    &form_subject,
                    "source-document-id",
                    PresentationPropertyValue::Identity(form.source_document_id.as_str().into()),
                );
                content.property(
                    &form_subject,
                    "checked-form-id",
                    PresentationPropertyValue::Identity(form.checked_form_id.as_str().into()),
                );
                content.line(
                    &form_subject,
                    format!(
                        "Checked Form {}; OPEN is inert and no Body, Plan, or Play exists",
                        open_form
                    ),
                );
                crate::portable_graph_projection::append_exact_graph(
                    &form_subject,
                    &graph,
                    None,
                    None,
                    &mut content,
                );
                actions.push(PresentationAction {
                    identity: format!("action/save/{form_subject}"),
                    intent: "conduit.intent/save@1".into(),
                    target: form_subject,
                    label: "Save".into(),
                    disclosure: PresentationDisclosureLevel::CurrentAction,
                    availability: PresentationActionAvailability::Available,
                });
                subjects = content.subjects;
                relationships = content.relationships;
                properties = content.properties;
                text = content.text;
            }
        }
        for refusal in &self.refusals {
            let subject = format!("sign/{}", refusal.sign_id.as_str());
            subjects.push(PresentationSubject {
                identity: subject.clone(),
                role: PresentationRole::Sign,
                label: format!("Refused {}", refusal.code),
                accessibility_name: format!("Front-door refusal Sign {}", refusal.code),
            });
            relationships.push(PresentationRelationship {
                source: host_subject.clone(),
                target: subject.clone(),
                kind: PresentationRelationshipKind::Observes,
            });
            properties.push(identity(&subject, "sign-id", refusal.sign_id.as_str()));
            properties.push(PresentationProperty {
                subject: subject.clone(),
                name: "refusal-code".into(),
                value: PresentationPropertyValue::Text(refusal.code.clone()),
            });
            text.push(PresentationText {
                subject,
                text: format!("REFUSED {}; no Body transition occurred", refusal.code),
            });
        }
        let mut sign_ids = self
            .body_candidates
            .iter()
            .map(|candidate| candidate.evidence_sign.clone())
            .chain(self.forms.iter().map(|form| form.evidence_sign.clone()))
            .chain(self.refusals.iter().map(|refusal| refusal.sign_id.clone()))
            .collect::<Vec<_>>();
        sign_ids.sort();
        sign_ids.dedup();
        let presentation = Presentation::new_with_semantics(
            self.revision,
            PresentationBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_form_id: None,
                expanded_form_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids,
            },
            subjects,
            relationships,
            properties,
            text,
            actions,
            disclosures,
        )
        .map_err(|error| error.to_string())?;
        let selected_form = selected_form.is_some();
        let navigation =
            crate::PatchbayNavigationProjection::for_zero_body(&presentation, selected_form)?;
        Ok(ZeroBodyFrontDoorProjection {
            presentation,
            navigation,
        })
    }
}

fn identity(subject: &str, name: &str, value: &str) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Identity(value.into()),
    }
}
