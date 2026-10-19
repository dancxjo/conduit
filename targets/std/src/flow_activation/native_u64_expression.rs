//! Exact local expression lifecycle with admitted native U64 interior values.
use conduit_core::{PlannedGear, PortTemporal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_plot::{PortableExpressionProgram, PreparedNativeU64Expression};
pub struct PreparedNativeU64ExpressionBack {
    program: PreparedNativeU64Expression,
    boundary: [u8; 8],
    staged: bool,
    canonical_staged: bool,
    complete: bool,
    flow: bool,
}
impl PreparedNativeU64ExpressionBack {
    pub fn prepare(program: &PortableExpressionProgram) -> Result<Self, String> {
        Ok(Self {
            program: PreparedNativeU64Expression::new(program)
                .map_err(|e| format!("prepare native U64 expression: {e:?}"))?,
            boundary: [0; 8],
            staged: false,
            canonical_staged: false,
            complete: false,
            flow: false,
        })
    }
    /// Initialization validates the same exact installed local Kind and Offer;
    /// the containing Host still owns Plan admission and native-region binding.
    pub fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let mut back = Self::prepare(&super::local_expression::validate(placement)?)?;
        back.flow = matches!(
            placement.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        Ok(back)
    }
    pub fn retained_capacity_bytes(&self) -> usize {
        self.program.retained_storage_bytes()
    }
}
impl<const P: usize> StepBack<P> for PreparedNativeU64ExpressionBack {
    fn native_u64_ports(&self) -> Option<(PortId, PortId)> {
        Some((PortId(0), PortId(0)))
    }
    fn step(&mut self, io: &mut StepIo<P>, inputs: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = false;
        self.canonical_staged = false;
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
        if reference.byte_len != 8 {
            return fail(1643);
        }
        let value = if let Some(value) = inputs.native_u64(PortId(0)) {
            value
        } else {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(1642);
            };
            let Ok(bytes) = <&[u8; 8]>::try_from(bytes) else {
                return fail(1643);
            };
            u64::from_le_bytes(*bytes)
        };
        let Ok(output) = self.program.evaluate(value) else {
            return fail(1644);
        };
        let sent = if io.native_u64_output(PortId(0)) {
            io.send_native_u64(PortId(0), output)
        } else {
            self.boundary = output.to_le_bytes();
            self.canonical_staged = true;
            io.send_prepared(PortId(0), 8)
        };
        if sent.is_err() || io.consume(PortId(0)).is_err() {
            return fail(1646);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged && self.canonical_staged).then_some(&self.boundary)
    }
    fn step_committed(&mut self) {
        self.complete |= self.staged && !self.flow;
        self.staged = false;
        self.canonical_staged = false;
    }
    fn cancel(&mut self) {
        self.complete = true;
        self.staged = false;
        self.canonical_staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
