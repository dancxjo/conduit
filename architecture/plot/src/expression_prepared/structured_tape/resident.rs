//! Resident constructor data; Root independently admits its full realization.
use super::*;
use crate::expression_prepared::{StaticProjectedExpression, StaticProjectedRefusal};
use crate::rust_binding::{PreparedNativeRefusal, ResidentNativeValidator};
#[path = "resident/generate.rs"]
mod generate;
pub use generate::{generate_static_structured_expression, StaticStructuredGenerationRefusal};
pub struct StaticStructuredProgram {
    pub instructions: &'static [StructuredTapeInstruction],
    pub constants: &'static [u8],
    pub maximum_input_bytes: usize,
    pub maximum_output_bytes: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticStructuredRefusal {
    Input(PreparedNativeRefusal),
    Output(PreparedNativeRefusal),
    Child {
        index: u16,
        refusal: StaticProjectedRefusal,
    },
    Expression(Refusal),
}
impl From<Refusal> for StaticStructuredRefusal {
    fn from(value: Refusal) -> Self {
        Self::Expression(value)
    }
}
/// A scalar child or a direct canonical node selected from the checked input.
pub enum StaticStructuredValue<const INPUT: usize> {
    Scalar(StaticProjectedExpression<INPUT>),
    InputPath(&'static [crate::StaticExpressionPathStep]),
}
/// No owned instructions, constants, schemas or result storage. This is a value
/// evaluator, never an offered Back or an execution/native admission witness.
/// The installing Root must bind the exact checked artifact and admit complete
/// state, traversal, child validation, stack, scratch and cooperative work.
pub struct StaticStructuredExpression<
    const INPUT: usize,
    const OUTPUT: usize,
    const CHILDREN: usize,
> {
    input: ResidentNativeValidator<INPUT>,
    output: ResidentNativeValidator<OUTPUT>,
    children: [StaticStructuredValue<INPUT>; CHILDREN],
    program: &'static StaticStructuredProgram,
}
impl<const I: usize, const O: usize, const C: usize> StaticStructuredExpression<I, O, C> {
    /// Data must be trusted Make output, never self-declared incoming authority.
    pub fn new(
        input: ResidentNativeValidator<I>,
        output: ResidentNativeValidator<O>,
        children: [StaticStructuredValue<I>; C],
        program: &'static StaticStructuredProgram,
    ) -> Result<Self, Refusal> {
        if C > MAX_VALUES
            || program.instructions.is_empty()
            || program.instructions.len() > MAX_OPERATIONS
            || program.constants.len() > MAX_CONSTANT_BYTES
            || [program.maximum_input_bytes, program.maximum_output_bytes]
                .iter()
                .any(|n| *n == 0 || *n > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        {
            return Err(Refusal::InvalidProgram);
        }
        for child in &children {
            if let StaticStructuredValue::InputPath(path) = child {
                if !crate::expression_prepared::resident_projection::valid(path) {
                    return Err(Refusal::InvalidProgram);
                }
            }
        }
        for (pc, op) in program.instructions.iter().enumerate() {
            let valid = match *op {
                Operation::Bytes { start, length } => (start as usize)
                    .checked_add(length as usize)
                    .is_some_and(|end| end <= program.constants.len()),
                Operation::InputNode { start, length } => (start as usize)
                    .checked_add(length as usize)
                    .is_some_and(|end| end <= program.maximum_input_bytes),
                Operation::Value(index) => usize::from(index) < C,
                Operation::BranchFalse { value, target } => {
                    usize::from(value) < C
                        && usize::from(target) > pc
                        && usize::from(target) <= program.instructions.len()
                }
                Operation::Jump(target) => {
                    usize::from(target) > pc && usize::from(target) <= program.instructions.len()
                }
            };
            if !valid {
                return Err(Refusal::InvalidProgram);
            }
        }
        Ok(Self {
            input,
            output,
            children,
            program,
        })
    }
    pub fn scratch_bytes(&self) -> usize {
        self.program.maximum_output_bytes
    }
    /// Scratch is separately admitted staging, reused after success or refusal.
    /// No bytes reach `output` until the full constructed Native value passes.
    pub fn evaluate_into(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        let validated = self
            .input
            .validate_borrowed(input)
            .map_err(StaticStructuredRefusal::Input)?;
        if input.len() > self.program.maximum_input_bytes {
            return Err(Refusal::InvalidInput.into());
        }
        if scratch.len() < self.program.maximum_output_bytes {
            return Err(Refusal::InvalidProgram.into());
        }
        let mut staged = SliceOutput {
            bytes: &mut scratch[..self.program.maximum_output_bytes],
            used: 0,
        };
        staged.write(self.output.schema_bytes())?;
        execute(
            self.program.instructions,
            self.program.constants,
            input,
            &mut staged,
            &mut ResidentValues {
                children: &mut self.children,
                input: validated,
            },
        )?;
        let value = &staged.bytes[..staged.used];
        self.output
            .validate(value)
            .map_err(StaticStructuredRefusal::Output)?;
        if output.len() < value.len() {
            return Err(Refusal::InvalidProgram.into());
        }
        output[..value.len()].copy_from_slice(value);
        Ok(value.len())
    }
}
struct SliceOutput<'a> {
    bytes: &'a mut [u8],
    used: usize,
}
impl Output for SliceOutput<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        let end = self
            .used
            .checked_add(bytes.len())
            .ok_or(Refusal::InvalidProgram)?;
        self.bytes
            .get_mut(self.used..end)
            .ok_or(Refusal::InvalidProgram)?
            .copy_from_slice(bytes);
        self.used = end;
        Ok(())
    }
}
struct ResidentValues<'a, const I: usize> {
    children: &'a mut [StaticStructuredValue<I>],
    input: conduit_core::ValidatedCanonicalStructuredValue<'a>,
}
impl<const I: usize> ResidentValues<'_, I> {
    fn evaluate(
        &self,
        index: u16,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        let StaticStructuredValue::Scalar(value) = self
            .children
            .get(usize::from(index))
            .ok_or(Refusal::InvalidProgram)?
        else {
            return Err(Refusal::InvalidProgram.into());
        };
        value
            .evaluate_into(input, output)
            .map_err(|refusal| StaticStructuredRefusal::Child { index, refusal })
    }
}
impl<const I: usize> Values for ResidentValues<'_, I> {
    type Error = StaticStructuredRefusal;
    fn append(
        &mut self,
        index: u16,
        input: &[u8],
        output: &mut impl Output,
    ) -> Result<(), Self::Error> {
        if let Some(StaticStructuredValue::InputPath(path)) = self.children.get(usize::from(index))
        {
            let value = crate::expression_prepared::resident_projection::select(self.input, path)?;
            output.write(value.value_node())?;
            return Ok(());
        }
        let mut bytes = [0; 16];
        let n = self.evaluate(index, input, &mut bytes)?;
        output.write(&[0])?;
        output.write(&(n as u32).to_le_bytes())?;
        output.write(&bytes[..n])?;
        Ok(())
    }
    fn condition(&mut self, index: u16, input: &[u8]) -> Result<bool, Self::Error> {
        let mut bytes = [0; 16];
        let n = self.evaluate(index, input, &mut bytes)?;
        match &bytes[..n] {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(Refusal::InvalidProgram.into()),
        }
    }
}
