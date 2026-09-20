//! Shared Crèche draft and explicit Birth handoff for the zero-body entrance.

use conduit_core::SignId;
use conduit_creche_model::birth::{BirthDraft, BirthFormChoice, BirthSelection};

use crate::{LocalFrontDoor, ZeroBodyFrontDoor};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedBirthEvidence {
    pub selection_revision: u32,
    pub friendly_name: String,
    pub workset: conduit_body::BodyWorkset,
    pub sign_id: SignId,
}

impl ZeroBodyFrontDoor {
    /// Build the shared Crèche draft from current reviewed inventory. Opening
    /// the draft grants no Form special status and creates no Body.
    pub fn creche_draft(&self, uuid: String) -> Result<BirthDraft, String> {
        BirthDraft::new(
            uuid,
            self.forms
                .iter()
                .map(|form| BirthFormChoice {
                    title: form.label.clone(),
                    search_text: format!("{} {}", form.source_name, form.provenance),
                    form: conduit_body::ResidentForm::new(
                        form.source_document_id.clone(),
                        form.checked_form_id.clone(),
                    ),
                    refusal: None,
                    selected: false,
                })
                .collect(),
        )
        .map(BirthDraft::allow_idle_body)
        .map_err(|error| format!("Crèche draft: {error:?}"))
    }

    /// Birth from the exact shared Crèche selection. Zero, one, and many
    /// resident Forms all cross this same explicit lifecycle boundary.
    pub fn birth_from_creche(
        self,
        selection: BirthSelection,
        revision: u64,
    ) -> Result<LocalFrontDoor, String> {
        self.require_revision(revision)?;
        let selected = selection
            .workset
            .forms()
            .iter()
            .map(|resident| {
                self.forms
                    .iter()
                    .find(|candidate| {
                        candidate.source_document_id == resident.source_document_id
                            && candidate.checked_form_id == resident.checked_form_id
                    })
                    .cloned()
                    .ok_or_else(|| {
                        "Crèche selected a form outside current reviewed inventory".into()
                    })
            })
            .collect::<Result<Vec<_>, String>>()?;
        LocalFrontDoor::born_from_selection(
            self.adapter,
            self.model,
            selection,
            selected,
            self.revision,
        )
    }
}
