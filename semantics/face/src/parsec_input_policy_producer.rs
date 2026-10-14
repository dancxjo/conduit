//! Prepared checked-source proposals, never current input or retained Body authority.
use crate::PreparedCheckedSourcePlan;
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, ConfigurationValue, PlanId, PlotIdentity,
    PreparedStructuredComposer, ValidatedCanonicalStructuredValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CheckedNativeType, CheckedSyntaxDocument, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog, PURE_EXPRESSION_REVISION,
};

pub const PARSEC_INPUT_POLICY_SOURCE: &str =
    include_str!("../components/parsec-input-policy.conduit");
/// Prepared storage bound, distinct from the existing unannotated source Fore.
pub const PARSEC_INPUT_POLICY_FRAME_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsecInputPolicyRefusal {
    Source,
    Shape,
    Preparation,
    State,
    Context,
    Evaluation,
    Output,
}

/// Import the actual owner's checked contracts; shape-only imports lose laws.
pub fn checked_parsec_input_policy() -> Result<CheckedSyntaxDocument, ParsecInputPolicyRefusal> {
    check_syntax_document(
        &parse_syntax_document(PARSEC_INPUT_POLICY_SOURCE),
        &parsec_input_policy_catalog()?,
    )
    .map_err(|_| ParsecInputPolicyRefusal::Source)
}

/// Exact generated owner metadata for checking and planning policy source.
/// Catalog construction provides no current input or execution authority.
pub fn parsec_input_policy_catalog() -> Result<StartupCatalog, ParsecInputPolicyRefusal> {
    let context = crate::generated::action_input_binding_checked_type()
        .map_err(|_| ParsecInputPolicyRefusal::Preparation)?;
    let affordance = crate::generated::generated_action_affordance_checked_type()
        .map_err(|_| ParsecInputPolicyRefusal::Preparation)?;
    let mut startup = StartupCatalog::new();
    // The policy constructs this nested native record. Preserve its own exact
    // laws as well as the containing proposal's contracts during construction.
    startup
        .insert_checked_native_type("GeneratedActionAffordance", &affordance)
        .map_err(|_| ParsecInputPolicyRefusal::Preparation)?;
    startup
        .insert_checked_native_type("ActionInputBindingProposalInfo", &context)
        .map_err(|_| ParsecInputPolicyRefusal::Preparation)?;
    Ok(startup)
}

struct PreparedLaws {
    schema: Vec<u8>,
    native: CheckedNativeType,
}
impl PreparedLaws {
    fn new(native: CheckedNativeType) -> Result<Self, ParsecInputPolicyRefusal> {
        // These exact owner types have field refinements, not whole-value invariants.
        // Refuse future additions until their prepared evaluator is explicitly admitted.
        if !native.invariants.is_empty() {
            return Err(ParsecInputPolicyRefusal::Shape);
        }
        Ok(Self {
            schema: native
                .value_type
                .canonical_bytes()
                .map_err(|_| ParsecInputPolicyRefusal::Preparation)?,
            native,
        })
    }
    fn validate(&self, value: ValidatedCanonicalStructuredValue<'_>) -> Result<(), ()> {
        if value.type_bytes() != self.schema {
            return Err(());
        }
        for law in &self.native.value_contracts {
            let mut field = value;
            for name in law
                .representation_path
                .strip_prefix('.')
                .ok_or(())?
                .split('.')
            {
                field = field.record_field(name).map_err(|_| ())?.ok_or(())?;
            }
            law.contract
                .validate(
                    field
                        .primitive_bytes(law.contract.value_kind.as_str())
                        .map_err(|_| ())?,
                )
                .map_err(|_| ())?;
        }
        Ok(())
    }
}

pub struct PreparedParsecInputPolicy {
    evaluator: PreparedPortableExpressionEvaluator,
    request: PreparedStructuredComposer,
    state_laws: PreparedLaws,
    proposal_laws: PreparedLaws,
    producer_plan: PlanId,
    producer_plot: PlotIdentity,
}

pub struct SourceEvaluatedParsecInputProposal<'a> {
    proposal: &'a [u8],
    producer_plan: &'a PlanId,
    producer_plot: &'a PlotIdentity,
}
impl SourceEvaluatedParsecInputProposal<'_> {
    pub fn proposal_bytes(&self) -> &[u8] {
        self.proposal
    }
    pub fn producer_plan_id(&self) -> &PlanId {
        self.producer_plan
    }
    pub fn producer_plot(&self) -> &PlotIdentity {
        self.producer_plot
    }
}

