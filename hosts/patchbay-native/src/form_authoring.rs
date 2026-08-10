//! Native dispatch adapter for source-preserving Patchbay authoring interactions.

use super::PatchbayApplication;
use patchbay_model::{
    PatchbayAction, PatchbayInteractionRequest, PatchbayInvocation, PatchbayInvocationOutcome,
    PatchbayRefusal, PatchbaySubjectRef,
};

impl PatchbayApplication {
    pub(super) fn dispatch_gear_configuration(
        &mut self,
        subject: &PatchbaySubjectRef,
        key: &str,
        value: conduit_core::ConfigurationValue,
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
            "{}@{}@{}@{}@{}@{}",
            source_id.as_str(),
            view.revision,
            subject.expanded_form_id.as_str(),
            subject.subject_identity,
            key,
            encode_value(&value)
        );
        self.dispatch_authoring_invocation(PatchbayAction::ConfigureGear, target)
    }

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

    pub(super) fn dispatch_cord_reroute(
        &mut self,
        cord: &PatchbaySubjectRef,
        endpoint: &PatchbaySubjectRef,
    ) -> Result<(), String> {
        if cord.expanded_form_id != endpoint.expanded_form_id {
            return Err("Cord and Port come from different checked Form revisions".into());
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
            cord.expanded_form_id.as_str(),
            cord.subject_identity,
            endpoint.subject_identity
        );
        self.dispatch_authoring_invocation(PatchbayAction::RerouteCord, target)
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
        let expected = match invocation.action {
            PatchbayAction::ConnectPorts | PatchbayAction::RerouteCord => 5,
            PatchbayAction::ConfigureGear => 6,
            _ => 4,
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
            PatchbayAction::RemoveCord => editor.remove_cord(
                revision,
                &conduit_core::ExpandedFormId::from(fields[2]),
                fields[3],
            ),
            PatchbayAction::ConnectPorts => editor.connect_ports(
                revision,
                &conduit_core::ExpandedFormId::from(fields[2]),
                fields[3],
                fields[4],
            ),
            PatchbayAction::RerouteCord => editor.reroute_cord_endpoint(
                revision,
                &conduit_core::ExpandedFormId::from(fields[2]),
                fields[3],
                fields[4],
            ),
            PatchbayAction::ConfigureGear => direct_gear_name(fields[3]).and_then(|name| {
                let value = decode_value(fields[5])?;
                editor.set_gear_configuration(
                    revision,
                    &conduit_core::ExpandedFormId::from(fields[2]),
                    name,
                    fields[4],
                    value,
                )
            }),
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
        PatchbayInvocationOutcome::Succeeded
    }
}

fn encode_value(value: &conduit_core::ConfigurationValue) -> String {
    let (tag, bytes): (&str, Vec<u8>) = match value {
        conduit_core::ConfigurationValue::Bool(value) => ("b", value.to_string().into_bytes()),
        conduit_core::ConfigurationValue::U64(value) => ("u", value.to_string().into_bytes()),
        conduit_core::ConfigurationValue::Text(value) => ("t", value.as_bytes().to_vec()),
    };
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{tag}{hex}")
}

fn decode_value(
    encoded: &str,
) -> Result<conduit_core::ConfigurationValue, patchbay_model::FormEditorError> {
    let (tag, hex) = encoded.split_at(encoded.len().min(1));
    if hex.len() % 2 != 0 {
        return Err(patchbay_model::FormEditorError::InvalidConfiguration(
            "malformed control value".into(),
        ));
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            patchbay_model::FormEditorError::InvalidConfiguration("malformed control value".into())
        })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        patchbay_model::FormEditorError::InvalidConfiguration("control text is not UTF-8".into())
    })?;
    match tag {
        "b" => text
            .parse::<bool>()
            .map(conduit_core::ConfigurationValue::Bool)
            .map_err(|_| {
                patchbay_model::FormEditorError::InvalidConfiguration(
                    "expected true or false".into(),
                )
            }),
        "u" => text
            .parse::<u64>()
            .map(conduit_core::ConfigurationValue::U64)
            .map_err(|_| {
                patchbay_model::FormEditorError::InvalidConfiguration(
                    "expected a whole number".into(),
                )
            }),
        "t" => Ok(conduit_core::ConfigurationValue::Text(text)),
        _ => Err(patchbay_model::FormEditorError::InvalidConfiguration(
            "unknown control value type".into(),
        )),
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
