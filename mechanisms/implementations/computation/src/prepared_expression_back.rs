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

pub struct PreparedExpressionBack {
    evaluator: PreparedPortableExpressionEvaluator,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
    staged: bool,
    staged_complete: bool,
    complete: bool,
    flow: bool,
    cancelled: bool,
    committed_values: u64,
}
impl PreparedExpressionBack {
    pub fn prepare(program: &PortableExpressionProgram) -> Result<Self, String> {
        let (maximum_input_bytes, maximum_output_bytes) = prepared_expression_bounds(program)?;
        Ok(Self {
            evaluator: PreparedPortableExpressionEvaluator::new_compact(program)
                .map_err(|e| format!("prepare local expression: {e:?}"))?,
            maximum_input_bytes,
            maximum_output_bytes,
            staged: false,
            staged_complete: false,
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
    /// Initialize only pure mutable lifecycle state from exact installed Make
    /// material. No offer, current Host, assignment or runtime authority exists.
    /// # Safety
    /// Caller authenticates complete checked Make origin and actual canonical
    /// Source/configuration binding, including the expected bounds and digest.
    pub unsafe fn from_linked_make(
        material: &'static [u8],
        program_digest: [u8; 32],
        bounds: (u32, u32),
        temporal: PortTemporal,
    ) -> Result<Self, String> {
        if !matches!(
            temporal,
            PortTemporal::Value | PortTemporal::Flow { closes: true }
        ) {
            return Err("computational expression requires Value or closing Flow".into());
        }
        // SAFETY: the caller supplies the complete installed-Make provenance.
        let evaluator = unsafe {
            PreparedPortableExpressionEvaluator::from_linked_make_with_bounds(
                material,
                program_digest,
                bounds,
            )
        }
        .map_err(|e| format!("prepare linked expression: {e:?}"))?;
        Ok(Self {
            evaluator,
            maximum_input_bytes: bounds.0,
            maximum_output_bytes: bounds.1,
            staged: false,
            staged_complete: false,
            complete: false,
            flow: matches!(temporal, PortTemporal::Flow { closes: true }),
            cancelled: false,
            committed_values: 0,
        })
    }
    pub fn output_capacity(&self) -> usize {
        self.evaluator.output_capacity()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for PreparedExpressionBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        // Discard only uncommitted intent on a retried transaction.
        self.staged = false;
        self.staged_complete = false;
        if self.complete {
            return StepOutcome::Complete;
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
                    self.staged_complete = true;
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
        let Some(input) = inputs.input(PortId(0)) else {
            return fail(1642);
        };
        if input.len() != reference.byte_len as usize
            || input.len() > self.maximum_input_bytes as usize
        {
            return fail(1643);
        }
        let Ok(output) = self.evaluator.evaluate(input) else {
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
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then(|| self.evaluator.output_bytes())
    }
    fn step_committed(&mut self) {
        if self.staged && !self.flow {
            self.committed_values += 1;
        }
        self.complete |= self.staged_complete || (self.staged && !self.flow);
        self.staged = false;
        self.staged_complete = false;
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.complete = true;
        self.staged = false;
        self.staged_complete = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(feature = "prepared-expression-session")]
impl<const P: usize> conduit_kernel::scheduler::prepared_session::ReusableStepBack<P>
    for PreparedExpressionBack
{
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn conduit_kernel::ValueStorage) -> bool {
        self.complete
            && !self.staged
            && !self.staged_complete
            && !self.cancelled
            && self.committed_values < u64::MAX
    }
    fn begin_next(&mut self, _: ()) {
        self.complete = false;
    }
}
