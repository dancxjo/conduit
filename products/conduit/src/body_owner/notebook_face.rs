//! One canonical notebook Face for every admitted presentation Mask.
use super::*;
use conduit_face::{
    BodyFace, Face, FaceAction, FaceActionArgument, FaceActionAvailability, FaceContext,
    FaceContribution, FaceContributionBasis, FaceContributionRole, FaceDisclosure,
    FaceDisclosureLevel, FaceFocus, FaceFragment, FaceInteraction, FaceNames, FaceProperty,
    FacePropertyValue, FaceResidentPlotName, FaceRole, FaceSubject, FaceText, MaskShow,
};

pub(crate) enum NotebookAction {
    Replace(Vec<u8>),
    Save,
}

impl Owner {
    /// Declare the checked edit intent before a Play can offer it. This lets an
    /// admitted presentation Host prepare its bounded refresh route while idle;
    /// no document contribution or executable action is invented here.
    pub(crate) fn project_notebook_idle_face(&self) -> Result<Face, String> {
        let face = self.project_face(None)?;
        let Some(resident) = self
            .resident
            .as_ref()
            .filter(|_| self.resident_name.as_deref() == Some("notebook/transition"))
        else {
            return Ok(face);
        };
        let mut actions = face.actions.clone();
        actions.push(FaceAction {
            identity: "notebook.replace".into(),
            intent: "notebook/replace@1".into(),
            target: format!("plot/{}", resident.checked_plot_id.as_str()),
            name: "Replace text".into(),
            arguments: vec![FaceActionArgument::text(
                "text".into(),
                "Document text".into(),
                0,
                256,
            )
            .map_err(debug)?],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Unavailable {
                reason_code: "notebook-edit-encounter-idle".into(),
                explanation: "Start the notebook before editing.".into(),
            },
        });
        let revised = Face::new_with_semantics_and_temporal(
            face.revision
                .checked_add(
                    self.notebook
                        .as_ref()
                        .map_or(0, |document| document.navigation.revision),
                )
                .ok_or("notebook Face revision exhausted")?,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            actions,
            face.disclosures,
            face.temporal_references,
            face.temporal_facts,
        )
        .and_then(|value| value.with_composition(face.composition))
        .and_then(|value| value.with_interaction_context(face.interaction_context))
        .map_err(debug)?;
        revised.validate().map_err(debug)?;
        Ok(revised)
    }

    pub(crate) fn project_notebook_face(&self) -> Result<Face, String> {
        Ok(self.project_notebook_body_face()?.face)
    }

