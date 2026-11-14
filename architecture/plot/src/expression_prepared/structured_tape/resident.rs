//! Resident constructor data; Root independently admits its full realization.
use super::*;
use crate::expression_prepared::canonical_input::CanonicalInput;
use crate::expression_prepared::{StaticProjectedExpression, StaticProjectedRefusal};
use crate::rust_binding::{PreparedNativeRefusal, ResidentNativeValidator};
#[path = "resident/borrowed.rs"]
mod borrowed;
#[path = "resident/children.rs"]
mod children;
#[path = "resident/generate.rs"]
mod generate;
#[path = "resident/image.rs"]
mod image;
pub use borrowed::BorrowedStaticStructuredExpression;
use children::ParentCheckedStructuredValue;
pub use children::StaticStructuredChildTable;
pub use generate::{
    generate_borrowed_static_structured_expression_with_work,
    generate_static_structured_expression, generate_static_structured_expression_with_work,
    GeneratedStaticStructuredExpression, StaticStructuredGenerationRefusal,
};
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
/// Sealed resident images preserve the same checked Native execution boundary.
/// Storage choices neither initialize a Back nor grant execution authority.
pub trait StructuredExpressionImage: image::sealed::Sealed {
    fn input_schema_bytes(&self) -> Option<&[u8]>;
    fn output_schema_bytes(&self) -> Option<&[u8]>;
    fn maximum_input_bytes(&self) -> usize;
    fn maximum_output_bytes(&self) -> usize;
    fn computation_work(&self) -> u32;
    fn scratch_bytes(&self) -> usize;
    fn evaluate_into(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal>;
    fn evaluate_parts_into(
        &mut self,
        schema: &[u8],
        node: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal>;
}
/// Independently prepared children retain their checked views inline. No heap
/// allocation is hidden in this owned metadata storage choice. Root must still
/// bind the checked artifact and admit state, stack, scratch and mandatory work.
pub struct StaticStructuredExpression<
    const INPUT: usize,
    const OUTPUT: usize,
    const CHILDREN: usize,
> {
    image: image::Image<INPUT, OUTPUT>,
    children: [ParentCheckedStructuredValue; CHILDREN],
}
impl<const I: usize, const O: usize, const C: usize> StaticStructuredExpression<I, O, C> {
    /// Data must be trusted Make output, never self-declared incoming authority.
    pub fn new(
        input: ResidentNativeValidator<I>,
        output: ResidentNativeValidator<O>,
        children: [StaticStructuredValue<I>; C],
        program: &'static StaticStructuredProgram,
    ) -> Result<Self, Refusal> {
        image::validate_program(&input, program, C)?;
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
            image: image::Image {
                input,
                output,
                program,
            },
            children,
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
        image::validate_program(&input, program, C)?;
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
            image: image::Image {
                input,
                output,
                program,
            },
            children,
        })
    }
    pub fn input_schema_bytes(&self) -> Option<&[u8]> {
        self.image.input_schema_bytes()
    }
    pub fn output_schema_bytes(&self) -> Option<&[u8]> {
        self.image.output_schema_bytes()
    }
    pub const fn maximum_input_bytes(&self) -> usize {
        self.image.maximum_input_bytes()
    }
    pub const fn maximum_output_bytes(&self) -> usize {
        self.image.maximum_output_bytes()
    }
    pub fn computation_work(&self) -> u32 {
        self.image.computation_work(&self.children)
    }
    pub fn scratch_bytes(&self) -> usize {
        self.image.scratch_bytes()
    }
    pub fn evaluate_into(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        self.image
            .evaluate_into(input, output, scratch, &self.children)
    }
    pub fn evaluate_parts_into(
        &mut self,
        schema: &[u8],
        node: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        self.image
            .evaluate_parts_into(schema, node, output, scratch, &self.children)
    }
}
impl<const I: usize, const O: usize, const C: usize> image::sealed::Sealed
    for StaticStructuredExpression<I, O, C>
{
}
impl<const I: usize, const O: usize, const C: usize> StructuredExpressionImage
    for StaticStructuredExpression<I, O, C>
{
    fn input_schema_bytes(&self) -> Option<&[u8]> {
        self.input_schema_bytes()
    }
    fn output_schema_bytes(&self) -> Option<&[u8]> {
        self.output_schema_bytes()
    }
    fn maximum_input_bytes(&self) -> usize {
        self.maximum_input_bytes()
    }
    fn maximum_output_bytes(&self) -> usize {
        self.maximum_output_bytes()
    }
    fn computation_work(&self) -> u32 {
        self.computation_work()
    }
    fn scratch_bytes(&self) -> usize {
        self.scratch_bytes()
    }
    fn evaluate_into(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        self.evaluate_into(input, output, scratch)
    }
    fn evaluate_parts_into(
        &mut self,
        schema: &[u8],
        node: &[u8],
        output: &mut [u8],
        scratch: &mut [u8],
    ) -> Result<usize, StaticStructuredRefusal> {
        self.evaluate_parts_into(schema, node, output, scratch)
    }
}
