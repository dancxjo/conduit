//! Resident typed operands use the ordinary scalar loop and Native checker.
use super::*;
use crate::rust_binding::{PreparedNativeRefusal, ResidentNativeValidator};
pub(in crate::expression_prepared) mod generate;
pub use generate::{generate_static_projected_expression, StaticProjectedGenerationRefusal};

/// Make-selected operand over a separately validated exact Native input.
#[derive(Clone, Copy, Debug)]
pub struct StaticProjectedInput {
    pub instruction: usize,
    pub source: StaticProjectedSource,
}
#[derive(Clone, Copy, Debug)]
pub enum StaticProjectedSource {
    Fixed {
        start: usize,
        length: usize,
    },
    Path(&'static [crate::StaticExpressionPathStep]),
    VariantIs {
        path: &'static [crate::StaticExpressionPathStep],
        tag: &'static str,
    },
    Length(&'static [crate::StaticExpressionPathStep]),
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
    /// Only trusted Make output may pair operands with an exact Native schema.
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
        if (maximum_input_bytes == 0 && !native.is_intrinsic_empty())
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
            let valid = match input.source {
                StaticProjectedSource::Fixed { start, length } => {
                    static_image::width(instruction.kind) == Some(length)
                        && start
                            .checked_add(length)
                            .is_some_and(|end| end <= maximum_input_bytes)
                }
                StaticProjectedSource::Path(path) => super::super::resident_projection::valid(path),
                StaticProjectedSource::VariantIs { path, tag } => {
                    instruction.kind == PrimitiveInfoKind::Bool
                        && !tag.is_empty()
                        && tag.len() <= conduit_core::MAXIMUM_STRUCTURED_NAME_BYTES
                        && super::super::resident_projection::valid(path)
                }
                StaticProjectedSource::Length(path) => {
                    instruction.kind == PrimitiveInfoKind::U64
                        && super::super::resident_projection::valid(path)
                }
            };
            if !matches!(instruction.operation, Operation::Input) || !valid {
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
        let validated = self.native.validate_borrowed(input)?;
        self.evaluate_checked_into(input, validated, output)
    }
    pub(in crate::expression_prepared) fn input_matches(
        &self,
        native: &ResidentNativeValidator<N>,
    ) -> bool {
        self.native.same_definition(native)
    }
    pub(in crate::expression_prepared) fn checked_work(&self) -> u32 {
        let mut work = (self.tape.instruction_count() as u32).saturating_add(4);
        for operand in self.inputs {
            let units = match operand.source {
                StaticProjectedSource::Fixed { .. } => 2,
                StaticProjectedSource::Path(path) => path.len() + 2,
                StaticProjectedSource::VariantIs { path, .. } => path.len() + 3,
                StaticProjectedSource::Length(path) => path.len() + 2,
            };
            work = work.saturating_add(units as u32);
        }
        work.saturating_add(self.output_contract.map_or(0, |c| c.validation_work()) as u32)
    }
    // Private reuse only after the enclosing evaluator checks its whole input.
    // Constructor-time definition equality preserves every child guard/bound.
    pub(in crate::expression_prepared) fn evaluate_checked_into(
        &self,
        input: &[u8],
        validated: conduit_core::ValidatedCanonicalStructuredValue<'_>,
        output: &mut [u8],
    ) -> Result<usize, StaticProjectedRefusal> {
        self.evaluate_checked_input_into(
            crate::expression_prepared::canonical_input::CanonicalInput::Whole(input),
            validated,
            output,
        )
    }
    pub(in crate::expression_prepared) fn evaluate_checked_input_into(
        &self,
        input: crate::expression_prepared::canonical_input::CanonicalInput<'_>,
        validated: conduit_core::ValidatedCanonicalStructuredValue<'_>,
        output: &mut [u8],
    ) -> Result<usize, StaticProjectedRefusal> {
        if input.len().is_none_or(|n| n > self.maximum_input_bytes) {
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
            use super::super::resident_projection::select;
            match location.source {
                StaticProjectedSource::Fixed { start, length } => {
                    let bytes = input
                        .get(start..start + length)
                        .ok_or(Refusal::InvalidInput)?;
                    static_image::validate_leaf(kind, bytes).map_err(|_| Refusal::InvalidInput)?;
                    PrimitiveValue::borrowed(kind, bytes)
                }
                StaticProjectedSource::Path(path) => {
                    let bytes = select(validated, path)?
                        .primitive_representation_bytes(kind_name(kind))
                        .map_err(|_| Refusal::InvalidInput)?;
                    static_image::validate_leaf(kind, bytes).map_err(|_| Refusal::InvalidInput)?;
                    PrimitiveValue::borrowed(kind, bytes)
                }
                StaticProjectedSource::VariantIs { path, tag } => PrimitiveValue::new(
                    PrimitiveInfoKind::Bool,
                    &[u8::from(
                        select(validated, path)?
                            .representation()
                            .map_err(|_| Refusal::InvalidInput)?
                            .variant_tag()
                            .map_err(|_| Refusal::InvalidInput)?
                            == tag,
                    )],
                ),
                StaticProjectedSource::Length(path) => PrimitiveValue::new(
                    PrimitiveInfoKind::U64,
                    &u64::from(
                        select(validated, path)?
                            .collection_length()
                            .map_err(|_| Refusal::InvalidInput)?,
                    )
                    .to_le_bytes(),
                ),
            }
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
