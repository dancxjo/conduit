//! Native dispatch adapter for source-preserving typed Patchbay authoring edits.

use super::PatchbayApplication;
use patchbay_model::{
    PatchbayAction, PatchbayEdit, PatchbayEditBasis, PatchbayInteractionRequest,
    PatchbayInvocationOutcome, PatchbayRefusal, PatchbaySubjectRef,
};

impl PatchbayApplication {
    pub(super) fn dispatch_gear_configuration(
        &mut self,
        subject: &PatchbaySubjectRef,
        key: &str,
        value: conduit_core::ConfigurationValue,
    ) -> Result<(), String> {
        let basis = self.edit_basis(subject.expanded_form_id.clone())?;
        self.dispatch_authoring_edit(PatchbayEdit::ConfigureGear {
            basis,
            subject_identity: subject.subject_identity.clone(),
            key: key.into(),
            value,
        })
    }

    pub(super) fn dispatch_palette_placement(
        &mut self,
        kind: &str,
        target: (i32, i32),
    ) -> Result<(), String> {
        let prior_gears = self
            .graphical_form
            .as_ref()
            .ok_or("graphical Form projection is absent")?
            .gears
            .iter()
            .map(|gear| gear.identity.clone())
            .collect::<Vec<_>>();
        let expanded_form_id = self
            .graphical_form
            .as_ref()
            .ok_or("graphical Form projection is absent")?
            .expanded_form_id
            .clone();
        let basis = self.edit_basis(expanded_form_id)?;
        self.dispatch_authoring_edit(PatchbayEdit::PlaceGear {
            basis,
            kind_id: kind.into(),
        })?;
        let Some((graph, gear)) = self.graphical_form.as_ref().and_then(|graph| {
            graph
                .gears
                .iter()
                .find(|gear| !prior_gears.contains(&gear.identity))
                .map(|gear| (graph, gear))
        }) else {
            return Ok(());
        };
        let subject = graph
            .subject_ref(&gear.identity)
            .map_err(|error| error.to_string())?;
        self.layout
            .move_gear(graph, &subject, target.0, target.1)
            .map_err(|error| format!("palette placement target: {error:?}"))?;
        self.publish_completed(format!(
            "Added {} at canvas target {}, {}",
            gear.gear_id.as_str(),
            target.0,
            target.1
        ));
        Ok(())
    }

    pub(super) fn dispatch_gear_edit(
        &mut self,
        action: PatchbayAction,
        subject: &PatchbaySubjectRef,
    ) -> Result<(), String> {
        let basis = self.edit_basis(subject.expanded_form_id.clone())?;
        let edit = match action {
            PatchbayAction::DuplicateGear => PatchbayEdit::DuplicateGear {
                basis,
                subject_identity: subject.subject_identity.clone(),
            },
            PatchbayAction::RemoveGear => PatchbayEdit::RemoveGear {
                basis,
                subject_identity: subject.subject_identity.clone(),
            },
            PatchbayAction::RemoveCord => PatchbayEdit::RemoveCord {
                basis,
                subject_identity: subject.subject_identity.clone(),
            },
            _ => return Err("action is not a typed Gear or Cord edit".into()),
        };
        self.dispatch_authoring_edit(edit)
    }

    pub(super) fn dispatch_port_connection(
        &mut self,
        source: &PatchbaySubjectRef,
        sink: &PatchbaySubjectRef,
    ) -> Result<(), String> {
        if source.expanded_form_id != sink.expanded_form_id {
            return Err("Ports come from different checked Form revisions".into());
        }
        let basis = self.edit_basis(source.expanded_form_id.clone())?;
        self.dispatch_authoring_edit(PatchbayEdit::ConnectPorts {
            basis,
            source_identity: source.subject_identity.clone(),
            sink_identity: sink.subject_identity.clone(),
        })
    }

    pub(super) fn dispatch_cord_reroute(
        &mut self,
        cord: &PatchbaySubjectRef,
        endpoint: &PatchbaySubjectRef,
    ) -> Result<(), String> {
        if cord.expanded_form_id != endpoint.expanded_form_id {
            return Err("Cord and Port come from different checked Form revisions".into());
        }
        let basis = self.edit_basis(cord.expanded_form_id.clone())?;
        self.dispatch_authoring_edit(PatchbayEdit::RerouteCord {
            basis,
            cord_identity: cord.subject_identity.clone(),
            endpoint_identity: endpoint.subject_identity.clone(),
        })
    }

    fn edit_basis(
        &self,
        expanded_form_id: conduit_core::ExpandedFormId,
    ) -> Result<PatchbayEditBasis, String> {
        let editor = self
            .form_editor
            .as_ref()
            .ok_or("canonical Form editor is absent")?;
        let view = editor.view();
        let source_document_id = view
            .checked
            .source_document_id
            .ok_or("checked canonical Form identity is absent")?;
        PatchbayEditBasis::new(source_document_id, view.revision, expanded_form_id)
            .map_err(|error| format!("typed edit basis: {error:?}"))
    }

