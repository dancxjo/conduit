//! Resident code supplies computation; the common expression Back owns lifecycle.
use super::*;
use conduit_plot::{PortableExpressionEvaluationRefusal, StaticScalarExpression};

/// Opaque resident evaluator state. The Back owns one fixed-width result buffer.
pub struct ResidentEvaluator {
    image: StaticScalarExpression,
    output: [u8; 16],
    length: usize,
}
impl ExpressionEngine for ResidentEvaluator {
    type Refusal = PortableExpressionEvaluationRefusal;
    fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], PortableExpressionEvaluationRefusal> {
        self.length = self.image.evaluate_into(input, &mut self.output)?;
        Ok(&self.output[..self.length])
    }
    fn output_bytes(&self) -> &[u8] {
        &self.output[..self.length]
    }
    fn private_fuel(&self) -> u16 {
        self.image.computation_work()
    }
}
pub type StaticScalarExpressionBack = PreparedExpressionBack<ResidentEvaluator>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticExpressionBackRefusal {
    UnsupportedTemporal,
    Bounds,
    Work,
}
impl StaticScalarExpressionBack {
    /// Install already validated resident code with the ordinary Back lifecycle.
    /// This allocates nothing and publishes no offer. Root must independently
    /// bind the code artifact to the exact selected Source/Fore and admit Back
    /// state, evaluation stack and worst-case instruction/validation fuel plus kernel IO.
    pub fn from_resident(
        image: StaticScalarExpression,
        temporal: PortTemporal,
    ) -> Result<Self, StaticExpressionBackRefusal> {
        if !matches!(
            temporal,
            PortTemporal::Value | PortTemporal::Flow { closes: true }
        ) {
            return Err(StaticExpressionBackRefusal::UnsupportedTemporal);
        }
        Ok(Self {
            maximum_input_bytes: image.input_bytes() as u32,
            maximum_output_bytes: image.output_bytes() as u32,
            evaluator: ResidentEvaluator {
                image,
                output: [0; 16],
                length: 0,
            },
            staged: false,
            complete: false,
            flow: matches!(temporal, PortTemporal::Flow { closes: true }),
            cancelled: false,
            committed_values: 0,
        })
    }
    /// Pure computation charge; kernel-visible IO consumes additional fuel.
    pub fn computation_fuel(&self) -> u16 {
        self.evaluator.private_fuel()
    }
    /// Root must compare this with the exact selected node's grant before Play.
    /// One send and one consume accompany the bounded computation charge.
    pub fn required_step_fuel(&self) -> u16 {
        self.computation_fuel() + 2
    }
    pub fn scratch_bytes(&self) -> usize {
        self.evaluator.image.scratch_bytes()
    }
}
