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
    pub(crate) fn project_notebook_face(&self) -> Result<Face, String> {
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
        let face = BodyFace::project_with_names(
            &self.session.evidence().body,
            Some(&realization.wake),
            self.session.evidence().last_sequence(),
            FaceContext::Overview,
            FaceFocus::Body,
            vec![FaceContribution::from_face(
                FaceContributionRole::Foreground,
                fragment,
            )],
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &names,
            },
        )
        .map_err(debug)?
        .face;
        face.validate().map_err(debug)?;
        Ok(face)
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
