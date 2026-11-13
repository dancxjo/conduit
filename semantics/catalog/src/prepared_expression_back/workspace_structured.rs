//! Resident expression state without Back-owned output or scratch arrays.
use super::*;
use conduit_kernel::scheduler::{StepWorkspace, StepWorkspaceRequirements};
use conduit_plot::{StaticStructuredExpression, StaticStructuredRefusal};
pub struct WorkspaceStructuredEvaluator<const I: usize, const O: usize, const C: usize> {
    image: StaticStructuredExpression<I, O, C>,
    fuel: u16,
    refusal: Option<StaticStructuredRefusal>,
    #[cfg(feature = "schema-value-storage")]
    separated_input: bool,
}
impl<const I: usize, const O: usize, const C: usize> ExpressionEngine
    for WorkspaceStructuredEvaluator<I, O, C>
{
    type Refusal = ();
    fn evaluate(&mut self, _: &[u8]) -> Result<&[u8], ()> {
        Err(())
    }
    fn output_bytes(&self) -> &[u8] {
        &[]
    }
    fn private_fuel(&self) -> u16 {
        self.fuel
    }
    #[cfg(feature = "schema-value-storage")]
    fn output_schema(&self) -> Option<&[u8]> {
        self.separated_input
            .then(|| self.image.output_schema_bytes())
            .flatten()
    }
    #[cfg(feature = "schema-value-storage")]
    fn input_schema(&self) -> Option<&[u8]> {
        self.separated_input
            .then(|| self.image.input_schema_bytes())
            .flatten()
    }
    fn workspace_requirements(&self) -> Option<StepWorkspaceRequirements> {
        Some(StepWorkspaceRequirements {
            output_bytes: self.image.maximum_output_bytes() as u32,
            scratch_bytes: self.image.scratch_bytes() as u32,
        })
    }
    fn evaluate_workspace(
        &mut self,
        input: Option<&[u8]>,
        parts: Option<conduit_kernel::CanonicalValueParts<'_>>,
        workspace: &mut StepWorkspace<'_>,
    ) -> Option<Result<usize, ()>> {
        self.refusal = None;
        let (output, scratch) = workspace.buffers();
        let result = if let Some(input) = input {
            self.image.evaluate_into(input, output, scratch)
        } else {
            #[cfg(feature = "schema-value-storage")]
            {
                if !self.separated_input {
                    return Some(Err(()));
                }
                let parts = parts?;
                self.image
                    .evaluate_parts_into(parts.schema, parts.node, output, scratch)
            }
            #[cfg(not(feature = "schema-value-storage"))]
            {
                let _ = parts;
                return Some(Err(()));
            }
        };
        Some(match result {
            Ok(n) => Ok(n),
            Err(refusal) => {
                self.refusal = Some(refusal);
                Err(())
            }
        })
    }
}
pub type WorkspaceStructuredExpressionBack<const I: usize, const O: usize, const C: usize> =
    PreparedExpressionBack<WorkspaceStructuredEvaluator<I, O, C>>;
impl<const I: usize, const O: usize, const C: usize> WorkspaceStructuredExpressionBack<I, O, C> {
    /// Select the independently checked resident image. All output/scratch
    /// storage is owned and admitted by the scheduler, never retained here.
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
        let fuel = u16::try_from(image.computation_work())
            .map_err(|_| StaticExpressionBackRefusal::Work)?;
        if fuel.checked_add(2).is_none() {
            return Err(StaticExpressionBackRefusal::Work);
        }
        Ok(Self {
            maximum_input_bytes: image.maximum_input_bytes() as u32,
            maximum_output_bytes: image.maximum_output_bytes() as u32,
            evaluator: WorkspaceStructuredEvaluator {
                image,
                fuel,
                refusal: None,
                #[cfg(feature = "schema-value-storage")]
                separated_input: false,
            },
            staged: false,
            closing: false,
            complete: false,
            flow: matches!(temporal, PortTemporal::Flow { closes: true }),
            cancelled: false,
            committed_values: 0,
        })
    }
    #[cfg(feature = "schema-value-storage")]
    pub fn from_resident_structured_parts(
        image: StaticStructuredExpression<I, O, C>,
        temporal: PortTemporal,
    ) -> Result<Self, StaticExpressionBackRefusal> {
        if image.input_schema_bytes().is_none() {
            return Err(StaticExpressionBackRefusal::Bounds);
        }
        let mut back = Self::from_resident_structured(image, temporal)?;
        back.evaluator.separated_input = true;
        Ok(back)
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
    pub fn reset_after_retirement(&mut self) -> bool {
        if !self.complete || self.staged || self.closing {
            return false;
        }
        self.evaluator.refusal = None;
        self.complete = false;
        self.cancelled = false;
        self.committed_values = 0;
        true
    }
}
