//! Shared prepared computational expression lifecycle, with no Host calls.
//! Target-specific offer validation remains with the installing Host.
use alloc::{format, string::String};
use conduit_core::{PortTemporal, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
pub fn prepared_expression_bounds(
    program: &PortableExpressionProgram,
) -> Result<(u32, u32), String> {
    let input = program
        .maximum_prepared_input_bytes()
        .map_err(|e| format!("expression input bound: {e:?}"))?;
    let output = program
        .maximum_prepared_output_bytes()
        .map_err(|e| format!("expression output bound: {e:?}"))?;
    if input as usize > MAXIMUM_STRUCTURED_CANONICAL_BYTES
        || output as usize > MAXIMUM_STRUCTURED_CANONICAL_BYTES
    {
        return Err("local expression exceeds canonical transport bound".into());
    }
    Ok((input, output))
}

mod resident;
pub use resident::{StaticExpressionBackRefusal, StaticScalarExpressionBack};
mod resident_structured;
pub use resident_structured::{StaticStructuredExpressionBack, StructuredResidentEvaluator};
#[cfg(feature = "execution-workspace")]
mod workspace_structured;
#[cfg(feature = "execution-workspace")]
pub use workspace_structured::{BorrowedWorkspaceStructuredExpressionBack, WorkspaceStructuredEvaluator, WorkspaceStructuredExpressionBack};

trait ExpressionEngine {
    type Refusal;
    fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], Self::Refusal>;
    #[cfg(feature = "schema-value-storage")]
    fn evaluate_parts(
        &mut self,
        _schema: &[u8],
        _node: &[u8],
    ) -> Option<Result<&[u8], Self::Refusal>> {
        None
    }
    fn input_schema(&self) -> Option<&[u8]> {
        None
    }
    fn output_schema(&self) -> Option<&[u8]> {
        None
    }
    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        None
    }
    #[cfg(feature = "execution-workspace")]
    fn evaluate_workspace(
        &mut self,
        _input: Option<&[u8]>,
        _parts: Option<conduit_kernel::CanonicalValueParts<'_>>,
        _workspace: &mut conduit_kernel::scheduler::StepWorkspace<'_>,
    ) -> Option<Result<usize, Self::Refusal>> {
        None
    }

    fn output_bytes(&self) -> &[u8];
    fn private_fuel(&self) -> u16 {
        0
    }
}
impl ExpressionEngine for PreparedPortableExpressionEvaluator {
    type Refusal = conduit_plot::PortableExpressionEvaluationRefusal;
    fn evaluate(
        &mut self,
        input: &[u8],
    ) -> Result<&[u8], conduit_plot::PortableExpressionEvaluationRefusal> {
        PreparedPortableExpressionEvaluator::evaluate(self, input)
    }
    fn output_bytes(&self) -> &[u8] {
        PreparedPortableExpressionEvaluator::output_bytes(self)
    }
}

