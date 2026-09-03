//! Canonical pre-Birth Form authoring through the zero-Body front door.

use crate::{
    FormCandidate, FormDocumentView, FormEditorError, OpenedFrontDoorSubject, PatchbayEdit,
    PatchbayInvocationOutcome, PatchbayRefusal, ZeroBodyFrontDoor,
};

impl FormCandidate {
    fn synchronize_from_editor(&mut self) -> Result<(), String> {
        let view = self.editor.view();
        let checked = view
            .checked
            .forms
            .first()
            .ok_or("edited Form source contains no Form")?;
        let source_document_id = view
            .checked
            .source_document_id
            .clone()
            .ok_or("edited Form source is unchecked")?;
        self.source = view.source;
        self.source_document_id = source_document_id;
        self.checked_form_id = checked.checked_form_id.clone();
        Ok(())
    }
}

impl ZeroBodyFrontDoor {
    pub fn opened_form_document(&self) -> Option<FormDocumentView> {
        let OpenedFrontDoorSubject::Form {
            checked_form_id, ..
        } = self.opened.as_ref()?
        else {
            return None;
        };
        self.forms
            .iter()
            .find(|form| &form.checked_form_id == checked_form_id)
            .map(|form| form.editor.view())
    }

    pub fn apply_opened_form_edit(&mut self, edit: &PatchbayEdit) -> PatchbayInvocationOutcome {
        match self.apply_opened_form_edit_inner(edit) {
            Ok(()) => PatchbayInvocationOutcome::Succeeded,
            Err(FormEditorError::IncompatiblePorts(_)) => {
                PatchbayInvocationOutcome::Refused(PatchbayRefusal::IncompatiblePorts)
            }
            Err(FormEditorError::DuplicateCord) => {
                PatchbayInvocationOutcome::Refused(PatchbayRefusal::DuplicateCord)
            }
            Err(
                FormEditorError::InvalidConfiguration(_) | FormEditorError::UnknownConfiguration(_),
            ) => PatchbayInvocationOutcome::Refused(PatchbayRefusal::InvalidConfiguration),
            Err(FormEditorError::StaleRevision { .. } | FormEditorError::StaleGraphBasis) => {
                PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
            }
            Err(_) => PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected),
        }
    }

    fn apply_opened_form_edit_inner(&mut self, edit: &PatchbayEdit) -> Result<(), FormEditorError> {
        let OpenedFrontDoorSubject::Form {
            checked_form_id, ..
        } = self
            .opened
            .clone()
            .ok_or_else(|| FormEditorError::UnknownForm("no opened Form".into()))?
        else {
            return Err(FormEditorError::UnknownForm("no opened Form".into()));
        };
        let form = self
            .forms
            .iter_mut()
            .find(|form| form.checked_form_id == checked_form_id)
            .ok_or_else(|| FormEditorError::UnknownForm("opened Form is absent".into()))?;
        let basis = edit.basis();
        let view = form.editor.view();
        let graph = form.editor.patchbay_graph_for_authoring(&view.open_form)?;
        if view.revision != basis.source_revision
            || view.checked.source_document_id.as_ref() != Some(&basis.source_document_id)
            || graph.expanded_form_id != basis.expanded_form_id
        {
            return Err(FormEditorError::StaleGraphBasis);
        }
        let revision = basis.source_revision;
        let expanded = &basis.expanded_form_id;
        match edit {
            PatchbayEdit::PlaceGear { kind_id, .. } => form
                .editor
                .place_palette_kind(revision, &conduit_core::kind_id(kind_id))
                .map(|_| ()),
            PatchbayEdit::DuplicateGear {
                subject_identity, ..
            } => direct_gear_name(&view.open_form, subject_identity)
                .and_then(|name| form.editor.duplicate_gear(revision, name).map(|_| ())),
            PatchbayEdit::RemoveGear {
                subject_identity, ..
            } => direct_gear_name(&view.open_form, subject_identity)
                .and_then(|name| form.editor.remove_gear(revision, name)),
            PatchbayEdit::RemoveCord {
                subject_identity, ..
            } => form
                .editor
                .remove_cord(revision, expanded, subject_identity),
            PatchbayEdit::ConnectPorts {
                source_identity,
                sink_identity,
                ..
            } => form
                .editor
                .connect_ports(revision, expanded, source_identity, sink_identity),
            PatchbayEdit::RerouteCord {
                cord_identity,
                endpoint_identity,
                ..
            } => form.editor.reroute_cord_endpoint(
                revision,
                expanded,
                cord_identity,
                endpoint_identity,
            ),
            PatchbayEdit::ConfigureGear {
                subject_identity,
                key,
                value,
                ..
            } => direct_gear_name(&view.open_form, subject_identity).and_then(|name| {
                form.editor
                    .set_gear_configuration(revision, expanded, name, key, value.clone())
            }),
        }?;
        form.synchronize_from_editor()
            .map_err(FormEditorError::Catalog)?;
        let next_checked_form_id = form.checked_form_id.clone();
        form.freshness_sequence = form.freshness_sequence.saturating_add(1);
        self.opened = Some(OpenedFrontDoorSubject::Form {
            checked_form_id: next_checked_form_id,
            observed_at: form.freshness_sequence,
        });
        self.advance().map_err(FormEditorError::Catalog)
    }

    pub fn mark_opened_form_saved(&mut self, revision: u64) -> Result<(), String> {
        let OpenedFrontDoorSubject::Form {
            checked_form_id, ..
        } = self.opened.clone().ok_or("SAVE requires an opened Form")?
        else {
            return Err("SAVE requires an opened Form".into());
        };
        let form = self
            .forms
            .iter_mut()
            .find(|form| form.checked_form_id == checked_form_id)
            .ok_or("opened Form is absent")?;
        form.editor
            .mark_saved(revision)
            .map_err(|error| error.to_string())?;
        self.advance()
    }
}

fn direct_gear_name<'a>(form: &str, identity: &'a str) -> Result<&'a str, FormEditorError> {
    identity
        .strip_prefix(&format!("gear/{form}/"))
        .filter(|name| !name.is_empty() && !name.contains('/'))
        .ok_or_else(|| FormEditorError::UnknownGear(identity.into()))
}
