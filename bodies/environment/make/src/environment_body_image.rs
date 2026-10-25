//! Select the actual checked environmental Sources for build-side birth.
use crate::{BodyImageRefusal, DisplayOutputSource, PreparedBodyImage};
use conduit_birth_plot::BirthPlotChoice;
use conduit_body::ResidentPlot;
use conduit_core::{ExpandedPlotId, SignId};

#[derive(Debug)]
pub enum EnvironmentBodyImageRefusal {
    Sampling(String),
    Display(String),
    Birth(BodyImageRefusal),
}

/// Build material retains both expanded identities for subsequent linking.
/// It carries no current Host/Boot, executable Plan or provider authority.
pub struct EnvironmentBodyImage {
    pub body: PreparedBodyImage,
    pub sampling: ExpandedPlotId,
    pub display: ExpandedPlotId,
}

impl EnvironmentBodyImage {
    pub fn prepare(
        construction_uuid: String,
        birth_sequence: u64,
        birth_sign: SignId,
    ) -> Result<Self, EnvironmentBodyImageRefusal> {
        use EnvironmentBodyImageRefusal as E;
        let sampling =
            conduit_environment_runtime::construction::compile_default().map_err(E::Sampling)?;
        let display = DisplayOutputSource::compile().map_err(E::Display)?;
        let choices = [
            ("Environmental sampling", &sampling.expanded),
            ("Seven-segment output", &display.plot.expanded),
        ]
        .into_iter()
        .map(|(title, plot)| BirthPlotChoice {
            title: title.into(),
            search_text: title.into(),
            plot: ResidentPlot::new(
                plot.source_document_id.clone(),
                plot.checked_plot_id.clone(),
            ),
            selected: true,
            refusal: None,
        })
        .collect();
        let body =
            PreparedBodyImage::prepare(construction_uuid, choices, birth_sequence, birth_sign)
                .map_err(E::Birth)?;
        Ok(Self {
            body,
            sampling: sampling.expanded.expanded_plot_id,
            display: display.plot.expanded.expanded_plot_id,
        })
    }
}
