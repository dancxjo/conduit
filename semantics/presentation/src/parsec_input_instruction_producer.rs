//! Checked component instruction candidates, never current binding or Show grants.
use crate::PreparedCheckedSourcePlan;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, validate_canonical_structured_value, ConfigurationValue, PlanId, PlotIdentity,
    PreparedStructuredComposer, StructuredInfoType, StructuredInfoValue,
    ValidatedCanonicalStructuredValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CheckedNativeType, CheckedSyntaxDocument, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog, PURE_EXPRESSION_REVISION,
};

pub const PARSEC_INPUT_INSTRUCTION_SOURCE: &str =
    include_str!("../components/parsec-input-instruction.conduit");
/// Prepared frame/queue bound, independent of the unannotated source Fore.
pub const PARSEC_INPUT_INSTRUCTION_FRAME_BYTES: usize = 4096;
pub const PARSEC_INPUT_INSTRUCTION_WORDING_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsecInputInstructionLook {
    Arcade,
    Linear,
    Spoken,
}
impl ParsecInputInstructionLook {
    pub fn parse(value: &str) -> Result<Self, ParsecInputInstructionRefusal> {
        match value {
            "arcade" => Ok(Self::Arcade),
            "linear" => Ok(Self::Linear),
            "spoken" => Ok(Self::Spoken),
            _ => Err(ParsecInputInstructionRefusal::Request),
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Arcade => 0,
            Self::Linear => 1,
            Self::Spoken => 2,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsecInputInstructionRefusal {
    Source,
    Shape,
    Preparation,
    State,
    Binding,
    Request,
    Evaluation,
    Correlation,
    Output,
}
use ParsecInputInstructionRefusal as Refusal;

/// Preparation metadata only. Import original owners rather than reconstructing laws.
pub fn parsec_input_instruction_catalog() -> Result<StartupCatalog, Refusal> {
    let mut catalog = crate::parsec_input_policy_catalog().map_err(|_| Refusal::Preparation)?;
    let state = crate::parsec_map_transition::parsec_map_state_checked_type()
        .map_err(|_| Refusal::Preparation)?;
    catalog
        .insert_checked_native_type("ParsecMapState", &state)
        .map_err(|_| Refusal::Preparation)?;
    Ok(catalog)
}
pub fn checked_parsec_input_instruction() -> Result<CheckedSyntaxDocument, Refusal> {
    check_syntax_document(
        &parse_syntax_document(PARSEC_INPUT_INSTRUCTION_SOURCE),
        &parsec_input_instruction_catalog()?,
    )
    .map_err(|_| Refusal::Source)
}

struct PreparedLaws {
    schema: Vec<u8>,
    native: CheckedNativeType,
}
impl PreparedLaws {
    fn new(native: CheckedNativeType) -> Result<Self, Refusal> {
        // Whole-value invariants require their own prepared evaluator; do not silently skip them.
        if !native.invariants.is_empty() {
            return Err(Refusal::Shape);
        }
        Ok(Self {
            schema: native
                .value_type
                .canonical_bytes()
                .map_err(|_| Refusal::Preparation)?,
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

pub struct PreparedParsecInputInstruction {
    evaluator: PreparedPortableExpressionEvaluator,
    request: PreparedStructuredComposer,
    state: PreparedLaws,
    binding: PreparedLaws,
    output: PreparedLaws,
    looks: [Vec<u8>; 3],
    withdrawal: [Vec<u8>; 2],
    wording: [u8; PARSEC_INPUT_INSTRUCTION_WORDING_BYTES],
    producer_plan: PlanId,
    producer_plot: PlotIdentity,
}

/// Borrowed output from this exact checked producer. No current owner or input grant.
/// The source's `valid` bit checks only phase/action/revision. It does not
/// establish keyboard/control correlation or current policy/origin; those checks
/// belong to the private runtime encounter.
/// `binding_bytes` is the supplied canonical frame after exact equality against
/// the binding forwarded through the evaluated output; it is never a rewritten grant.
pub struct SourceEvaluatedParsecInstructionCandidate<'a> {
    canonical: &'a [u8],
    binding: &'a [u8],
    wording: &'a str,
    withdrawal: bool,
    producer_plan: &'a PlanId,
    producer_plot: &'a PlotIdentity,
}
impl SourceEvaluatedParsecInstructionCandidate<'_> {
    pub fn canonical_bytes(&self) -> &[u8] {
        self.canonical
    }
    pub fn binding_bytes(&self) -> &[u8] {
        self.binding
    }
    pub fn wording(&self) -> &str {
        self.wording
    }
    pub fn withdrawal(&self) -> bool {
        self.withdrawal
    }
    pub fn producer_plan_id(&self) -> &PlanId {
        self.producer_plan
    }
    pub fn producer_plot(&self) -> &PlotIdentity {
        self.producer_plot
    }
}

fn leaf(kind: &str, bytes: &[u8]) -> Result<Vec<u8>, Refusal> {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(kind)).map_err(|_| Refusal::Preparation)?,
        bytes.to_vec(),
    )
    .map_err(|_| Refusal::Preparation)?
    .canonical_bytes()
    .map_err(|_| Refusal::Preparation)
}

pub fn prepare_checked_parsec_input_instruction(
    source: &PreparedCheckedSourcePlan<'_>,
) -> Result<PreparedParsecInputInstruction, Refusal> {
    let checked = checked_parsec_input_instruction()?;
    let expected = expand_canonical_plot_for_authoring(
        &checked,
        "parsec/input-instruction",
        &ProfileCatalog::new(),
    )
    .map_err(|_| Refusal::Source)?;
    if source.expanded() != &expected {
        return Err(Refusal::Source);
    }
    let [gear] = expected.expanded.gears.as_slice() else {
        return Err(Refusal::Shape);
    };
    if gear.kind_contract_revision.as_str() != PURE_EXPRESSION_REVISION {
        return Err(Refusal::Shape);
    }
    let [field] = gear.configuration.as_slice() else {
        return Err(Refusal::Shape);
    };
    let ("program", ConfigurationValue::Text(encoded)) = (field.key.as_str(), &field.value) else {
        return Err(Refusal::Shape);
    };
    let program =
        PortableExpressionProgram::from_canonical_hex(encoded).map_err(|_| Refusal::Shape)?;
    let native = |name| {
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .cloned()
            .ok_or(Refusal::Shape)
    };
    let request = native("ParsecInputInstructionRequest")?;
    let output = native("ParsecInputInstructionInfo")?;
    if program.input_type != request.value_type || program.output_type != output.value_type {
        return Err(Refusal::Shape);
    }
    let state = crate::parsec_map_transition::parsec_map_state_checked_type()
        .map_err(|_| Refusal::Preparation)?;
    let binding =
        crate::generated::action_input_binding_checked_type().map_err(|_| Refusal::Preparation)?;
    let expanded = &expected.expanded;
    Ok(PreparedParsecInputInstruction {
        evaluator: PreparedPortableExpressionEvaluator::new(&program)
            .map_err(|_| Refusal::Preparation)?,
        request: PreparedStructuredComposer::new(
            &program.input_type,
            PARSEC_INPUT_INSTRUCTION_FRAME_BYTES,
        )
        .map_err(|_| Refusal::Preparation)?,
        state: PreparedLaws::new(state)?,
        binding: PreparedLaws::new(binding)?,
        output: PreparedLaws::new(output)?,
        looks: [
            leaf(conduit_core::TEXT_INFO_ID, b"arcade")?,
            leaf(conduit_core::TEXT_INFO_ID, b"linear")?,
            leaf(conduit_core::TEXT_INFO_ID, b"spoken")?,
        ],
        withdrawal: [leaf("value/bool", &[0])?, leaf("value/bool", &[1])?],
        wording: [0; PARSEC_INPUT_INSTRUCTION_WORDING_BYTES],
        producer_plan: source.plan().plan_id.clone(),
        producer_plot: PlotIdentity {
            source_document_id: expanded.source_document_id.clone(),
            checked_plot_id: expanded.checked_plot_id.clone(),
            expanded_plot_id: expanded.expanded_plot_id.clone(),
        },
    })
}

fn member<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>, Refusal> {
    value
        .record_field(name)
        .map_err(|_| Refusal::Output)?
        .ok_or(Refusal::Output)
}
fn flag(value: ValidatedCanonicalStructuredValue<'_>) -> Result<bool, Refusal> {
    match value
        .primitive_bytes("value/bool")
        .map_err(|_| Refusal::Output)?
    {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(Refusal::Output),
    }
}

impl PreparedParsecInputInstruction {
    /// Caller values are candidate metadata only, even when source-correlated.
    /// Actual accepted input, acknowledged state and per-Mask Show remain mandatory.
    pub fn evaluate<'a>(
        &'a mut self,
        state: &[u8],
        binding: &'a [u8],
        look: ParsecInputInstructionLook,
        withdrawal: bool,
    ) -> Result<SourceEvaluatedParsecInstructionCandidate<'a>, Refusal> {
        let look_index = look.index();
        self.evaluate_inner(state, binding, look_index, withdrawal)
    }
    fn evaluate_inner<'a>(
        &'a mut self,
        state: &[u8],
        binding_bytes: &'a [u8],
        look_index: usize,
        withdrawal: bool,
    ) -> Result<SourceEvaluatedParsecInstructionCandidate<'a>, Refusal> {
        if state.len() > PARSEC_INPUT_INSTRUCTION_FRAME_BYTES {
            return Err(Refusal::State);
        }
        if binding_bytes.len() > PARSEC_INPUT_INSTRUCTION_FRAME_BYTES {
            return Err(Refusal::Binding);
        }
        let state = validate_canonical_structured_value(state).map_err(|_| Refusal::State)?;
        self.state.validate(state).map_err(|_| Refusal::State)?;
        let binding =
            validate_canonical_structured_value(binding_bytes).map_err(|_| Refusal::Binding)?;
        self.binding
            .validate(binding)
            .map_err(|_| Refusal::Binding)?;
        let look = validate_canonical_structured_value(&self.looks[look_index])
            .map_err(|_| Refusal::Request)?;
        let withdrawal =
            validate_canonical_structured_value(&self.withdrawal[usize::from(withdrawal)])
                .map_err(|_| Refusal::Request)?;
        let request = self
            .request
            .record(&[binding, look, state, withdrawal])
            .map_err(|_| Refusal::Request)?;
        let canonical = self
            .evaluator
            .evaluate(request)
            .map_err(|_| Refusal::Evaluation)?;
        if canonical.len() > PARSEC_INPUT_INSTRUCTION_FRAME_BYTES {
            return Err(Refusal::Output);
        }
        let output = validate_canonical_structured_value(canonical).map_err(|_| Refusal::Output)?;
        self.output.validate(output).map_err(|_| Refusal::Output)?;
        let forwarded = member(output, "binding")?;
        if forwarded.type_bytes() != binding.type_bytes()
            || forwarded.value_node() != binding.value_node()
        {
            return Err(Refusal::Output);
        }
        if !flag(member(output, "valid")?)? {
            return Err(Refusal::Correlation);
        }
        let mut length = 0usize;
        for name in ["a", "b", "c"] {
            let segment = member(output, name)?
                .primitive_bytes(conduit_core::TEXT_INFO_ID)
                .map_err(|_| Refusal::Output)?;
            let end = length
                .checked_add(segment.len())
                .filter(|end| *end <= self.wording.len())
                .ok_or(Refusal::Output)?;
            self.wording[length..end].copy_from_slice(segment);
            length = end;
        }
        let wording = core::str::from_utf8(&self.wording[..length]).map_err(|_| Refusal::Output)?;
        if wording.is_empty() {
            return Err(Refusal::Output);
        }
        Ok(SourceEvaluatedParsecInstructionCandidate {
            canonical,
            binding: binding_bytes,
            wording,
            withdrawal: flag(member(output, "withdrawal")?)?,
            producer_plan: &self.producer_plan,
            producer_plot: &self.producer_plot,
        })
    }
    pub fn prepared_buffer_bytes(&self) -> usize {
        self.request.prepared_heap_bytes()
            + self.evaluator.output_capacity()
            + self.wording.len()
            + self.looks.iter().map(Vec::capacity).sum::<usize>()
            + self.withdrawal.iter().map(Vec::capacity).sum::<usize>()
    }
}
