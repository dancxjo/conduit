//! Shared Crèche draft and explicit Birth handoff for the zero-body entrance.

use conduit_birth_plot::{BirthDraft, BirthPlotChoice, BirthSelection};
use conduit_core::SignId;

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
    /// the draft grants no Plot special status and creates no Body.
    pub fn creche_draft(&self, uuid: String) -> Result<BirthDraft, String> {
        BirthDraft::new(
            uuid,
            self.plots
                .iter()
                .map(|plot| BirthPlotChoice {
                    title: plot.label.clone(),
                    search_text: format!("{} {}", plot.source_name, plot.provenance),
                    plot: conduit_body::ResidentPlot::new(
                        plot.source_document_id.clone(),
                        plot.checked_plot_id.clone(),
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
    /// resident Plots all cross this same explicit lifecycle boundary.
    pub fn birth_from_creche(
        self,
        selection: BirthSelection,
        revision: u64,
    ) -> Result<LocalFrontDoor, String> {
        self.require_revision(revision)?;
        let selected = selection
            .workset
            .plots()
            .iter()
            .map(|resident| {
                self.plots
                    .iter()
                    .find(|candidate| {
                        candidate.source_document_id == resident.source_document_id
                            && candidate.checked_plot_id == resident.checked_plot_id
                    })
                    .cloned()
                    .ok_or_else(|| {
                        "Crèche selected a plot outside current reviewed inventory".into()
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
