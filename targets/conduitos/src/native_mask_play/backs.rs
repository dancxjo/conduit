//! Bounded kernel Backs for the ordinary native Mask Plot.
use super::PORTS;
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId};

pub(super) enum MaskBack {
    ResourceSource,
    Tee,
    Renderer {
        pending: bool,
        emitted: bool,
        maximum_value_bytes: u32,
    },
    Interaction {
        seen_face: bool,
        pending: bool,
        maximum_value_bytes: u32,
    },
}

impl StepBack<PORTS> for MaskBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::ResourceSource => StepOutcome::Complete,
            Self::Tee => pass_one(io),
            Self::Renderer {
                pending,
                emitted,
                maximum_value_bytes,
            } => render(pending, emitted, *maximum_value_bytes, io),
            Self::Interaction {
                seen_face,
                pending,
                maximum_value_bytes,
            } => interact(seen_face, pending, *maximum_value_bytes, io),
        }
    }
}

fn pass_one(io: &mut StepIo<PORTS>) -> StepOutcome {
    if let Some(value) = io.input(PortId(0)) {
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err() || io.send(PortId(0), value).is_err() {
            return failure();
        }
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return failure();
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn render(
    pending: &mut bool,
    emitted: &mut bool,
    maximum_value_bytes: u32,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if *pending {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        let Some(output) = outcome.output else {
            return failure();
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if request != RequestId(0)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.failure.is_some()
            || io.consume_host_completion().is_err()
            || io.send(PortId(0), output.value).is_err()
        {
            return failure();
        }
        *pending = false;
        *emitted = true;
        return StepOutcome::Progress;
    }
    if !*emitted && let Some(value) = io.input(PortId(0)) {
        let Ok(value) = BoundedValueRef::new(value, maximum_value_bytes) else {
            return failure();
        };
        if io.consume(PortId(0)).is_err()
            || io
                .request_host_call(RequestId(0), HostCallId(0), value)
                .is_err()
        {
            return failure();
        }
        *pending = true;
        return StepOutcome::Progress;
    }
    if *emitted {
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn interact(
    seen_face: &mut bool,
    pending: &mut bool,
    maximum_value_bytes: u32,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if *pending {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if request != RequestId(1)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.failure.is_some()
        {
            return failure();
        }
        if let Some(output) = outcome.output {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.send(PortId(0), output.value).is_err() {
                return failure();
            }
        }
        if io.consume_host_completion().is_err() {
            return failure();
        }
        *pending = false;
        return StepOutcome::Complete;
    }
    if !*seen_face && io.input(PortId(0)).is_some() {
        if io.consume(PortId(0)).is_err() {
            return failure();
        }
        *seen_face = true;
        return StepOutcome::Progress;
    }
    if *seen_face && let Some(show) = io.input(PortId(1)) {
        let Ok(input) = BoundedValueRef::new(show, maximum_value_bytes) else {
            return failure();
        };
        if io.consume(PortId(1)).is_err()
            || io
                .request_host_call(RequestId(1), HostCallId(0), input)
                .is_err()
        {
            return failure();
        }
        *pending = true;
        return StepOutcome::Progress;
    }
    StepOutcome::Await
}

fn failure() -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail: 1,
    })
}
