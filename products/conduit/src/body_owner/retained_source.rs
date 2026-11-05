//! Root-owned acquisition of the installed Body's retained Source.
use conduit_body::ResidentPlot;
use conduit_plot::{canonical_source_document_id, ExpandedAuthoringPlot};
use conduit_std_host::source_file::CapturedSourceFile;
use std::{fs::File, io::ErrorKind, path::Path};

/// Recover the authoritative installation/biography pair before selecting its
/// Source. Absence is observable as absence; unrelated bytes cannot request
/// parsing, scoped equipment, or restoration of a resident Plot.
pub(super) struct RetainedPlotSource {
    pub(super) source: crate::plot_source::CanonicalSource,
    pub(super) plot: ExpandedAuthoringPlot,
}

pub(super) fn checked(root: &Path) -> Result<Option<ExpandedAuthoringPlot>, String> {
    match workset(root)? {
        None => Ok(None),
        Some(mut sources) if sources.len() <= 1 => Ok(sources.pop().map(|source| source.plot)),
        Some(_) => Err("single-Plot operation requires one Source; no partial selection".into()),
    }
}

pub(super) fn workset(root: &Path) -> Result<Option<Vec<RetainedPlotSource>>, String> {
    let biography = super::state::load(root)?;
    if let Some(biography) = &biography {
        if let Some(plots) = super::state::checked_workset_sources(root, biography)? {
            return Ok(Some(plots));
        }
    }
    let file = match File::open(root.join("body/source.conduit")) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("retained Source selection: {error}")),
    };
    let biography = biography.ok_or("retained Source has no installed Body owner")?;
    let captured = CapturedSourceFile::from_file(file, super::MAXIMUM_SOURCE as usize)
        .map_err(|error| format!("retained Source capture: {error:?}"))?;
    let source_id = canonical_source_document_id(captured.source());
    if !biography
        .body
        .workset
        .plots()
        .iter()
        .any(|resident| resident.source_document_id == source_id)
    {
        return Err("retained Source bytes differ from the installed Body workset".into());
    }
    // Content membership is not checked-Plot identity. Recheck the same captured
    // bytes through the ordinary product entrance and require the exact pair.
    // No Source pathname is reopened after selection.
    let source = crate::plot_source::parse(captured.source())?;
    let checked = source.expand_entry_for_authoring()?;
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id.clone(),
        checked.expanded.checked_plot_id.clone(),
    );
    if !biography.body.workset.plots().contains(&resident) {
        return Err("retained checked Plot differs from the installed Body workset".into());
    }
    if biography.body.workset.plots().len() != 1 {
        return Err("retained Source does not cover the complete Body workset".into());
    }
    Ok(Some(vec![RetainedPlotSource {
        source,
        plot: checked,
    }]))
}

#[cfg(test)]
mod tests;
