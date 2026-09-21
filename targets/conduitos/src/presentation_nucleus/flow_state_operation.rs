//! Backs owned only by the bounded `state/latest > flow/tee` proof.

use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub(super) enum FlowStateBack {
    Source { value: ValueRef, emitted: bool },
    Latest { held: Option<ValueRef> },
    Tee,
    Sink { pending: bool, complete: bool },
}

impl StepBack<PORTS> for FlowStateBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value, emitted } => {
                if *emitted {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.send(PortId(0), *value).is_err() {
                    return invalid(50);
                }
                *emitted = true;
                StepOutcome::Complete
            }
            Self::Latest { held } => step_latest(held, io),
            Self::Tee => step_tee(io),
            Self::Sink { pending, complete } => step_sink(pending, complete, io),
        }
    }

    fn cancel(&mut self) {
        if let Self::Latest { held } = self {
            *held = None;
        }
    }
}

fn step_latest(held: &mut Option<ValueRef>, io: &mut StepIo<PORTS>) -> StepOutcome {
    if let Some(value) = io.input(PortId(0)) {
        if value.byte_len != conduit_core::SCALAR_ENCODED_LEN as u32 {
            return invalid(50);
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.take_input(PortId(0)).is_err() {
            return invalid(50);
        }
        if let Some(previous) = held.replace(value)
            && io.discard(previous).is_err()
        {
            return invalid(50);
        }
        if io.send(PortId(0), value).is_err() {
            return invalid(50);
        }
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(50);
        }
        if let Some(value) = held.take()
            && io.discard(value).is_err()
        {
            return invalid(50);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn step_tee(io: &mut StepIo<PORTS>) -> StepOutcome {
    if let Some(value) = io.input(PortId(0)) {
        if value.byte_len != conduit_core::SCALAR_ENCODED_LEN as u32 {
            return invalid(50);
        }
        if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err()
            || io.send(PortId(0), value).is_err()
            || io.send(PortId(1), value).is_err()
        {
            return invalid(50);
        }
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(50);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn step_sink(pending: &mut bool, complete: &mut bool, io: &mut StepIo<PORTS>) -> StepOutcome {
    if *pending {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if request != RequestId(0)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
            || io.consume_host_completion().is_err()
        {
            return invalid(50);
        }
        *pending = false;
        *complete = true;
        return StepOutcome::Progress;
    }
    if let Some(value) = io.input(PortId(0)) {
        let Ok(input) = BoundedValueRef::new(value, conduit_core::SCALAR_ENCODED_LEN as u32) else {
            return invalid(51);
        };
        if io.consume(PortId(0)).is_err()
            || io
                .request_host_call(RequestId(0), HostCallId(0), input)
                .is_err()
        {
            return invalid(50);
        }
        *pending = true;
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) && *complete {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(50);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