pub fn prepare_checked_parsec_input_policy(
    source: &PreparedCheckedSourcePlan<'_>,
) -> Result<PreparedParsecInputPolicy, ParsecInputPolicyRefusal> {
    let checked = checked_parsec_input_policy()?;
    let expected = expand_canonical_plot_for_authoring(
        &checked,
        "parsec/input-policy",
        &ProfileCatalog::new(),
    )
    .map_err(|_| ParsecInputPolicyRefusal::Source)?;
    if source.expanded() != &expected {
        return Err(ParsecInputPolicyRefusal::Source);
    }
    let [gear] = expected.expanded.gears.as_slice() else {
        return Err(ParsecInputPolicyRefusal::Shape);
    };
    if gear.kind_contract_revision.as_str() != PURE_EXPRESSION_REVISION {
        return Err(ParsecInputPolicyRefusal::Shape);
    }
    let [field] = gear.configuration.as_slice() else {
        return Err(ParsecInputPolicyRefusal::Shape);
    };
    let ("program", ConfigurationValue::Text(encoded)) = (field.key.as_str(), &field.value) else {
        return Err(ParsecInputPolicyRefusal::Shape);
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|_| ParsecInputPolicyRefusal::Shape)?;
    let context = crate::generated::action_input_binding_checked_type()
        .map_err(|_| ParsecInputPolicyRefusal::Preparation)?;
    let request = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "ParsecInputPolicyRequest")
        .ok_or(ParsecInputPolicyRefusal::Shape)?;
    if program.input_type != request.value_type || program.output_type != context.value_type {
        return Err(ParsecInputPolicyRefusal::Shape);
    }
    let state = crate::parsec_map_transition::parsec_map_state_checked_type()
        .map_err(|_| ParsecInputPolicyRefusal::Preparation)?;
    let expanded = &source.expanded().expanded;
    Ok(PreparedParsecInputPolicy {
        request: PreparedStructuredComposer::new(
            &program.input_type,
            PARSEC_INPUT_POLICY_FRAME_BYTES,
        )
        .map_err(|_| ParsecInputPolicyRefusal::Preparation)?,
        evaluator: PreparedPortableExpressionEvaluator::new(&program)
            .map_err(|_| ParsecInputPolicyRefusal::Preparation)?,
        state_laws: PreparedLaws::new(state)?,
        proposal_laws: PreparedLaws::new(context)?,
        producer_plan: source.plan().plan_id.clone(),
        producer_plot: PlotIdentity {
            source_document_id: expanded.source_document_id.clone(),
            checked_plot_id: expanded.checked_plot_id.clone(),
            expanded_plot_id: expanded.expanded_plot_id.clone(),
        },
    })
}

impl PreparedParsecInputPolicy {
    /// Evaluates supplied canonical values only. The caller must independently
    /// establish current retained state, context revision, accepted input origin,
    /// and Face/Show custody before using a proposal as an instruction.
    pub fn evaluate<'a>(
        &'a mut self,
        state: &[u8],
        context: &[u8],
    ) -> Result<SourceEvaluatedParsecInputProposal<'a>, ParsecInputPolicyRefusal> {
        if state.len() > PARSEC_INPUT_POLICY_FRAME_BYTES {
            return Err(ParsecInputPolicyRefusal::State);
        }
        if context.len() > PARSEC_INPUT_POLICY_FRAME_BYTES {
            return Err(ParsecInputPolicyRefusal::Context);
        }
        let state = validate_canonical_structured_value(state)
            .map_err(|_| ParsecInputPolicyRefusal::State)?;
        self.state_laws
            .validate(state)
            .map_err(|_| ParsecInputPolicyRefusal::State)?;
        let context = validate_canonical_structured_value(context)
            .map_err(|_| ParsecInputPolicyRefusal::Context)?;
        self.proposal_laws
            .validate(context)
            .map_err(|_| ParsecInputPolicyRefusal::Context)?;
        let field = |name| {
            state
                .record_field(name)
                .map_err(|_| ParsecInputPolicyRefusal::State)?
                .ok_or(ParsecInputPolicyRefusal::State)
        };
        let request = self
            .request
            .record(&[context, field("keyboard")?, field("started")?])
            .map_err(|_| ParsecInputPolicyRefusal::Evaluation)?;
        let proposal = self
            .evaluator
            .evaluate(request)
            .map_err(|_| ParsecInputPolicyRefusal::Evaluation)?;
        if proposal.len() > PARSEC_INPUT_POLICY_FRAME_BYTES {
            return Err(ParsecInputPolicyRefusal::Output);
        }
        let output = validate_canonical_structured_value(proposal)
            .map_err(|_| ParsecInputPolicyRefusal::Output)?;
        self.proposal_laws
            .validate(output)
            .map_err(|_| ParsecInputPolicyRefusal::Output)?;
        Ok(SourceEvaluatedParsecInputProposal {
            proposal,
            producer_plan: &self.producer_plan,
            producer_plot: &self.producer_plot,
        })
    }
    pub fn prepared_buffer_bytes(&self) -> usize {
        self.request.prepared_heap_bytes() + self.evaluator.output_capacity()
    }
}
