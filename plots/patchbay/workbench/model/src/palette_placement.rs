//! Catalog-backed placement edits canonical source, never a renderer graph.
use crate::plot_editor::{check_revision_with_catalog, ensure_source_bound};
use crate::{PlotEditor, PlotEditorError};

impl PlotEditor {
    /// Places one fresh semantic Gear by editing canonical Plot source. The
    /// offered revision is a stale-gesture precondition; no model state changes
    /// unless the resulting source parses and checks successfully.
    pub fn place_palette_kind(
        &mut self,
        offered_revision: u64,
        kind_id: &conduit_core::KindId,
    ) -> Result<String, PlotEditorError> {
        if offered_revision != self.revision {
            return Err(PlotEditorError::StaleRevision {
                current: self.revision,
                offered: offered_revision,
            });
        }
        let palette = self.authoring_catalog()?;
        let Some(kind) = palette
            .iter()
            .find(|kind| &kind.contract.kind_id == kind_id && kind.authorable)
        else {
            return Err(PlotEditorError::UnknownPaletteKind(kind_id.as_str().into()));
        };
        let entry = &kind.contract;
        let plot = self
            .checked
            .plots
            .iter()
            .find(|plot| plot.name == self.open_plot)
            .ok_or_else(|| PlotEditorError::UnknownPlot(self.open_plot.clone()))?;
        let stem = canonical_gear_stem(kind_id.as_str())?;
        let mut suffix = 1_u32;
        let name = loop {
            let candidate = if suffix == 1 {
                stem.clone()
            } else {
                format!("{stem}-{suffix}")
            };
            let identity = format!("plot/{}/gear/{candidate}", self.open_plot);
            if !plot.items.iter().any(|item| item.identity == identity) {
                break candidate;
            }
            suffix = suffix
                .checked_add(1)
                .ok_or(PlotEditorError::GraphTooLarge)?;
        };
        let close = self.source[plot.source_span.start..plot.source_span.end]
            .rfind('}')
            .map(|offset| plot.source_span.start + offset)
            .ok_or_else(|| PlotEditorError::UnknownPlot(self.open_plot.clone()))?;
        let mut candidate = self.source.clone();
        let invocation = if entry.configuration.is_empty() {
            kind_id.as_str().to_owned()
        } else {
            let arguments = entry
                .configuration
                .iter()
                .map(|field| {
                    crate::front_configuration::configuration_spelling(
                        &field.rule,
                        &field.default_value,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({arguments})", kind_id.as_str())
        };
        candidate.insert_str(close, &format!("    {name}: {invocation}\n"));
        ensure_source_bound(&candidate)?;
        let next_revision = self.revision.saturating_add(1);
        let checked =
            check_revision_with_catalog(next_revision, &candidate, &self.startup_catalog)?;
        if let Some(diagnostic) = checked.diagnostics.first() {
            return Err(PlotEditorError::Catalog(diagnostic.message.clone()));
        }
        self.source = candidate;
        self.revision = next_revision;
        self.checked = checked;
        self.selection = None;
        Ok(name)
    }
}

fn canonical_gear_stem(kind: &str) -> Result<String, PlotEditorError> {
    let stem = kind.rsplit('/').next().unwrap_or(kind);
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(PlotEditorError::InvalidGearName);
    }
    Ok(stem.into())
}
