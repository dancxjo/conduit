//! Select the actual checked environmental Sources for build-side birth.
use crate::{
    BodyImageRefusal, DisplayOutputSource, NativeSoftwareSourceRecipe, PreparedBodyImage,
    SourceRecipeRefusal,
};
use conduit_birth_plot::BirthPlotChoice;
use conduit_body::ResidentPlot;
use conduit_core::{ExpandedPlotId, SignId, SignIdentity};

#[derive(Debug)]
pub enum EnvironmentBodyImageRefusal {
    BirthSign,
    Sampling(String),
    Display(String),
    Birth(BodyImageRefusal),
    Software(SourceRecipeRefusal),
}

/// Build material retains both expanded identities for subsequent linking.
/// It carries no current Host/Boot, executable Plan or provider authority.
pub struct EnvironmentBodyImage {
    pub body: PreparedBodyImage,
    pub sampling: ExpandedPlotId,
    pub display: ExpandedPlotId,
    pub sampling_software: NativeSoftwareSourceRecipe,
}

impl EnvironmentBodyImage {
    /// Use the construction host's actual Sign as the catalogue seed and body
    /// birth attribution. No name or UUID is supplied by firmware or regenerated
    /// at MCU boot. This checks canonical identity, not remote authentication.
    pub fn prepare_from_sign(sign: &SignIdentity) -> Result<Self, EnvironmentBodyImageRefusal> {
        let canonical = conduit_core::bind_sign(
            &sign.host_id,
            &sign.boot_id,
            sign.active_play_id.as_ref(),
            sign.sequence,
        );
        if canonical != *sign {
            return Err(EnvironmentBodyImageRefusal::BirthSign);
        }
        // A UUID-shaped catalogue encounter seed derived from the exact birth
        // identity. It is metadata, never an admission credential or entropy.
        let text = sign.sign_id.as_str();
        let uuid = format!(
            "{}-{}-{}-{}-{}",
            &text[..8],
            &text[8..12],
            &text[12..16],
            &text[16..20],
            &text[20..32],
        );
        Self::prepare(uuid, sign.sequence, sign.sign_id.clone())
    }

    pub fn prepare(
        construction_uuid: String,
        birth_sequence: u64,
        birth_sign: SignId,
    ) -> Result<Self, EnvironmentBodyImageRefusal> {
        use EnvironmentBodyImageRefusal as E;
        let sampling =
            conduit_environment_runtime::construction::compile_default().map_err(E::Sampling)?;
        let display = DisplayOutputSource::compile().map_err(E::Display)?;
        let sampling_software =
            NativeSoftwareSourceRecipe::prepare(&sampling).map_err(E::Software)?;
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
            sampling_software,
        })
    }
}
