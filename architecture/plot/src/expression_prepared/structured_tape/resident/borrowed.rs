//! Checked flash child tables with explicitly funded sparse guard views.
use super::*;
pub struct BorrowedStaticStructuredExpression<
    const INPUT: usize,
    const OUTPUT: usize,
    const GUARDS: usize,
> {
    image: image::Image<INPUT, OUTPUT>,
    children: children::BorrowedChildren<GUARDS>,
}
impl<const I: usize, const O: usize, const G: usize> BorrowedStaticStructuredExpression<I, O, G> {
    /// Descriptions inherit this exact parent Native input. Actual child count
    /// comes from the immutable table; G funds only validated output guards.
    pub fn from_child_programs(
        input: ResidentNativeValidator<I>,
        output: ResidentNativeValidator<O>,
        table: &'static StaticStructuredChildTable,
        program: &'static StaticStructuredProgram,
    ) -> Result<Self, StaticStructuredRefusal> {
        image::validate_program(&input, program, table.children.len())?;
        let children = children::BorrowedChildren::prepare(&input, table)?;
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
impl<const I: usize, const O: usize, const G: usize> image::sealed::Sealed
    for BorrowedStaticStructuredExpression<I, O, G>
{
}
impl<const I: usize, const O: usize, const G: usize> StructuredExpressionImage
    for BorrowedStaticStructuredExpression<I, O, G>
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
