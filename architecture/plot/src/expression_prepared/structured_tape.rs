//! Linear constructor encoding; owned and resident profiles share one loop.
use super::{child, PreparedChild};
use crate::expression_prepared::canonical_input::CanonicalInput;
use crate::expression_prepared::{PreparedInput, ProgramView, Refusal};
use crate::PortableExpressionOperation as Expression;
use alloc::vec::Vec;
#[path = "structured_tape/compile.rs"]
mod compile;
#[path = "structured_tape/execution.rs"]
pub(super) mod execution;
#[path = "structured_tape/resident.rs"]
mod resident;
use compile::ConstructorTape;
pub use execution::StructuredTapeInstruction;
use execution::{execute, Output, Values};
pub use resident::{
    generate_borrowed_static_structured_expression_with_work,
    generate_static_structured_expression, generate_static_structured_expression_with_work,
    BorrowedStaticStructuredExpression, GeneratedStaticStructuredExpression,
    StaticStructuredChildProgram, StaticStructuredChildTable, StaticStructuredExpression,
    StaticStructuredGenerationRefusal, StaticStructuredProgram, StaticStructuredRefusal,
    StaticStructuredValue, StructuredExpressionImage,
};
use StructuredTapeInstruction as Operation;
const MAX_OPERATIONS: usize = 512;
const MAX_VALUES: usize = 128;
const MAX_CONSTANT_BYTES: usize = 64 * 1024;
pub(super) struct StructuredTape {
    program: ConstructorTape,
    values: Vec<PreparedChild>,
}
impl StructuredTape {
    /// Optimization refusal leaves ordinary preparation to report its result.
    pub(super) fn prepare(program: ProgramView<'_>, input: &PreparedInput) -> Option<Self> {
        if !matches!(
            program.root.operation,
            Expression::Record(_)
                | Expression::Tuple(_)
                | Expression::Collection(_)
                | Expression::Variant { .. }
                | Expression::Conditional { .. }
        ) {
            return None;
        }
        let mut values = Vec::new();
        let tape = ConstructorTape::compile(program, input, &mut |node| {
            values.push(child(program, node, input)?);
            Ok(())
        })
        .ok()?;
        values.shrink_to_fit();
        Some(Self {
            program: tape,
            values,
        })
    }
    pub(super) fn evaluate(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<(), Refusal> {
        execute(
            &self.program.code,
            &self.program.constants,
            CanonicalInput::Whole(input),
            output,
            &mut OwnedValues(&mut self.values),
        )
    }
}
struct OwnedValues<'a>(&'a mut [PreparedChild]);
impl Values for OwnedValues<'_> {
    type Error = Refusal;
    fn append(
        &mut self,
        index: u16,
        input: CanonicalInput<'_>,
        output: &mut impl Output,
    ) -> Result<(), Refusal> {
        self.0
            .get_mut(usize::from(index))
            .ok_or(Refusal::InvalidProgram)?
            .append_node(input.whole().ok_or(Refusal::InvalidInput)?, output)
    }
    fn condition(&mut self, index: u16, input: CanonicalInput<'_>) -> Result<bool, Refusal> {
        match self
            .0
            .get_mut(usize::from(index))
            .ok_or(Refusal::InvalidProgram)?
            .evaluator
            .evaluate(input.whole().ok_or(Refusal::InvalidInput)?)?
        {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(Refusal::InvalidProgram),
        }
    }
}
