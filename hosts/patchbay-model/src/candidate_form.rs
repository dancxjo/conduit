//! Patchbay inspection entrance for an inert model-produced candidate Form.

use conduit_ai::{CandidateForm, CandidateFormProvenance, CandidateLifecycle};
use conduit_form::{ProfileCatalog, StartupCatalog};
use std::path::PathBuf;

use crate::{FormEditor, FormEditorError};

pub struct PatchbayCandidateForm {
    pub candidate_identity: String,
    pub provenance: CandidateFormProvenance,
    pub lifecycle: CandidateLifecycle,
    pub editor: FormEditor,
}

impl PatchbayCandidateForm {
    /// Opens candidate source in the ordinary editor. Planning and playing remain separate,
    /// explicit lifecycle actions outside this adapter.
    pub fn open(
        candidate: CandidateForm,
        startup: StartupCatalog,
        profile: ProfileCatalog,
    ) -> Result<Self, FormEditorError> {
        let editor = FormEditor::from_source_with_catalogs(
            PathBuf::from("candidate.conduit"),
            candidate.source,
            startup,
            profile,
        )?;
        Ok(Self {
            candidate_identity: candidate.candidate_identity,
            provenance: candidate.provenance,
            lifecycle: candidate.lifecycle,
            editor,
        })
    }
}

#[cfg(test)]
#[path = "candidate_form_tests.rs"]
mod tests;
