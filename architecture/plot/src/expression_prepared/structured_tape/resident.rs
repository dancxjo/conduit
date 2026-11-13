//! Resident constructor data; Root independently admits its full realization.
use super::*;
use crate::expression_prepared::canonical_input::CanonicalInput;
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
/// Make-owned children sharing the enclosing expression's exact Native input.
/// Descriptors are not prepared values or execution/admission witnesses.
pub enum StaticStructuredChildProgram {
    Scalar(&'static crate::StaticProjectedProgram),
    InputPath(&'static [crate::StaticExpressionPathStep]),
}
// Constructor-only input retains each independently prepared validator until
// exact parent correspondence has passed. Runtime children retain only code.
enum ParentCheckedStructuredValue {
    Scalar(crate::expression_prepared::scalar_tape::projected::ParentCheckedProjectedExpression),
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
    children: [ParentCheckedStructuredValue; CHILDREN],
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
        Self::validate_program(&input, program)?;
        for child in &children {
            if let StaticStructuredValue::Scalar(child) = child {
                if !child.input_matches(&input) {
                    return Err(Refusal::InvalidProgram);
                }
            }
            if let StaticStructuredValue::InputPath(path) = child {
                if !crate::expression_prepared::resident_projection::valid(path) {
                    return Err(Refusal::InvalidProgram);
                }
            }
        }
        let children = children.map(|child| match child {
            StaticStructuredValue::Scalar(child) => {
                ParentCheckedStructuredValue::Scalar(child.into_parent_checked())
            }
            StaticStructuredValue::InputPath(path) => ParentCheckedStructuredValue::InputPath(path),
        });
        Ok(Self {
            input,
            output,
            children,
            program,
        })
    }
    /// Prepare immutable child descriptions through one bounded loop. Every
    /// scalar inherits this exact already-prepared Native input; independently
    /// prepared children still use `new` and its correspondence checks.
    #[inline(never)]
    pub fn from_child_programs(
        input: ResidentNativeValidator<I>,
        output: ResidentNativeValidator<O>,
        descriptors: &'static [StaticStructuredChildProgram; C],
        program: &'static StaticStructuredProgram,
    ) -> Result<Self, StaticStructuredRefusal> {
        Self::validate_program(&input, program)?;
        let mut refusal = None;
        let children = core::array::from_fn(|index| {
            if refusal.is_some() {
                return ParentCheckedStructuredValue::InputPath(&[]);
            }
            let prepared = match &descriptors[index] {
                StaticStructuredChildProgram::Scalar(program) => {
                    StaticProjectedExpression::new(input.clone(), program)
                        .map(|child| {
                            ParentCheckedStructuredValue::Scalar(child.into_parent_checked())
                        })
                        .map_err(|error| StaticStructuredRefusal::Child {
                            index: index as u16,
                            refusal: StaticProjectedRefusal::Expression(error),
                        })
                }
                StaticStructuredChildProgram::InputPath(path) => {
                    if crate::expression_prepared::resident_projection::valid(path) {
                        Ok(ParentCheckedStructuredValue::InputPath(path))
                    } else {
                        Err(Refusal::InvalidProgram.into())
                    }
                }
            };
            match prepared {
                Ok(child) => child,
                Err(error) => {
                    refusal = Some(error);
                    ParentCheckedStructuredValue::InputPath(&[])
                }
            }
        });
        if let Some(refusal) = refusal {
            return Err(refusal);
        }
        Ok(Self {
            input,
            output,
            children,
            program,
        })
    }
    fn validate_program(
        input: &ResidentNativeValidator<I>,
        program: &'static StaticStructuredProgram,
    ) -> Result<(), Refusal> {
        if C > MAX_VALUES
            || program.instructions.is_empty()
            || program.instructions.len() > MAX_OPERATIONS
            || program.constants.len() > MAX_CONSTANT_BYTES
            || [program.maximum_input_bytes, program.maximum_output_bytes]
                .iter()
                .any(|n| *n > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES)
            || (program.maximum_input_bytes == 0 && !input.is_intrinsic_empty())
            || program.maximum_output_bytes == 0
        {
            return Err(Refusal::InvalidProgram);
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
        Ok(())
    }
    /// Exact immutable structured input schema; raw Empty has no such transport.
    pub fn input_schema_bytes(&self) -> Option<&[u8]> {
        (!self.input.is_intrinsic_empty()).then(|| self.input.schema_bytes())
    }
    /// Immutable output schema, independent of any particular produced value.
    pub fn output_schema_bytes(&self) -> Option<&[u8]> {
        (!self.output.is_intrinsic_empty()).then(|| self.output.schema_bytes())
    }
    pub const fn maximum_input_bytes(&self) -> usize {
        self.program.maximum_input_bytes
    }
    pub const fn maximum_output_bytes(&self) -> usize {
        self.program.maximum_output_bytes
    }
    /// Upper bound in cooperative bounded-operation units, not CPU cycles.
    /// Both validation passes, all possible constructor/child operations and
    /// publication staging are charged, including unselected branches.
    // Keep cold accounting separate from each generated constructor. Inlining
    // through linked child tables duplicates traversal code per expression.
    #[inline(never)]
    pub fn computation_work(&self) -> u32 {
        let mut work = self
            .input
            .validation_work(self.program.maximum_input_bytes)
            .saturating_add(
                self.output
                    .validation_work(self.program.maximum_output_bytes),
            )
            .saturating_add(2); // schema prefix and final bounded result copy
        let child_work = |index: u16| {
            self.children
                .get(usize::from(index))
                .map_or(u32::MAX, |child| match child {
                    ParentCheckedStructuredValue::Scalar(child) => child.checked_work(),
                    ParentCheckedStructuredValue::InputPath(path) => (path.len() + 1) as u32,
                })
        };
        for op in self.program.instructions {
            let units = match *op {
                Operation::Value(i) => child_work(i).saturating_add(4),
                Operation::BranchFalse { value, .. } => child_work(value).saturating_add(2),
                Operation::Bytes { .. } | Operation::InputNode { .. } => 2,
                Operation::Jump(_) => 1,
            };
            work = work.saturating_add(units);
        }
        work
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
        self.evaluate_checked_into(CanonicalInput::Whole(input), validated, output, scratch)
    }
    /// Evaluate schema-separated input without concatenating it. Every Native
    /// input/output check and the original combined byte ceilings remain.
    pub fn evaluate_parts_into(
        &mut self,
        schema: &[u8],
        node: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        let validated = self
            .input
            .validate_parts_borrowed(schema, node)
            .map_err(StaticStructuredRefusal::Input)?;
        self.evaluate_checked_into(
            CanonicalInput::Separated {
                schema: validated.type_bytes(),
                node: validated.value_node(),
            },
            validated,
            output,
            scratch,
        )
    }
    fn evaluate_checked_into(
        &mut self,
        input: CanonicalInput<'_>,
        validated: conduit_core::ValidatedCanonicalStructuredValue<'_>,
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        if input
            .len()
            .is_none_or(|n| n > self.program.maximum_input_bytes)
        {
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
struct ResidentValues<'a> {
    children: &'a mut [ParentCheckedStructuredValue],
    input: conduit_core::ValidatedCanonicalStructuredValue<'a>,
}
impl ResidentValues<'_> {
    fn evaluate(
        &self,
        index: u16,
        input: CanonicalInput<'_>,
        output: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        let ParentCheckedStructuredValue::Scalar(value) = self
            .children
            .get(usize::from(index))
            .ok_or(Refusal::InvalidProgram)?
        else {
            return Err(Refusal::InvalidProgram.into());
        };
        value
            .evaluate_checked_input_into(input, self.input, output)
            .map_err(|refusal| StaticStructuredRefusal::Child { index, refusal })
    }
}
impl Values for ResidentValues<'_> {
    type Error = StaticStructuredRefusal;
    fn append(
        &mut self,
        index: u16,
        input: CanonicalInput<'_>,
        output: &mut impl Output,
    ) -> Result<(), Self::Error> {
        if let Some(ParentCheckedStructuredValue::InputPath(path)) =
            self.children.get(usize::from(index))
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
    fn condition(&mut self, index: u16, input: CanonicalInput<'_>) -> Result<bool, Self::Error> {
        let mut bytes = [0; 16];
        let n = self.evaluate(index, input, &mut bytes)?;
        match &bytes[..n] {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(Refusal::InvalidProgram.into()),
        }
    }
}
