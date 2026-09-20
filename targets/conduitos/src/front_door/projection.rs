//! Immutable semantic projection of the native entrance.
use super::*;

impl FrontDoor {
    pub fn presentation(&self) -> Result<Presentation, Error> {
        if let Some(arrival) = &self.arrival {
            return arrival.presentation(self);
        }
        let host = format!("host/{}/{}", self.host_id.as_str(), self.boot_id.as_str());
        let form = self.form_subject.clone();
        let mut subjects = vec![
            PresentationSubject {
                identity: host.clone(),
                role: PresentationRole::Host,
                label: "This host".into(),
                accessibility_name: "This host; current body none".into(),
            },
            PresentationSubject {
                identity: form.clone(),
                role: PresentationRole::Form,
                label: "ConduitOS entrance Form".into(),
                accessibility_name: "Openable checked IMAGE Form; opening is inert".into(),
            },
        ];
        let mut relationships = vec![PresentationRelationship {
            source: host.clone(),
            target: form.clone(),
            kind: PresentationRelationshipKind::Observes,
        }];
        let mut properties = vec![
            property(
                &host,
                "current-body",
                PresentationPropertyValue::Text("none".into()),
            ),
            property(&host, "host-id", identity(self.host_id.as_str())),
            property(&host, "boot-id", identity(self.boot_id.as_str())),
            property(
                &host,
                "offer-generation",
                PresentationPropertyValue::Count(self.offer_generation.0),
            ),
            property(
                &host,
                "offer-count",
                PresentationPropertyValue::Count(self.offer_count),
            ),
            property(
                &form,
                "source-document-id",
                identity(self.source_document_id.as_str()),
            ),
            property(
                &form,
                "checked-form-id",
                identity(self.checked_form_id.as_str()),
            ),
            property(
                &form,
                "opened",
                PresentationPropertyValue::Flag(self.form_open),
            ),
        ];
        if self.exact_details_open {
            properties.extend([
                property(&host, "profile-id", identity(&self.profile_id)),
                property(&host, "build-id", identity(&self.build_id)),
                property(&host, "image-id", identity(&self.image_id)),
            ]);
        }
        if let Some(journey) = &self.journey {
            if let Some(body_id) = &journey.body_id {
                let body = format!("body/{}", body_id.as_str());
                subjects.push(PresentationSubject {
                    identity: body.clone(),
                    role: PresentationRole::Body,
                    label: journey
                        .friendly_name
                        .clone()
                        .unwrap_or_else(|| "Current body".into()),
                    accessibility_name: format!("Current body; {:?}", journey.status),
                });
                relationships.push(PresentationRelationship {
                    source: host.clone(),
                    target: body.clone(),
                    kind: PresentationRelationshipKind::Contains,
                });
                properties.push(property(&body, "body-id", identity(body_id.as_str())));
                if let Some(born_sign_id) = &journey.born_sign_id {
                    properties.push(property(
                        &body,
                        "born-sign-id",
                        identity(born_sign_id.as_str()),
                    ));
                }
                if let Some(part_id) = &journey.part_id {
                    properties.push(property(&body, "part-id", identity(part_id.as_str())));
                }
            }
            if let Some(expanded_form_id) = &journey.expanded_form_id {
                properties.push(property(
                    &form,
                    "expanded-form-id",
                    identity(expanded_form_id.as_str()),
                ));
            }
            if let Some(wake_id) = &journey.wake_id {
                properties.push(property(&host, "wake-id", identity(wake_id.as_str())));
            }
            if let Some(plan_id) = &journey.plan_id {
                properties.push(property(&host, "plan-id", identity(plan_id.as_str())));
            }
            if let Some(play_id) = &journey.active_play_id {
                properties.push(property(
                    &host,
                    "active-play-id",
                    identity(play_id.as_str()),
                ));
            }
            if let Some(result) = &journey.result {
                properties.push(property(
                    &host,
                    "semantic-result-empty",
                    PresentationPropertyValue::Flag(result.is_empty()),
                ));
                if !result.is_empty() {
                    properties.push(property(
                        &host,
                        "semantic-result",
                        PresentationPropertyValue::Text(result.clone()),
                    ));
                }
            }
            properties.push(property(
                &host,
                "input-events",
                PresentationPropertyValue::Count(u64::from(journey.input_count)),
            ));
            properties.push(property(
                &host,
                "result-omitted-bytes",
                PresentationPropertyValue::Count(journey.result_omitted_bytes),
            ));
            if let Some(gap) = journey.kernel_sign_gap {
                for (name, value) in [
                    ("kernel-sign-gap-first-sequence", gap.first_sequence),
                    ("kernel-sign-gap-last-sequence", gap.last_sequence),
                    ("kernel-sign-gap-entries", gap.entries),
                ] {
                    properties.push(property(
                        &host,
                        name,
                        PresentationPropertyValue::Count(u64::from(value)),
                    ));
                }
            }
        }
        if let Some(line) = &self.connectivity {
            subjects.push(PresentationSubject {
                identity: line.line_id.clone(),
                role: PresentationRole::Line,
                label: "USB-backed Conduit Line".into(),
                accessibility_name: line.status.label().into(),
            });
            relationships.push(PresentationRelationship {
                source: host.clone(),
                target: line.line_id.clone(),
                kind: PresentationRelationshipKind::Connects,
            });
            properties.push(property(
                &line.line_id,
                "status",
                PresentationPropertyValue::Text(line.status.label().into()),
            ));
            properties.push(property(
                &line.line_id,
                "body-id-unchanged",
                identity(line.body_id.as_str()),
            ));
            if let Some(value) = &line.value {
                properties.push(property(
                    &line.line_id,
                    "received-value",
                    PresentationPropertyValue::Text(value.clone()),
                ));
            }
        }
        if let Some(refusal) = &self.refusal {
            properties.push(property(
                &host,
                refusal.key(),
                PresentationPropertyValue::Text(refusal.reason().into()),
            ));
        }
        let home_identity = "conduitos/shell/home";
        if let Some(view) = self.home_view() {
            subjects.push(PresentationSubject {
                identity: home_identity.into(),
                role: PresentationRole::Region,
                label: "ConduitOS Home".into(),
                accessibility_name: format!("ConduitOS Home; {} view", view.as_str()),
            });
            relationships.push(PresentationRelationship {
                source: home_identity.into(),
                target: host.clone(),
                kind: PresentationRelationshipKind::Observes,
            });
            properties.push(property(
                home_identity,
                "view",
                PresentationPropertyValue::Text(view.as_str().into()),
            ));
            properties.push(property(
                home_identity,
                "selected-launcher-index",
                PresentationPropertyValue::Count(self.home_selection().unwrap_or(0) as u64),
            ));
        }
        let basis = self.journey.as_ref().map_or_else(
            || PresentationBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_form_id: None,
                expanded_form_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            |journey| {
                if journey.body_id.is_some() && journey.wake_id.is_some() {
                    PresentationBasis {
                        body_id: journey.body_id.clone(),
                        wake_id: journey.wake_id.clone(),
                        source_document_id: journey.source_document_id.clone(),
                        checked_form_id: journey.checked_form_id.clone(),
                        expanded_form_id: journey.expanded_form_id.clone(),
                        plan_id: journey.plan_id.clone(),
                        active_play_id: journey.active_play_id.clone(),
                        sign_ids: journey
                            .input_sign_id
                            .iter()
                            .chain(journey.result_sign_id.iter())
                            .cloned()
                            .collect(),
                    }
                } else {
                    PresentationBasis {
                        body_id: None,
                        wake_id: None,
                        source_document_id: None,
                        checked_form_id: None,
                        expanded_form_id: None,
                        plan_id: None,
                        active_play_id: None,
                        sign_ids: vec![],
                    }
                }
            },
        );
        let host_text = self.journey.as_ref().map_or_else(
            || "BODY NONE; entering Patchbay creates no Body, Wake, Plan, or Play".into(),
            |journey| lifecycle_summary(journey).into(),
        );
        let mut texts = vec![
            PresentationText {
                subject: host.clone(),
                text: host_text,
            },
            PresentationText {
                subject: form.clone(),
                text: "IMAGE-embedded checked form; OPEN permits inspection only".into(),
            },
        ];
        let mut disclosures = vec![
            PresentationDisclosure {
                subject: form.clone(),
                level: PresentationDisclosureLevel::Primary,
            },
            PresentationDisclosure {
                subject: host.clone(),
                level: PresentationDisclosureLevel::Context,
            },
        ];
        if self.home_view().is_some() {
            texts.push(PresentationText {
                subject: home_identity.into(),
                text: "Persistent launcher and finite Conduit command bar".into(),
            });
            disclosures.push(PresentationDisclosure {
                subject: home_identity.into(),
                level: PresentationDisclosureLevel::Primary,
            });
        }
        Presentation::new_with_semantics(
            self.revision,
            basis,
            subjects,
            relationships,
            properties,
            texts,
            self.semantic_actions(&form),
            disclosures,
        )
        .map_err(|_| Error::Presentation)
    }
}

fn identity(value: &str) -> PresentationPropertyValue {
    PresentationPropertyValue::Identity(value.into())
}

fn property(subject: &str, name: &str, value: PresentationPropertyValue) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}
