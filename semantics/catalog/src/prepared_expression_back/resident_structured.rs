//! Resident structured evaluation uses the common transactional Back lifecycle.
use super::*;
use conduit_plot::{StaticStructuredExpression, StaticStructuredRefusal};
pub struct StructuredResidentEvaluator<
    const I: usize,
    const O: usize,
    const C: usize,
    const BYTES: usize,
> {
    image: StaticStructuredExpression<I, O, C>,
    output: [u8; BYTES],
    scratch: [u8; BYTES],
    length: usize,
    fuel: u16,
    refusal: Option<StaticStructuredRefusal>,
}
impl<const I: usize, const O: usize, const C: usize, const B: usize> ExpressionEngine
    for StructuredResidentEvaluator<I, O, C, B>
{
    type Refusal = ();
    fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], ()> {
        self.length = 0;
        self.refusal = None;
        match self
            .image
            .evaluate_into(input, &mut self.output, &mut self.scratch)
        {
            Ok(length) => {
                self.length = length;
                Ok(&self.output[..length])
            }
            Err(refusal) => {
                self.refusal = Some(refusal);
                Err(())
            }
        }
    }
    fn output_bytes(&self) -> &[u8] {
        &self.output[..self.length]
    }
    fn private_fuel(&self) -> u16 {
        self.fuel
    }
}
pub type StaticStructuredExpressionBack<
    const I: usize,
    const O: usize,
    const C: usize,
    const B: usize,
> = PreparedExpressionBack<StructuredResidentEvaluator<I, O, C, B>>;
impl<const I: usize, const O: usize, const C: usize, const B: usize>
    StaticStructuredExpressionBack<I, O, C, B>
{
    /// Finite storage and work are checked without allocation. Root must still
    /// admit the actual Back object, stack, byte envelopes and exact per-Step
    /// fuel and bind Source/Fore/artifact before publishing an initialized Back.
    pub fn from_resident_structured(
        image: StaticStructuredExpression<I, O, C>,
        temporal: PortTemporal,
    ) -> Result<Self, StaticExpressionBackRefusal> {
        if !matches!(
            temporal,
            PortTemporal::Value | PortTemporal::Flow { closes: true }
        ) {
            return Err(StaticExpressionBackRefusal::UnsupportedTemporal);
        }
        if B < image.maximum_output_bytes() || B > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(StaticExpressionBackRefusal::Bounds);
        }
        let fuel = u16::try_from(image.computation_work())
            .map_err(|_| StaticExpressionBackRefusal::Work)?;
        if fuel.checked_add(2).is_none() {
            return Err(StaticExpressionBackRefusal::Work);
        }
        Ok(Self {
            maximum_input_bytes: image.maximum_input_bytes() as u32,
            maximum_output_bytes: image.maximum_output_bytes() as u32,
            evaluator: StructuredResidentEvaluator {
                image,
                output: [0; B],
                scratch: [0; B],
                length: 0,
                fuel,
                refusal: None,
            },
            staged: false,
            complete: false,
            flow: matches!(temporal, PortTemporal::Flow { closes: true }),
            cancelled: false,
            committed_values: 0,
        })
    }
    pub fn computation_fuel(&self) -> u16 {
        self.evaluator.fuel
    }
    pub fn required_step_fuel(&self) -> u16 {
        self.evaluator.fuel + 2
    }
    pub fn last_evaluation_refusal(&self) -> Option<&StaticStructuredRefusal> {
        self.evaluator.refusal.as_ref()
    }
    pub fn output_capacity(&self) -> usize {
        B
    }
    pub fn scratch_bytes(&self) -> usize {
        B
    }
}
