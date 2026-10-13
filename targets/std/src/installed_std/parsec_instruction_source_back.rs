//! One original completion ValueRef forwarded after the actual Show branch arrives.
use crate::flow_activation::parsec_instruction_source_offer as source;
use conduit_core::{PlannedGear, PreparedStructuredValueValidator};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(crate) struct ParsecInstructionSourceBack {
    pending: bool,
    emitted: bool,
    cancelled: bool,
    validator: PreparedStructuredValueValidator,
}
impl ParsecInstructionSourceBack {
    pub(crate) fn prepare(placement: &PlannedGear) -> Result<Self, String> {
        source::validate_placement(placement)?;
        Ok(Self {
            pending: false,
            emitted: false,
            cancelled: false,
            validator: PreparedStructuredValueValidator::new(
                &source::instruction_native_type()?.value_type,
                source::MAXIMUM_INSTRUCTION_BYTES as usize,
            )
            .map_err(|e| format!("instruction validator: {e:?}"))?,
        })
    }
}
fn failed(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
impl<const PORTS: usize> StepBack<PORTS> for ParsecInstructionSourceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled || self.emitted {
            return StepOutcome::Complete;
        }
        if self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != RequestId(0)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return failed(FailureCode::InvalidLifecycle, 1);
            }
            let (Some(output), Some(canonical)) = (outcome.output, bytes.host_output()) else {
                return failed(FailureCode::InvalidInput, 2);
            };
            if output.value.byte_len as usize != canonical.len()
                || self.validator.validate(canonical).is_err()
            {
                return failed(FailureCode::InvalidInput, 3);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err() || io.send(PortId(0), output.value).is_err() {
                return failed(FailureCode::InvalidLifecycle, 4);
            }
            self.pending = false;
            self.emitted = true;
            return StepOutcome::Progress;
        }
        if let Some(shown) = io.input(PortId(0)) {
            let Ok(shown) = BoundedValueRef::new(shown, source::MAXIMUM_SHOW_BYTES) else {
                return failed(FailureCode::InvalidInput, 5);
            };
            if io.consume(PortId(0)).is_err()
                || io
                    .request_host_call(RequestId(0), HostCallId(0), shown)
                    .is_err()
            {
                return failed(FailureCode::InvalidLifecycle, 6);
            }
            self.pending = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.pending = false;
    }
}