pub struct PreparedExpressionBack<E = PreparedPortableExpressionEvaluator> {
    evaluator: E,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
    staged: bool,
    closing: bool,
    complete: bool,
    flow: bool,
    cancelled: bool,
    committed_values: u64,
}
impl PreparedExpressionBack {
    pub fn prepare(program: &PortableExpressionProgram) -> Result<Self, String> {
        let (maximum_input_bytes, maximum_output_bytes) = prepared_expression_bounds(program)?;
        Ok(Self {
            evaluator: PreparedPortableExpressionEvaluator::new(program)
                .map_err(|e| format!("prepare local expression: {e:?}"))?,
            maximum_input_bytes,
            maximum_output_bytes,
            staged: false,
            closing: false,
            complete: false,
            flow: false,
            cancelled: false,
            committed_values: 0,
        })
    }
    pub fn prepare_with_temporal(
        program: &PortableExpressionProgram,
        temporal: PortTemporal,
    ) -> Result<Self, String> {
        if !matches!(
            temporal,
            PortTemporal::Value | PortTemporal::Flow { closes: true }
        ) {
            return Err("computational expression requires Value or closing Flow".into());
        }
        let mut back = Self::prepare(program)?;
        back.flow = matches!(temporal, PortTemporal::Flow { closes: true });
        Ok(back)
    }
    pub fn output_capacity(&self) -> usize {
        self.evaluator.output_capacity()
    }
}
impl<E> PreparedExpressionBack<E> {
    fn step_inner<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        #[cfg(feature = "execution-workspace")] workspace: Option<
            &mut conduit_kernel::scheduler::StepWorkspace<'_>,
        >,
    ) -> StepOutcome
    where
        E: ExpressionEngine,
    {
        // Discard only uncommitted intent on a retried transaction.
        self.staged = false;
        self.closing = false;
        if self.complete {
            return StepOutcome::Complete;
        }
        #[cfg(feature = "execution-workspace")]
        if self.evaluator.workspace_requirements().is_some() && workspace.is_none() {
            return fail(1648);
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(1640);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if self.flow {
                    if io.consume_closed(PortId(0)).is_err() {
                        return fail(1641);
                    }
                    self.closing = true;
                    StepOutcome::Complete
                } else {
                    fail(1641)
                }
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let input = inputs.input(PortId(0));
        #[cfg(feature = "schema-value-storage")]
        let parts = inputs.canonical_parts(PortId(0));
        #[cfg(not(feature = "schema-value-storage"))]
        let parts: Option<conduit_kernel::CanonicalValueParts<'_>> = None;
        if input.is_none() && parts.is_none() || input.is_some() && parts.is_some() {
            return fail(1642);
        }
        let length = input
            .map(<[u8]>::len)
            .or_else(|| parts.and_then(|p| p.schema.len().checked_add(p.node.len())));
        if length != Some(reference.byte_len as usize)
            || length.is_none_or(|n| n > self.maximum_input_bytes as usize)
        {
            return fail(1643);
        }
        let private = self.evaluator.private_fuel();
        if private > 0 {
            let Some(complete) = private.checked_add(2) else {
                return fail(1647);
            };
            if io.remaining_fuel() < complete {
                let _ = io.consume_fuel(complete); // sticky fault, before work or IO intent
                return StepOutcome::Yield;
            }
        }
        if io.consume_fuel(private).is_err() {
            return StepOutcome::Yield; // sticky kernel fuel fault rejects the whole Step
        }
        #[cfg(feature = "execution-workspace")]
        if let Some(workspace) =
            workspace.filter(|_| self.evaluator.workspace_requirements().is_some())
        {
            let Some(Ok(length)) = self.evaluator.evaluate_workspace(input, parts, workspace)
            else {
                return fail(1644);
            };
            let Ok(length) = u32::try_from(length) else {
                return fail(1645);
            };
            if length > self.maximum_output_bytes {
                return fail(1645);
            }
            if io.send_workspace_output(PortId(0), length).is_err()
                || io.consume(PortId(0)).is_err()
            {
                return fail(1646);
            }
            self.staged = true;
            return StepOutcome::Progress;
        }
        let output = if let Some(input) = input {
            self.evaluator.evaluate(input).ok()
        } else {
            #[cfg(feature = "schema-value-storage")]
            {
                parts
                    .and_then(|p| self.evaluator.evaluate_parts(p.schema, p.node))
                    .and_then(Result::ok)
            }
            #[cfg(not(feature = "schema-value-storage"))]
            {
                None
            }
        };
        let Some(output) = output else {
            return fail(1644);
        };
        if output.len() > self.maximum_output_bytes as usize {
            return fail(1645);
        }
        if io.send_prepared(PortId(0), output.len() as u32).is_err()
            || io.consume(PortId(0)).is_err()
        {
            return fail(1646);
        }
        self.staged = true;
        StepOutcome::Progress
    }
}
impl<const PORTS: usize, E: ExpressionEngine> StepBack<PORTS> for PreparedExpressionBack<E> {
    fn canonical_input_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.evaluator.input_schema())
            .flatten()
    }
    fn canonical_output_schema(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.evaluator.output_schema())
            .flatten()
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.step_inner(
            io,
            inputs,
            #[cfg(feature = "execution-workspace")]
            None,
        )
    }
    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        self.evaluator.workspace_requirements()
    }
    #[cfg(feature = "execution-workspace")]
    fn step_with_workspace(
        &mut self,
        io: &mut StepIo<PORTS>,
        inputs: &StepInputBytes<'_, PORTS>,
        workspace: &mut conduit_kernel::scheduler::StepWorkspace<'_>,
    ) -> StepOutcome {
        self.step_inner(io, inputs, Some(workspace))
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        #[cfg(feature = "execution-workspace")]
        if self.evaluator.workspace_requirements().is_some() {
            return None;
        }
        (port == PortId(0) && self.staged).then(|| self.evaluator.output_bytes())
    }
    fn step_committed(&mut self) {
        if self.staged && !self.flow {
            self.committed_values += 1;
        }
        self.complete |= self.closing || (self.staged && !self.flow);
        self.staged = false;
        self.closing = false;
    }
    fn cancel(&mut self) {
        self.closing = false;
        self.cancelled = true;
        self.complete = true;
        self.staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(feature = "prepared-expression-session")]
impl<const P: usize, E: ExpressionEngine>
    conduit_kernel::scheduler::prepared_session::ReusableStepBack<P> for PreparedExpressionBack<E>
{
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn conduit_kernel::ValueStorage) -> bool {
        self.complete
            && !self.staged
            && !self.flow
            && !self.cancelled
            && self.committed_values < u64::MAX
    }
    fn begin_next(&mut self, _: ()) {
        self.complete = false;
    }
}