    fn dispatch_authoring_edit(&mut self, edit: PatchbayEdit) -> Result<(), String> {
        let mut interaction = self
            .interaction
            .take()
            .expect("interaction state is installed");
        let graph = self.graphical_form.clone();
        let result = interaction
            .next_request_id(edit.operation())
            .and_then(|request_id| PatchbayInteractionRequest::edit(request_id, edit))
            .and_then(|request| {
                interaction.execute(graph.as_ref(), request, |request| {
                    self.apply_interaction_request(request)
                })
            });
        self.interaction = Some(interaction);
        self.finish_interaction(result)
    }

    pub(super) fn apply_authoring_edit(
        &mut self,
        edit: &PatchbayEdit,
    ) -> PatchbayInvocationOutcome {
        if self.lifecycle_flow().state_code != "FORM_CHECKED" {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable);
        }
        let basis = edit.basis();
        let before = match self.semantic_checkpoint() {
            Ok(checkpoint) => checkpoint,
            Err(_) => return PatchbayInvocationOutcome::Failed,
        };
        let Some(editor) = self.form_editor.as_mut() else {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable);
        };
        let view = editor.view();
        if view.revision != basis.source_revision
            || view.checked.source_document_id.as_ref() != Some(&basis.source_document_id)
            || self
                .graphical_form
                .as_ref()
                .map(|graph| &graph.expanded_form_id)
                != Some(&basis.expanded_form_id)
        {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation);
        }
        let revision = basis.source_revision;
        let expanded = &basis.expanded_form_id;
        let result = match edit {
            PatchbayEdit::PlaceGear { kind_id, .. } => {
                let kind_id = conduit_core::kind_id(kind_id);
                match patchbay_model::GearPalette::standard() {
                    Ok(palette) if palette.find(&kind_id).is_some() => {
                        editor.place_palette_kind(revision, &kind_id).map(|_| ())
                    }
                    Ok(_) => {
                        return PatchbayInvocationOutcome::Refused(
                            PatchbayRefusal::OperationRejected,
                        )
                    }
                    Err(_) => return PatchbayInvocationOutcome::Failed,
                }
            }
            PatchbayEdit::DuplicateGear {
                subject_identity, ..
            } => direct_gear_name(subject_identity)
                .and_then(|name| editor.duplicate_gear(revision, name).map(|_| ())),
            PatchbayEdit::RemoveGear {
                subject_identity, ..
            } => direct_gear_name(subject_identity)
                .and_then(|name| editor.remove_gear(revision, name)),
            PatchbayEdit::RemoveCord {
                subject_identity, ..
            } => editor.remove_cord(revision, expanded, subject_identity),
            PatchbayEdit::ConnectPorts {
                source_identity,
                sink_identity,
                ..
            } => editor.connect_ports(revision, expanded, source_identity, sink_identity),
            PatchbayEdit::RerouteCord {
                cord_identity,
                endpoint_identity,
                ..
            } => editor.reroute_cord_endpoint(revision, expanded, cord_identity, endpoint_identity),
            PatchbayEdit::ConfigureGear {
                subject_identity,
                key,
                value,
                ..
            } => direct_gear_name(subject_identity).and_then(|name| {
                editor.set_gear_configuration(revision, expanded, name, key, value.clone())
            }),
        };
        if let Err(error) = result {
            return match error {
                patchbay_model::FormEditorError::IncompatiblePorts(_) => {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::IncompatiblePorts)
                }
                patchbay_model::FormEditorError::DuplicateCord => {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::DuplicateCord)
                }
                patchbay_model::FormEditorError::InvalidConfiguration(_)
                | patchbay_model::FormEditorError::UnknownConfiguration(_) => {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::InvalidConfiguration)
                }
                patchbay_model::FormEditorError::StaleRevision { .. }
                | patchbay_model::FormEditorError::StaleGraphBasis => {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
                }
                _ => PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected),
            };
        }
        self.form_selection = 0;
        if self.refresh_graphical_form().is_err() {
            return PatchbayInvocationOutcome::Failed;
        }
        let after = match self.semantic_checkpoint() {
            Ok(checkpoint) => checkpoint,
            Err(_) => return PatchbayInvocationOutcome::Failed,
        };
        let Some(history) = self.semantic_history.as_mut() else {
            return PatchbayInvocationOutcome::Failed;
        };
        if history.record_accepted(&before, after).is_err() {
            return PatchbayInvocationOutcome::Failed;
        }
        let title = self.title();
        if let Some(window) = &self.window {
            window.set_title(&title);
        }
        PatchbayInvocationOutcome::Succeeded
    }
}

fn direct_gear_name(subject_identity: &str) -> Result<&str, patchbay_model::FormEditorError> {
    let path = subject_identity
        .strip_prefix("gear/")
        .ok_or_else(|| patchbay_model::FormEditorError::UnknownGear(subject_identity.into()))?;
    let mut components = path.split('/');
    let _form = components.next();
    let name = components
        .next()
        .ok_or_else(|| patchbay_model::FormEditorError::UnknownGear(subject_identity.into()))?;
    if components.next().is_some() {
        return Err(patchbay_model::FormEditorError::NestedGearEditUnsupported(
            subject_identity.into(),
        ));
    }
    Ok(name)
}
