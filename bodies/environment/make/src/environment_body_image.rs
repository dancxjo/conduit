//! Select the actual checked environmental Sources for build-side birth.
use crate::{
    BodyImageRefusal, DisplayOutputSource, NativeSoftwareSourceRecipe, PreparedBodyImage,
    SevenSegmentMaskSource, SourceRecipeRefusal,
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
    /// Checked expanded identity of the explicitly selected output or ordinary Mask.
    pub display: ExpandedPlotId,
    pub sampling_software: NativeSoftwareSourceRecipe,
}

impl EnvironmentBodyImage {
    /// Use the construction host's actual Sign as the catalogue seed and body
    /// birth attribution. No name or UUID is supplied by firmware or regenerated
    /// at MCU boot. This checks canonical identity, not remote authentication.
    pub fn prepare_from_sign(sign: &SignIdentity) -> Result<Self, EnvironmentBodyImageRefusal> {
        let (uuid, sequence, birth_sign) = construction_seed(sign)?;
        Self::prepare(uuid, sequence, birth_sign)
    }

    /// Birth sampler plus the true ordinary three-node seven-segment Mask.
    /// The attributable Sign and catalogue seed follow the same construction
    /// boundary as the existing raw-output profile; no runtime owner is implied.
    pub fn prepare_mask_from_sign(
        sign: &SignIdentity,
    ) -> Result<Self, EnvironmentBodyImageRefusal> {
        let (uuid, sequence, birth_sign) = construction_seed(sign)?;
        Self::prepare_mask(uuid, sequence, birth_sign)
    }

    pub fn prepare(
        construction_uuid: String,
        birth_sequence: u64,
        birth_sign: SignId,
    ) -> Result<Self, EnvironmentBodyImageRefusal> {
        Self::prepare_profile(construction_uuid, birth_sequence, birth_sign, false)
    }

    /// Explicit ordinary Mask construction; the default API remains raw output.
    pub fn prepare_mask(
        construction_uuid: String,
        birth_sequence: u64,
        birth_sign: SignId,
    ) -> Result<Self, EnvironmentBodyImageRefusal> {
        Self::prepare_profile(construction_uuid, birth_sequence, birth_sign, true)
    }

    fn prepare_profile(
        construction_uuid: String,
        birth_sequence: u64,
        birth_sign: SignId,
        ordinary_mask: bool,
    ) -> Result<Self, EnvironmentBodyImageRefusal> {
        use EnvironmentBodyImageRefusal as E;
        let sampling =
            conduit_environment_runtime::construction::compile_default().map_err(E::Sampling)?;
        let (display, title) = if ordinary_mask {
            let source = SevenSegmentMaskSource::compile().map_err(E::Display)?;
            (source.plot.expanded, "Seven-segment Mask")
        } else {
            let source = DisplayOutputSource::compile().map_err(E::Display)?;
            (source.plot.expanded, "Seven-segment output")
        };
        let sampling_software =
            NativeSoftwareSourceRecipe::prepare(&sampling).map_err(E::Software)?;
        let choices = [
            ("Environmental sampling", &sampling.expanded),
            (title, &display),
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
            display: display.expanded_plot_id,
            sampling_software,
        })
    }
}

/// Validate the exact actual construction Sign before deriving naming metadata.
/// The same seed and attribution policy apply to both checked workload choices.
fn construction_seed(
    sign: &SignIdentity,
) -> Result<(String, u64, SignId), EnvironmentBodyImageRefusal> {
    let canonical = conduit_core::bind_sign(
        &sign.host_id,
        &sign.boot_id,
        sign.active_play_id.as_ref(),
        sign.sequence,
    );
    if canonical != *sign {
        return Err(EnvironmentBodyImageRefusal::BirthSign);
    }
    let text = sign.sign_id.as_str();
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &text[..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    );
    Ok((uuid, sign.sequence, sign.sign_id.clone()))
}