    pub(super) fn project_notebook_body_face(&self) -> Result<BodyFace, String> {
        let document = self.notebook.as_ref().ok_or("notebook is not open")?;
        let current = TextDocument::decode(&document.current).map_err(debug)?;
        let resident = self
            .resident
            .as_ref()
            .ok_or("notebook has no resident Source")?;
        let realization = self
            .session
            .realization()
            .ok_or("notebook has no admitted Plan")?;
        let play = realization
            .play
            .as_ref()
            .ok_or("notebook has no retained started Play")?;
        if self.resident_name.as_deref() != Some("notebook/transition")
            || realization.plan.plots.len() != 1
            || realization.plan.plots[0].plot != *resident
        {
            return Err("notebook Face differs from its current resident Play".into());
        }
        let target = "notebook/document".to_string();
        let status = "notebook/save-status".to_string();
        let status_text = if document.save_failed {
            "Save failed. Current text preserved."
        } else if document.committed.as_ref() == Some(&document.current) {
            "Saved"
        } else {
            "Unsaved changes"
        };
        let mut text = vec![FaceText {
            subject: status.clone(),
            text: status_text.into(),
        }];
        if !current.text.is_empty() {
            text.push(FaceText {
                subject: target.clone(),
                text: std::str::from_utf8(current.text)
                    .map_err(|error| error.to_string())?
                    .into(),
            });
        }
        #[cfg(target_os = "linux")]
        let save_available = document.storage.is_some();
        #[cfg(not(target_os = "linux"))]
        let save_available = false;
        let text_argument = FaceActionArgument::text("text".into(), "Document text".into(), 0, 256)
            .map_err(debug)?;
        let fragment = FaceFragment {
            basis: FaceContributionBasis {
                checked_plot_id: resident.checked_plot_id.clone(),
                plan_id: realization.plan.plan_id.clone(),
                active_play_id: play.active_play_id.clone(),
                required_interaction_context: None,
            },
            subjects: vec![
                FaceSubject {
                    identity: target.clone(),
                    role: FaceRole::TextEntry,
                    name: document.name.clone(),
                },
                FaceSubject {
                    identity: status.clone(),
                    role: FaceRole::Status,
                    name: "Save status".into(),
                },
            ],
            relationships: vec![conduit_face::FaceRelationship {
                source: target.clone(),
                target: status.clone(),
                kind: conduit_face::FaceRelationshipKind::Contains,
            }],
            composition: vec![],
            properties: vec![
                FaceProperty {
                    subject: target.clone(),
                    name: "notebook-revision".into(),
                    value: FacePropertyValue::Count(current.revision),
                },
                FaceProperty {
                    subject: target.clone(),
                    name: "current-value".into(),
                    value: FacePropertyValue::TypedValue {
                        contract: text_argument.contract.clone(),
                        bytes: current.text.to_vec(),
                    },
                },
                FaceProperty {
                    subject: target.clone(),
                    name: "value-change-action".into(),
                    value: FacePropertyValue::Identity("notebook.replace".into()),
                },
            ],
            text,
            actions: vec![
                FaceAction {
                    identity: "notebook.replace".into(),
                    intent: "notebook/replace@1".into(),
                    target: target.clone(),
                    name: "Replace text".into(),
                    arguments: vec![text_argument],
                    disclosure: FaceDisclosureLevel::CurrentAction,
                    availability: FaceActionAvailability::Available,
                },
                FaceAction {
                    identity: "notebook.save".into(),
                    intent: "notebook/save@1".into(),
                    target: target.clone(),
                    name: "Save".into(),
                    arguments: vec![],
                    disclosure: FaceDisclosureLevel::CurrentAction,
                    availability: if save_available {
                        FaceActionAvailability::Available
                    } else {
                        FaceActionAvailability::Unavailable {
                            reason_code: "notebook-storage-not-selected".into(),
                            explanation: "Select a storage directory before saving.".into(),
                        }
                    },
                },
            ],
            disclosures: vec![
                FaceDisclosure {
                    subject: target,
                    level: FaceDisclosureLevel::Primary,
                },
                FaceDisclosure {
                    subject: status,
                    level: FaceDisclosureLevel::Primary,
                },
            ],
            temporal_references: vec![],
            temporal_facts: vec![],
        };
        let names = [FaceResidentPlotName {
            source_document_id: &resident.source_document_id,
            checked_plot_id: &resident.checked_plot_id,
            name: "notebook/transition",
        }];
        let mut projected = BodyFace::project_with_names(
            &self.session.evidence().body,
            Some(&realization.wake),
            self.session
                .evidence()
                .last_sequence()
                .checked_add(document.navigation.revision)
                .ok_or("notebook Face revision exhausted")?,
            document.navigation.context.clone(),
            if matches!(document.navigation.context, FaceContext::ResidentPlot(_)) {
                FaceFocus::Contribution {
                    role: FaceContributionRole::Foreground,
                    node_key: None,
                }
            } else {
                FaceFocus::Body
            },
            vec![FaceContribution::from_face(
                FaceContributionRole::Foreground,
                fragment,
            )],
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &names,
            },
        )
        .map_err(debug)?;
        if matches!(document.navigation.context, FaceContext::Library) {
            for disclosure in &mut projected.face.disclosures {
                if disclosure.subject == format!("plot/{}", resident.checked_plot_id.as_str()) {
                    disclosure.level = FaceDisclosureLevel::Primary;
                } else if matches!(
                    disclosure.subject.as_str(),
                    "notebook/document" | "notebook/save-status"
                ) {
                    disclosure.level = FaceDisclosureLevel::Context;
                }
            }
            projected.face = Face::new_with_semantics_and_temporal(
                projected.face.revision,
                projected.face.basis,
                projected.face.subjects,
                projected.face.relationships,
                projected.face.properties,
                projected.face.text,
                projected.face.actions,
                projected.face.disclosures,
                projected.face.temporal_references,
                projected.face.temporal_facts,
            )
            .and_then(|face| face.with_composition(projected.face.composition))
            .and_then(|face| face.with_interaction_context(projected.face.interaction_context))
            .map_err(debug)?;
        }
        projected.face.validate().map_err(debug)?;
        Ok(projected)
    }

    pub(super) fn resolve_notebook_interaction(
        &self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<NotebookAction, String> {
        let face = self.project_notebook_face()?;
        interaction
            .validate_against(&face, show)
            .map_err(|error| format!("notebook current Show refused: {error:?}"))?;
        if interaction.target != "notebook/document" {
            return Err("notebook interaction differs from its checked target".into());
        }
        match (
            interaction.action_id.as_str(),
            interaction.arguments.as_slice(),
        ) {
            ("notebook.replace", [argument]) => Ok(NotebookAction::Replace(argument.value.clone())),
            ("notebook.save", []) => Ok(NotebookAction::Save),
            _ => Err("notebook interaction differs from its checked action".into()),
        }
    }
}
