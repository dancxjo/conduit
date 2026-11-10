//! Root-owned acquisition of the installed Body's retained Source.
use conduit_body::{BodyBiographyEvidence, ResidentPlot};
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

/// Immutable Root-owned capture. It contains no checked candidate and grants no
/// Source operation or execution authority. Capture/JSON workspace is still the
/// caller's responsibility; compiler preparation can be admitted separately.
pub(super) struct CapturedRetainedWorkset {
    biography: BodyBiographyEvidence,
    inventory: CapturedInventory,
}
enum CapturedInventory {
    Complete(super::state::WorksetSources),
    Single(CapturedSourceFile),
}
impl CapturedRetainedWorkset {
    /// Compile only the already captured bytes, without reopening Source paths.
    /// The exact complete resident workset remains the acceptance boundary.
    pub(super) fn expand(self) -> Result<Vec<RetainedPlotSource>, String> {
        let captured = match self.inventory {
            CapturedInventory::Complete(sources) => return sources.into_expanded(&self.biography),
            CapturedInventory::Single(captured) => captured,
        };
        let source = crate::plot_source::parse_owned(captured.into_source())?;
        let checked = source.expand_entry_for_authoring()?;
        let resident = ResidentPlot::new(
            checked.expanded.source_document_id.clone(),
            checked.expanded.checked_plot_id.clone(),
        );
        if !self.biography.body.workset.plots().contains(&resident) {
            return Err("retained checked Plot differs from the installed Body workset".into());
        }
        if self.biography.body.workset.plots().len() != 1 {
            return Err("retained Source does not cover the complete Body workset".into());
        }
        Ok(vec![RetainedPlotSource {
            source,
            plot: checked,
        }])
    }
}

pub(super) fn workset(root: &Path) -> Result<Option<Vec<RetainedPlotSource>>, String> {
    capture(root)?
        .map(CapturedRetainedWorkset::expand)
        .transpose()
}

pub(super) fn capture(root: &Path) -> Result<Option<CapturedRetainedWorkset>, String> {
    let biography = super::state::load(root)?;
    if let Some(biography) = &biography {
        if let Some(sources) = super::state::capture_workset_sources(root, biography)? {
            return Ok(Some(CapturedRetainedWorkset {
                biography: biography.clone(),
                inventory: CapturedInventory::Complete(sources),
            }));
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
    Ok(Some(CapturedRetainedWorkset {
        biography,
        inventory: CapturedInventory::Single(captured),
    }))
}

#[cfg(test)]
mod tests;
