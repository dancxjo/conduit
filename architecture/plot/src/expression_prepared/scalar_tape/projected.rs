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
    checked: ParentCheckedProjectedExpression,
}
/// Immutable Make-emitted code and operand metadata, retained once in flash.
/// This is a program description, not a validation or execution witness.
pub struct StaticProjectedProgram {
    pub maximum_input_bytes: usize,
    pub output: PrimitiveInfoKind,
    pub instructions: &'static [ScalarTapeInstruction],
    pub result: usize,
    pub inputs: &'static [StaticProjectedInput],
    pub output_contract: Option<&'static conduit_core::FixedIntegerContractSpec<'static>>,
}
impl StaticProjectedProgram {
    fn tape(&self) -> ScalarTape {
        ScalarTape {
            instructions: Cow::Borrowed(self.instructions),
            result: self.result,
            scratch: ScratchProfile::prepare(self.instructions.len()),
        }
    }
}
/// Retained only after constructor checks, with the enclosing parent's exact
/// Native definition matched before its input validator can be discarded.
#[derive(Clone, Copy)]
pub(in crate::expression_prepared) struct ParentCheckedProjectedExpression {
    program: &'static StaticProjectedProgram,
    output_contract: Option<conduit_core::BorrowedFixedIntegerContract<'static>>,
}
impl<const N: usize> StaticProjectedExpression<N> {
    /// Only trusted Make output may pair operands with an exact Native schema.
    /// Every Input instruction requires one ordered, bounded payload descriptor.
    pub fn new(
        native: ResidentNativeValidator<N>,
        program: &'static StaticProjectedProgram,
    ) -> Result<Self, Refusal> {
        let StaticProjectedProgram {
            maximum_input_bytes,
            output,
            instructions,
            result,
            inputs,
            output_contract,
        } = *program;
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
            checked: ParentCheckedProjectedExpression {
                program,
                output_contract,
            },
        })
    }
    pub fn instruction_count(&self) -> usize {
        self.checked.program.instructions.len()
    }
    pub fn scratch_bytes(&self) -> usize {
        self.checked.program.tape().scratch_bytes()
    }
    pub fn evaluate_into(
        &self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, StaticProjectedRefusal> {
        let validated = self.native.validate_borrowed(input)?;
        self.checked.evaluate_checked_input_into(
            crate::expression_prepared::canonical_input::CanonicalInput::Whole(input),
            validated,
            output,
        )
    }
    pub(in crate::expression_prepared) fn input_matches(
        &self,
        native: &ResidentNativeValidator<N>,
    ) -> bool {
        self.native.same_definition(native)
    }
    /// Caller first checks `input_matches` against the retained parent. This
    /// consumes the duplicate validator; it does not skip value validation.
    pub(in crate::expression_prepared) fn into_parent_checked(
        self,
    ) -> ParentCheckedProjectedExpression {
        self.checked
    }
}
impl ParentCheckedProjectedExpression {
    pub(in crate::expression_prepared) fn output_contract(
        &self,
    ) -> Option<conduit_core::BorrowedFixedIntegerContract<'static>> {
        self.output_contract
    }
    /// Private reuse after the parent constructor validated the immutable
    /// program and retained this exact output guard in its checked slot map.
    pub(in crate::expression_prepared) fn from_parent_checked_program(
        program: &'static StaticProjectedProgram,
        output_contract: Option<conduit_core::BorrowedFixedIntegerContract<'static>>,
    ) -> Self {
        Self {
            program,
            output_contract,
        }
    }

    // One cold traversal over immutable operand metadata, shared by children.
    #[inline(never)]
    pub(in crate::expression_prepared) fn checked_work(&self) -> u32 {
        use crate::expression_prepared::resident_work::{projected_work, ProjectedInputWork as W};
        projected_work(
            self.program.instructions.len(),
            self.program.inputs.iter().map(|input| match input.source {
                StaticProjectedSource::Fixed { .. } => W::Fixed,
                StaticProjectedSource::Path(path) => W::Path(path.len()),
                StaticProjectedSource::VariantIs { path, .. } => W::VariantIs(path.len()),
                StaticProjectedSource::Length(path) => W::Length(path.len()),
            }),
            self.output_contract.map_or(0, |c| c.validation_work()),
        )
    }
    // Private reuse only after the enclosing evaluator checks its whole input.
    // Constructor-time definition equality preserves every child guard/bound.
    pub(in crate::expression_prepared) fn evaluate_checked_input_into(
        &self,
        input: crate::expression_prepared::canonical_input::CanonicalInput<'_>,
        validated: conduit_core::ValidatedCanonicalStructuredValue<'_>,
        output: &mut [u8],
    ) -> Result<usize, StaticProjectedRefusal> {
        if input
            .len()
            .is_none_or(|n| n > self.program.maximum_input_bytes)
        {
            return Err(Refusal::InvalidInput.into());
        }
        let length = static_image::width(self.program.output).ok_or(Refusal::InvalidProgram)?;
        if output.len() < length {
            return Err(Refusal::InvalidProgram.into());
        }
        let tape = self.program.tape();
        let value = tape.evaluate_with_inputs(&|pc, kind| {
            let index = self
                .program
                .inputs
                .binary_search_by_key(&pc, |i| i.instruction)
                .map_err(|_| Refusal::InvalidProgram)?;
            let location = &self.program.inputs[index];
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
        if value.kind != self.program.output {
            return Err(Refusal::InvalidProgram.into());
        }
        static_image::validate_leaf(self.program.output, value.as_slice())?;
        if let Some(contract) = self.output_contract {
            contract
                .validate(value.as_slice())
                .map_err(Refusal::OutputValueConstraint)?;
        }
        output[..length].copy_from_slice(value.as_slice());
        Ok(length)
    }
}
