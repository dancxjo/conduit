//! Read-only observation of an actual private input instruction effect.
//! Serialization does not grant current binding, input or other Mask authority.
use conduit_body::BodyPlayIdentity;
use conduit_core::{PlanId, PlotIdentity, PreparationHostIdentity};
use conduit_presentation::{ActionInputBindingProposalInfo, MaskShow, Presentation};

/// Created only by the installed private owner after exact current scope,
/// source-policy correlation and successful terminal write/flush. This is
/// intentionally Serialize-only: replayed artifacts cannot enter runtime admission.
#[derive(Debug, serde::Serialize)]
pub struct ParsecInputInstructionObservation {
    pub(crate) proof_class: &'static str,
    pub(crate) face: Presentation,
    pub(crate) show: MaskShow,
    pub(crate) body_play: BodyPlayIdentity,
    pub(crate) host: PreparationHostIdentity,
    pub(crate) binding: ActionInputBindingProposalInfo,
    pub(crate) producer_plan_id: PlanId,
    pub(crate) producer_plot: PlotIdentity,
    pub(crate) prompt_text: String,
    pub(crate) prompt_text_sha256: [u8; 32],
    pub(crate) input_view_epoch: u64,
    pub(crate) effect: super::ParsecInstructionEffectObservation,
}
impl ParsecInputInstructionObservation {
    pub fn effect(&self) -> &super::ParsecInstructionEffectObservation {
        &self.effect
    }
    pub fn face(&self) -> &Presentation {
        &self.face
    }
    pub fn show(&self) -> &MaskShow {
        &self.show
    }
    pub fn binding(&self) -> &ActionInputBindingProposalInfo {
        &self.binding
    }
    pub fn prompt_text(&self) -> &str {
        &self.prompt_text
    }
    pub fn input_view_epoch(&self) -> u64 {
        self.input_view_epoch
    }
}
