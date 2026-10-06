//! Canonical pre-birth Plot authoring through the zero-body front door.

use crate::{
    OpenedFrontDoorSubject, PatchbayEdit, PatchbayInvocationOutcome, PatchbayRefusal,
    PlotCandidate, PlotDocumentView, PlotEditorError, ZeroBodyFrontDoor,
};

impl PlotCandidate {
    fn synchronize_from_editor(&mut self) -> Result<(), String> {
        let view = self.editor.view();
        let checked = view
            .checked
            .plots
            .first()
            .ok_or("edited Plot source contains no Plot")?;
        let source_document_id = view
            .checked
            .source_document_id
            .clone()
            .ok_or("edited Plot source is unchecked")?;
        self.source = view.source;
        self.source_document_id = source_document_id;
        self.checked_plot_id = checked.checked_plot_id.clone();
        Ok(())
    }
}

impl ZeroBodyFrontDoor {
    pub fn opened_plot_editor(&self) -> Option<crate::PlotEditor> {
        let OpenedFrontDoorSubject::Plot {
            checked_plot_id, ..
        } = self.opened.as_ref()?
        else {
            return None;
        };
        self.plots
            .iter()
            .find(|plot| &plot.checked_plot_id == checked_plot_id)
            .map(|plot| plot.editor.clone())
    }

    pub fn opened_plot_document(&self) -> Option<PlotDocumentView> {
        let OpenedFrontDoorSubject::Plot {
            checked_plot_id, ..
        } = self.opened.as_ref()?
        else {
            return None;
        };
        self.plots
            .iter()
            .find(|plot| &plot.checked_plot_id == checked_plot_id)
            .map(|plot| plot.editor.view())
    }

    pub fn apply_opened_plot_edit(&mut self, edit: &PatchbayEdit) -> PatchbayInvocationOutcome {
        match self.apply_opened_plot_edit_inner(edit) {
            Ok(()) => PatchbayInvocationOutcome::Succeeded,
            Err(PlotEditorError::IncompatiblePorts(_)) => {
                PatchbayInvocationOutcome::Refused(PatchbayRefusal::IncompatiblePorts)
            }
            Err(PlotEditorError::DuplicateCord) => {
                PatchbayInvocationOutcome::Refused(PatchbayRefusal::DuplicateCord)
            }
            Err(
                PlotEditorError::InvalidConfiguration(_) | PlotEditorError::UnknownConfiguration(_),
            ) => PatchbayInvocationOutcome::Refused(PatchbayRefusal::InvalidConfiguration),
            Err(PlotEditorError::StaleRevision { .. } | PlotEditorError::StaleGraphBasis) => {
                PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation)
            }
            Err(_) => PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationRejected),
        }
    }

    fn apply_opened_plot_edit_inner(&mut self, edit: &PatchbayEdit) -> Result<(), PlotEditorError> {
        let OpenedFrontDoorSubject::Plot {
            checked_plot_id, ..
        } = self
            .opened
            .clone()
            .ok_or_else(|| PlotEditorError::UnknownPlot("no opened Plot".into()))?
        else {
            return Err(PlotEditorError::UnknownPlot("no opened Plot".into()));
        };
        let plot = self
            .plots
            .iter_mut()
            .find(|plot| plot.checked_plot_id == checked_plot_id)
            .ok_or_else(|| PlotEditorError::UnknownPlot("opened Plot is absent".into()))?;
        let basis = edit.basis();
        let view = plot.editor.view();
        let graph = plot.editor.patchbay_graph_for_authoring(&view.open_plot)?;
        if view.revision != basis.source_revision
            || view.checked.source_document_id.as_ref() != Some(&basis.source_document_id)
            || graph.expanded_plot_id != basis.expanded_plot_id
        {
            return Err(PlotEditorError::StaleGraphBasis);
        }
        let revision = basis.source_revision;
        let expanded = &basis.expanded_plot_id;
        match edit {
            PatchbayEdit::PlaceGear { kind_id, .. } => plot
                .editor
                .place_palette_kind(revision, &conduit_core::kind_id(kind_id))
                .map(|_| ()),
            PatchbayEdit::DuplicateGear {
                subject_identity, ..
            } => direct_gear_name(&view.open_plot, subject_identity)
                .and_then(|name| plot.editor.duplicate_gear(revision, name).map(|_| ())),
            PatchbayEdit::RemoveGear {
                subject_identity, ..
            } => direct_gear_name(&view.open_plot, subject_identity)
                .and_then(|name| plot.editor.remove_gear(revision, name)),
            PatchbayEdit::RemoveCord {
                subject_identity, ..
            } => plot
                .editor
                .remove_cord(revision, expanded, subject_identity),
            PatchbayEdit::ConnectPorts {
                source_identity,
                sink_identity,
                ..
            } => plot
                .editor
                .connect_ports(revision, expanded, source_identity, sink_identity),
            PatchbayEdit::RerouteCord {
                cord_identity,
                endpoint_identity,
                ..
            } => plot.editor.reroute_cord_endpoint(
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
            } => direct_gear_name(&view.open_plot, subject_identity).and_then(|name| {
                plot.editor
                    .set_gear_configuration(revision, expanded, name, key, value.clone())
            }),
        }?;
        plot.synchronize_from_editor()
            .map_err(PlotEditorError::Catalog)?;
        let next_checked_plot_id = plot.checked_plot_id.clone();
        plot.freshness_sequence = plot.freshness_sequence.saturating_add(1);
        self.opened = Some(OpenedFrontDoorSubject::Plot {
            checked_plot_id: next_checked_plot_id,
            observed_at: plot.freshness_sequence,
        });
        self.advance().map_err(PlotEditorError::Catalog)
    }

    pub fn mark_opened_plot_saved(&mut self, revision: u64) -> Result<(), String> {
        let OpenedFrontDoorSubject::Plot {
            checked_plot_id, ..
        } = self.opened.clone().ok_or("SAVE requires an opened Plot")?
        else {
            return Err("SAVE requires an opened Plot".into());
        };
        let plot = self
            .plots
            .iter_mut()
            .find(|plot| plot.checked_plot_id == checked_plot_id)
            .ok_or("opened Plot is absent")?;
        plot.editor
            .mark_saved(revision)
            .map_err(|error| error.to_string())?;
        self.advance()
    }
}

fn direct_gear_name<'a>(plot: &str, identity: &'a str) -> Result<&'a str, PlotEditorError> {
    identity
        .strip_prefix(&format!("gear/{plot}/"))
        .filter(|name| !name.is_empty() && !name.contains('/'))
        .ok_or_else(|| PlotEditorError::UnknownGear(identity.into()))
}
