//! Resident fixed projections use the ordinary scalar loop and Native checker.
use super::*;
use crate::rust_binding::{PreparedNativeRefusal, ResidentNativeValidator};
mod generate;
pub use generate::{generate_static_projected_expression, StaticProjectedGenerationRefusal};

/// Make-selected raw payload geometry in a separately validated exact input.
/// This is executable image data, not a Source alias or incoming authority.
#[derive(Clone, Copy, Debug)]
pub struct StaticProjectedInput {
    pub instruction: usize,
    pub start: usize,
    pub length: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticProjectedRefusal {
    Native(PreparedNativeRefusal),
    Expression(Refusal),
}
impl From<Refusal> for StaticProjectedRefusal {
    fn from(value: Refusal) -> Self {
        Self::Expression(value)
    }
}
impl From<PreparedNativeRefusal> for StaticProjectedRefusal {
    fn from(value: PreparedNativeRefusal) -> Self {
        Self::Native(value)
    }
}
/// Fixed-scalar output from a checked structured input, without owned execution
/// metadata or per-field evaluators. The installing Root still binds the exact
/// checked image and admits complete state, traversal, stack and mandatory work.
/// This validates values; it neither creates a Native admission witness nor
/// installs an offer, reservation or Play.
pub struct StaticProjectedExpression<const CONTRACTS: usize> {
    native: ResidentNativeValidator<CONTRACTS>,
    tape: ScalarTape,
    inputs: &'static [StaticProjectedInput],
    output: PrimitiveInfoKind,
    maximum_input_bytes: usize,
    output_contract: Option<conduit_core::BorrowedFixedIntegerContract<'static>>,
}
impl<const N: usize> StaticProjectedExpression<N> {
    /// Only trusted Make output may pair offsets with an exact Native schema.
    /// Every Input instruction requires one ordered, bounded payload descriptor.
    pub fn new(
        native: ResidentNativeValidator<N>,
        maximum_input_bytes: usize,
        output: PrimitiveInfoKind,
        instructions: &'static [ScalarTapeInstruction],
        result: usize,
        inputs: &'static [StaticProjectedInput],
        output_contract: Option<&'static conduit_core::FixedIntegerContractSpec<'static>>,
    ) -> Result<Self, Refusal> {
        if maximum_input_bytes == 0
            || maximum_input_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || inputs.len() > MAXIMUM_INSTRUCTIONS
            || inputs
                .windows(2)
                .any(|pair| pair[0].instruction >= pair[1].instruction)
        {
            return Err(Refusal::InvalidProgram);
        }
        for input in inputs {
            let instruction = instructions
                .get(input.instruction)
                .ok_or(Refusal::InvalidProgram)?;
            if !matches!(instruction.operation, Operation::Input)
                || static_image::width(instruction.kind) != Some(input.length)
                || input
                    .start
                    .checked_add(input.length)
                    .is_none_or(|end| end > maximum_input_bytes)
            {
                return Err(Refusal::InvalidProgram);
            }
        }
        static_image::validate_with_inputs(output, instructions, result, |pc, _| {
            inputs.binary_search_by_key(&pc, |i| i.instruction).is_ok()
        })?;
        let output_contract = output_contract
            .map(|spec| {
                if conduit_core::primitive_info_kind(spec.value_kind) != Some(output) {
                    return Err(Refusal::InvalidProgram);
                }
                conduit_core::BorrowedFixedIntegerContract::new(spec)
                    .map_err(|_| Refusal::InvalidProgram)
            })
            .transpose()?;
        Ok(Self {
            native,
            tape: ScalarTape {
                instructions: Cow::Borrowed(instructions),
                result,
                scratch: ScratchProfile::prepare(instructions.len()),
            },
            inputs,
            output,
            maximum_input_bytes,
            output_contract,
        })
    }
    pub fn instruction_count(&self) -> usize {
        self.tape.instruction_count()
    }
    pub fn scratch_bytes(&self) -> usize {
        self.tape.scratch_bytes()
    }
    pub fn evaluate_into(
        &self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, StaticProjectedRefusal> {
        self.native.validate(input)?;
        if input.len() > self.maximum_input_bytes {
            return Err(Refusal::InvalidInput.into());
        }
        let length = static_image::width(self.output).ok_or(Refusal::InvalidProgram)?;
        if output.len() < length {
            return Err(Refusal::InvalidProgram.into());
        }
        let value = self.tape.evaluate_with_inputs(&|pc, kind| {
            let index = self
                .inputs
                .binary_search_by_key(&pc, |i| i.instruction)
                .map_err(|_| Refusal::InvalidProgram)?;
            let location = &self.inputs[index];
            let bytes = input
                .get(location.start..location.start + location.length)
                .ok_or(Refusal::InvalidInput)?;
            static_image::validate_leaf(kind, bytes).map_err(|_| Refusal::InvalidInput)?;
            PrimitiveValue::borrowed(kind, bytes)
        })?;
        if value.kind != self.output {
            return Err(Refusal::InvalidProgram.into());
        }
        static_image::validate_leaf(self.output, value.as_slice())?;
        if let Some(contract) = self.output_contract {
            contract
                .validate(value.as_slice())
                .map_err(Refusal::OutputValueConstraint)?;
        }
        output[..length].copy_from_slice(value.as_slice());
        Ok(length)
    }
}
