//! Shared instruction validation and evaluation for independently checked storage.
use super::children::Children;
use super::*;
pub mod sealed {
    pub trait Sealed {}
}
pub(super) struct Image<const I: usize, const O: usize> {
    pub(super) input: ResidentNativeValidator<I>,
    pub(super) output: ResidentNativeValidator<O>,
    pub(super) program: &'static StaticStructuredProgram,
}
pub(super) fn validate_program<const I: usize>(
    input: &ResidentNativeValidator<I>,
    program: &'static StaticStructuredProgram,
    child_count: usize,
) -> Result<(), Refusal> {
    if child_count > MAX_VALUES
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
            Operation::Value(index) => usize::from(index) < child_count,
            Operation::BranchFalse { value, target } => {
                usize::from(value) < child_count
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

impl<const I: usize, const O: usize> Image<I, O> {
    /// Exact immutable structured input schema; raw Empty has no such transport.
    pub(super) fn input_schema_bytes(&self) -> Option<&[u8]> {
        (!self.input.is_intrinsic_empty()).then(|| self.input.schema_bytes())
    }
    /// Immutable output schema, independent of any particular produced value.
    pub(super) fn output_schema_bytes(&self) -> Option<&[u8]> {
        (!self.output.is_intrinsic_empty()).then(|| self.output.schema_bytes())
    }
    pub(super) const fn maximum_input_bytes(&self) -> usize {
        self.program.maximum_input_bytes
    }
    pub(super) const fn maximum_output_bytes(&self) -> usize {
        self.program.maximum_output_bytes
    }
    /// Upper bound in cooperative bounded-operation units, not CPU cycles.
    /// Both validation passes, all possible constructor/child operations and
    /// publication staging are charged, including unselected branches.
    // Keep cold accounting separate from each generated constructor. Inlining
    // through linked child tables duplicates traversal code per expression.
    #[inline(never)]
    pub(super) fn computation_work(&self, children: &impl Children) -> u32 {
        use crate::expression_prepared::resident_work::{direct_path_work, structured_work};
        structured_work(
            self.input.validation_work(self.program.maximum_input_bytes),
            self.output
                .validation_work(self.program.maximum_output_bytes),
            self.program.instructions,
            |index| {
                children
                    .get(usize::from(index))
                    .map_or(u32::MAX, |child| match child {
                        ParentCheckedStructuredValue::Scalar(child) => child.checked_work(),
                        ParentCheckedStructuredValue::InputPath(path) => {
                            direct_path_work(path.len())
                        }
                    })
            },
        )
    }
    pub(super) fn scratch_bytes(&self) -> usize {
        self.program.maximum_output_bytes
    }
    /// Scratch is separately admitted staging, reused after success or refusal.
    /// No bytes reach `output` until the full constructed Native value passes.
    pub(super) fn evaluate_into(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
        children: &impl Children,
    ) -> Result<usize, StaticStructuredRefusal> {
        let validated = self
            .input
            .validate_borrowed(input)
            .map_err(StaticStructuredRefusal::Input)?;
        self.evaluate_checked_into(
            CanonicalInput::Whole(input),
            validated,
            output,
            scratch,
            children,
        )
    }
    /// Evaluate schema-separated input without concatenating it. Every Native
    /// input/output check and the original combined byte ceilings remain.
    pub(super) fn evaluate_parts_into(
        &mut self,
        schema: &[u8],
        node: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
        children: &impl Children,
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
            children,
        )
    }
    fn evaluate_checked_into(
        &mut self,
        input: CanonicalInput<'_>,
        validated: conduit_core::ValidatedCanonicalStructuredValue<'_>,
        output: &mut [u8],
        scratch: &mut [u8],
        children: &impl Children,
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
                children,
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
struct ResidentValues<'a, T: Children> {
    children: &'a T,
    input: conduit_core::ValidatedCanonicalStructuredValue<'a>,
}
impl<T: Children> ResidentValues<'_, T> {
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
impl<T: Children> Values for ResidentValues<'_, T> {
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
