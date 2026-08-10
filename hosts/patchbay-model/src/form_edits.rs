//! Bounded source-preserving semantic edits over the canonical Form document.

use crate::form_editor::{check_revision, ensure_source_bound, FormEditor, FormEditorError};

impl FormEditor {
    /// Places one fresh semantic Gear by editing canonical Form source. The
    /// offered revision is a stale-gesture precondition; no model state changes
    /// unless the resulting source parses and checks successfully.
    pub fn place_palette_kind(
        &mut self,
        offered_revision: u64,
        kind_id: &conduit_core::KindId,
    ) -> Result<String, FormEditorError> {
        if offered_revision != self.revision {
            return Err(FormEditorError::StaleRevision {
                current: self.revision,
                offered: offered_revision,
            });
        }
        let palette = crate::GearPalette::standard()
            .map_err(|error| FormEditorError::Catalog(format!("{error:?}")))?;
        if palette.find(kind_id).is_none() {
            return Err(FormEditorError::UnknownPaletteKind(kind_id.as_str().into()));
        }
        let form = self
            .checked
            .forms
            .iter()
            .find(|form| form.name == self.open_form)
            .ok_or_else(|| FormEditorError::UnknownForm(self.open_form.clone()))?;
        let stem = canonical_gear_stem(kind_id.as_str())?;
        let mut suffix = 1_u32;
        let name = loop {
            let candidate = if suffix == 1 {
                stem.clone()
            } else {
                format!("{stem}-{suffix}")
            };
            let identity = format!("form/{}/gear/{candidate}", self.open_form);
            if !form.items.iter().any(|item| item.identity == identity) {
                break candidate;
            }
            suffix = suffix
                .checked_add(1)
                .ok_or(FormEditorError::GraphTooLarge)?;
        };
        let close = self.source[form.source_span.start..form.source_span.end]
            .rfind('}')
            .map(|offset| form.source_span.start + offset)
            .ok_or_else(|| FormEditorError::UnknownForm(self.open_form.clone()))?;
        let mut candidate = self.source.clone();
        candidate.insert_str(close, &format!("    {name}: {}\n", kind_id.as_str()));
        ensure_source_bound(&candidate)?;
        let next_revision = self.revision.saturating_add(1);
        let checked = check_revision(next_revision, &candidate)?;
        if let Some(diagnostic) = checked.diagnostics.first() {
            return Err(FormEditorError::Catalog(diagnostic.message.clone()));
        }
        self.source = candidate;
        self.revision = next_revision;
        self.checked = checked;
        self.selection = None;
        Ok(name)
    }
}

fn canonical_gear_stem(kind: &str) -> Result<String, FormEditorError> {
    let stem = kind.rsplit('/').next().unwrap_or(kind);
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(FormEditorError::InvalidGearName);
    }
    Ok(stem.into())
}
