//! Source-evaluated proposals, not binding witnesses or Host execution receipts.
use crate::{ActionInputBindingProposalInfo, PreparedCheckedSourcePlan};
use conduit_core::{ConfigurationValue, PlanId, PlotIdentity, StructuredInfoValue};
use conduit_plot::{
    rust_binding::NativeRustBinding, PortableExpressionProgram, PURE_EXPRESSION_REVISION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputBindingProducerRefusal {
    Context,
    ProducerShape,
    Program,
    Output,
}

pub struct SourceProducedInputBindingProposal {
    proposal: ActionInputBindingProposalInfo,
    producer_plan_id: PlanId,
    producer_plot: PlotIdentity,
}
impl SourceProducedInputBindingProposal {
    pub fn producer_plot(&self) -> &PlotIdentity {
        &self.producer_plot
    }
    pub fn proposal(&self) -> &ActionInputBindingProposalInfo {
        &self.proposal
    }
    pub fn producer_plan_id(&self) -> &PlanId {
        &self.producer_plan_id
    }
}

/// Evaluate the exact checked pure producer with a canonically validated context.
/// Does not reserve resources, observe a controller, acknowledge a Show, or
/// establish current binding availability. Raw context fields confer no authority.
pub fn produce_checked_input_binding_proposal(
    source: &PreparedCheckedSourcePlan<'_>,
    context: ActionInputBindingProposalInfo,
) -> Result<SourceProducedInputBindingProposal, InputBindingProducerRefusal> {
    let context = context
        .into_structured()
        .map_err(|_| InputBindingProducerRefusal::Context)?;
    let context = ActionInputBindingProposalInfo::from_structured(context)
        .map_err(|_| InputBindingProducerRefusal::Context)?;
    let gears = &source.expanded().expanded.gears;
    if gears.len() != 1 || gears[0].kind_contract_revision.as_str() != PURE_EXPRESSION_REVISION {
        return Err(InputBindingProducerRefusal::ProducerShape);
    }
    let fields = &gears[0].configuration;
    let mut programs = fields.iter().filter(|field| field.key == "program");
    let field = programs
        .next()
        .ok_or(InputBindingProducerRefusal::Program)?;
    if programs.next().is_some() {
        return Err(InputBindingProducerRefusal::Program);
    }
    let ConfigurationValue::Text(encoded) = &field.value else {
        return Err(InputBindingProducerRefusal::Program);
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|_| InputBindingProducerRefusal::Program)?;
    let expected = ActionInputBindingProposalInfo::semantic_type()
        .map_err(|_| InputBindingProducerRefusal::ProducerShape)?;
    if program.input_type != expected || program.output_type != expected {
        return Err(InputBindingProducerRefusal::ProducerShape);
    }
    let input = context
        .into_structured()
        .map_err(|_| InputBindingProducerRefusal::Context)?
        .canonical_bytes()
        .map_err(|_| InputBindingProducerRefusal::Context)?;
    let output = program
        .evaluate(&input)
        .map_err(|_| InputBindingProducerRefusal::Output)?;
    let value = StructuredInfoValue::from_canonical_bytes(&output)
        .map_err(|_| InputBindingProducerRefusal::Output)?;
    let proposal = ActionInputBindingProposalInfo::from_structured(value)
        .map_err(|_| InputBindingProducerRefusal::Output)?;
    Ok(SourceProducedInputBindingProposal {
        proposal,
        producer_plan_id: source.plan().plan_id.clone(),
        producer_plot: PlotIdentity {
            source_document_id: source.expanded().expanded.source_document_id.clone(),
            checked_plot_id: source.expanded().expanded.checked_plot_id.clone(),
            expanded_plot_id: source.expanded().expanded.expanded_plot_id.clone(),
        },
    })
}
