//! Retained facts from actual Source forwarding and supplementary effect ACK.
//! This serializable observation is not a current-input witness or replay grant.
use conduit_core::{PlanId, PlotIdentity};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ParsecInstructionEffectObservation {
    pub(crate) canonical_instruction: Vec<u8>,
    pub(crate) instruction_producer_plan: PlanId,
    pub(crate) instruction_producer_plot: PlotIdentity,
    pub(crate) source_node: u16,
    pub(crate) source_call: u16,
    pub(crate) source_request: u32,
    pub(crate) value_slot: u16,
    pub(crate) value_generation: u16,
    pub(crate) value_bytes: u32,
    pub(crate) effect_node: u16,
    pub(crate) effect_call: u16,
    pub(crate) effect_request: u32,
}
impl ParsecInstructionEffectObservation {
    pub fn canonical_instruction(&self) -> &[u8] {
        &self.canonical_instruction
    }
    pub fn instruction_producer_plan(&self) -> &PlanId {
        &self.instruction_producer_plan
    }
    pub fn instruction_producer_plot(&self) -> &PlotIdentity {
        &self.instruction_producer_plot
    }
}
