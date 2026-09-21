use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};

use super::robotics_back::{RoboticsDiscardBack, RoboticsDriveBack, RoboticsSourceBack};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub(super) enum PresentationBack {
    Source {
        value: ValueRef,
        emitted: bool,
    },
    Transform {
        maximum_input_bytes: u32,
        pending: bool,
        emitted: bool,
    },
    LogicInputs {
        input_count: u8,
        seen: u8,
        next_request: u32,
        pending: bool,
        emitted: bool,
    },
    RoboticsSource(RoboticsSourceBack),
    RoboticsDrive(RoboticsDriveBack),
    RoboticsDiscard(RoboticsDiscardBack),
    Sink {
        maximum_input_bytes: u32,
        pending: bool,
        complete: bool,
    },
}

impl StepOperation<PORTS> for PresentationBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value, emitted } => step_source(*value, emitted, io),
            Self::Transform {
                maximum_input_bytes,
                pending,
                emitted,
            } => step_transform(*maximum_input_bytes, pending, emitted, io),
            Self::LogicInputs {
                input_count,
                seen,
                next_request,
                pending,
                emitted,
            } => step_logic_inputs(*input_count, seen, next_request, pending, emitted, io),
            Self::RoboticsSource(operation) => operation.step(io),
            Self::RoboticsDrive(operation) => operation.step(io, input_bytes),
            Self::RoboticsDiscard(operation) => operation.step(io),
            Self::Sink {
                maximum_input_bytes,
                pending,
                complete,
            } => step_sink(*maximum_input_bytes, pending, complete, io),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::RoboticsSource(operation) => operation.cancel(),
            Self::RoboticsDrive(operation) => operation.cancel(),
            _ => {}
        }
    }
}

impl PresentationBack {
    pub(super) fn robotics_effect(&self) -> Option<super::robotics_back::RoboticsDriveEffect> {
        match self {
            Self::RoboticsDrive(operation) => operation.effect(),
            _ => None,
        }
    }
}

fn step_source(value: ValueRef, emitted: &mut bool, io: &mut StepIo<PORTS>) -> StepOutcome {
    if *emitted {
        return StepOutcome::Complete;
    }
    if !io.output_ready(PortId(0)) {
        return StepOutcome::Await;
    }
    if io.send(PortId(0), value).is_err() {
        return invalid(1);
    }
    *emitted = true;
    StepOutcome::Complete
}

fn step_transform(
    maximum_input_bytes: u32,
    pending: &mut bool,
    emitted: &mut bool,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if *pending {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        let Some(output) = outcome.output else {
            return invalid(3);
        };
        if request != RequestId(0)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.failure.is_some()
            || !io.output_ready(PortId(0))
            || io.consume_host_completion().is_err()
            || io.send(PortId(0), output.value).is_err()
        {
            return invalid(3);
        }
        *pending = false;
        *emitted = true;
        return StepOutcome::Progress;
    }
    if !*emitted && let Some(value) = io.input(PortId(0)) {
        let Ok(input) = BoundedValueRef::new(value, maximum_input_bytes) else {
            return invalid(1);
        };
        if io.consume(PortId(0)).is_err()
            || io
                .request_host_call(RequestId(0), HostCallId(0), input)
                .is_err()
        {
            return invalid(1);
        }
        *pending = true;
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(4);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn step_logic_inputs(
    input_count: u8,
    seen: &mut u8,
    next_request: &mut u32,
    pending: &mut bool,
    emitted: &mut bool,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if *emitted {
        return StepOutcome::Complete;
    }
    if *pending {
        let Some((_request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
            return invalid(4);
        }
        if let Some(output) = outcome.output {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err() || io.send(PortId(0), output.value).is_err() {
                return invalid(4);
            }
            *pending = false;
            *emitted = true;
            return StepOutcome::Progress;
        }
        if io.consume_host_completion().is_err() {
            return invalid(4);
        }
        *pending = false;
        return StepOutcome::Progress;
    }
    for index in 0..usize::from(input_count) {
        let port = PortId(index as u16);
        let Some(value) = io.input(port) else {
            continue;
        };
        let bit = 1_u8 << port.0;
        if *seen & bit != 0 {
            return invalid(5);
        }
        let Ok(input) = BoundedValueRef::new(value, conduit_core::SCALAR_ENCODED_LEN as u32) else {
            return invalid(5);
        };
        let request = RequestId(*next_request * 4 + u32::from(port.0));
        if io.consume(port).is_err() || io.request_host_call(request, HostCallId(0), input).is_err()
        {
            return invalid(5);
        }
        *seen |= bit;
        *next_request += 1;
        *pending = true;
        return StepOutcome::Progress;
    }
    for index in 0..usize::from(input_count) {
        let port = PortId(index as u16);
        if io.input_closed(port) && *seen & (1_u8 << port.0) == 0 {
            if io.consume_closed(port).is_err() {
                return invalid(4);
            }
            return StepOutcome::Complete;
        }
    }
    StepOutcome::Await
}

fn step_sink(
    maximum_input_bytes: u32,
    pending: &mut bool,
    complete: &mut bool,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if *pending {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if request != RequestId(0)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.failure.is_some()
            || outcome.output.is_some()
            || io.consume_host_completion().is_err()
        {
            return invalid(2);
        }
        *pending = false;
        *complete = true;
        return StepOutcome::Progress;
    }
    if let Some(value) = io.input(PortId(0)) {
        let Ok(input) = BoundedValueRef::new(value, maximum_input_bytes) else {
            return invalid(2);
        };
        if io.consume(PortId(0)).is_err()
            || io
                .request_host_call(RequestId(0), HostCallId(0), input)
                .is_err()
        {
            return invalid(2);
        }
        *pending = true;
        return StepOutcome::Progress;
    }
    if *complete && io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(4);
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
