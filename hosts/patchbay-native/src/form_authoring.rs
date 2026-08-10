//! Native dispatch adapter for source-preserving Patchbay authoring interactions.

use super::PatchbayApplication;
use patchbay_model::{
    PatchbayAction, PatchbayInteractionRequest, PatchbayInvocation, PatchbayInvocationOutcome,
    PatchbayRefusal, PatchbaySubjectRef,
};

impl PatchbayApplication {
    pub(super) fn dispatch_palette_placement(&mut self, kind: &str) -> Result<(), String> {
        let editor = self
            .form_editor
            .as_ref()
            .ok_or("canonical Form editor is absent")?;
        let view = editor.view();
        let source_id = view
            .checked
            .source_document_id
            .ok_or("checked canonical Form identity is absent")?;
        let target = format!("{}@{}@{kind}", source_id.as_str(), view.revision);
        self.dispatch_authoring_invocation(PatchbayAction::PlaceGear, target)
    }

    pub(super) fn dispatch_gear_edit(
        &mut self,
        action: PatchbayAction,
        subject: &PatchbaySubjectRef,
    ) -> Result<(), String> {
        let editor = self
            .form_editor
            .as_ref()
            .ok_or("canonical Form editor is absent")?;
        let view = editor.view();
        let source_id = view
            .checked
            .source_document_id
            .ok_or("checked canonical Form identity is absent")?;
        let target = format!(
            "{}@{}@{}@{}",
            source_id.as_str(),
            view.revision,
            subject.expanded_form_id.as_str(),
            subject.subject_identity
        );
        self.dispatch_authoring_invocation(action, target)
    }

    pub(super) fn dispatch_port_connection(
        &mut self,
        source: &PatchbaySubjectRef,
        sink: &PatchbaySubjectRef,
    ) -> Result<(), String> {
        if source.expanded_form_id != sink.expanded_form_id {
            return Err("Ports come from different checked Form revisions".into());
        }
        let editor = self
            .form_editor
            .as_ref()
            .ok_or("canonical Form editor is absent")?;
        let view = editor.view();
        let source_id = view
            .checked
            .source_document_id
            .ok_or("checked canonical Form identity is absent")?;
        let target = format!(
            "{}@{}@{}@{}@{}",
            source_id.as_str(),
            view.revision,
            source.expanded_form_id.as_str(),
            source.subject_identity,
            sink.subject_identity
        );
        self.dispatch_authoring_invocation(PatchbayAction::ConnectPorts, target)
    }

    fn dispatch_authoring_invocation(
        &mut self,
        action: PatchbayAction,
        target: String,
    ) -> Result<(), String> {
        let mut interaction = self
            .interaction
            .take()
            .expect("interaction state is installed");
        let graph = self.graphical_form.clone();
        let result = interaction
            .next_request_id(action.as_str())
            .and_then(|request_id| PatchbayInteractionRequest::invoke(request_id, action, target))
            .and_then(|request| {
                interaction.execute(graph.as_ref(), request, |invocation| {
                    self.apply_invocation(invocation)
                })
            });
        self.interaction = Some(interaction);
        self.finish_interaction(result)
    }

    pub(super) fn apply_palette_placement(&mut self, target: &str) -> PatchbayInvocationOutcome {
        let mut fields = target.splitn(3, '@');
        let (Some(source_id), Some(revision), Some(kind)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected);
        };
        let Ok(revision) = revision.parse::<u64>() else {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected);
        };
        let Some(editor) = self.form_editor.as_mut() else {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable);
        };
        let view = editor.view();
        if view.revision != revision
            || view
                .checked
                .source_document_id
                .as_ref()
                .map(|id| id.as_str())
                != Some(source_id)
        {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation);
        }
        let kind_id = conduit_core::kind_id(kind);
        let Ok(palette) = patchbay_model::GearPalette::standard() else {
            return PatchbayInvocationOutcome::Failed;
        };
        if palette.find(&kind_id).is_none() {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected);
        }
        if editor.place_palette_kind(revision, &kind_id).is_err() {
            return PatchbayInvocationOutcome::Failed;
        }
        self.form_selection = 0;
        if self.refresh_graphical_form().is_err() {
            return PatchbayInvocationOutcome::Failed;
        }
        let title = self.title();
        if let Some(window) = &self.window {
            window.set_title(&title);
        }
        PatchbayInvocationOutcome::Succeeded
    }

    pub(super) fn apply_authoring_edit(
        &mut self,
        invocation: &PatchbayInvocation,
    ) -> PatchbayInvocationOutcome {
        let fields = invocation.target_identity.split('@').collect::<Vec<_>>();
        let expected = if invocation.action == PatchbayAction::ConnectPorts {
            5
        } else {
            4
        };
        if fields.len() != expected {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected);
        }
        let Ok(revision) = fields[1].parse::<u64>() else {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected);
        };
        let Some(editor) = self.form_editor.as_mut() else {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable);
        };
        let view = editor.view();
        if view.revision != revision
            || view
                .checked
                .source_document_id
                .as_ref()
                .map(|id| id.as_str())
                != Some(fields[0])
            || self
                .graphical_form
                .as_ref()
                .map(|graph| graph.expanded_form_id.as_str())
                != Some(fields[2])
        {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation);
        }
        let edit = match invocation.action {
            PatchbayAction::DuplicateGear => direct_gear_name(fields[3])
                .and_then(|name| editor.duplicate_gear(revision, name).map(|_| ())),
            PatchbayAction::RemoveGear => {
                direct_gear_name(fields[3]).and_then(|name| editor.remove_gear(revision, name))
            }
            PatchbayAction::ConnectPorts => editor.connect_ports(
                revision,
                &conduit_core::ExpandedFormId::from(fields[2]),
                fields[3],
                fields[4],
            ),
            _ => unreachable!("authoring action was matched"),
        };
        if let Err(error) = edit {
            return match error {
                patchbay_model::FormEditorError::IncompatiblePorts(_) => {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::IncompatiblePorts)
                }
                patchbay_model::FormEditorError::DuplicateCord => {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::DuplicateCord)
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
