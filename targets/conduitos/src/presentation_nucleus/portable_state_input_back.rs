//! Fixed-storage Backs for bounded count, toggle, and typed key fan-out.

use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub(super) enum PortableStateInputBack {
    Source {
        value: ValueRef,
        emitted: bool,
    },
    Count {
        values: [ValueRef; 2],
        next: usize,
        initial_emitted: bool,
    },
    Toggle {
        values: [ValueRef; 2],
        next: usize,
        initial_emitted: bool,
    },
    KeyTee,
    Sink {
        maximum_bytes: u32,
        pending: bool,
        next_request: u32,
    },
}

impl StepOperation<PORTS> for PortableStateInputBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source { value, emitted } => step_source(*value, emitted, io),
            Self::Count {
                values,
                next,
                initial_emitted,
            }
            | Self::Toggle {
                values,
                next,
                initial_emitted,
            } => step_state(values, next, initial_emitted, io),
            Self::KeyTee => step_key_tee(io),
            Self::Sink {
                maximum_bytes,
                pending,
                next_request,
            } => step_sink(*maximum_bytes, pending, next_request, io),
        }
    }

    fn cancel(&mut self) {
        if let Self::Sink { pending, .. } = self {
            *pending = false;
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
        return invalid(60);
    }
    *emitted = true;
    StepOutcome::Complete
}

fn step_state(
    values: &[ValueRef; 2],
    next: &mut usize,
    initial_emitted: &mut bool,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if !*initial_emitted {
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.send(PortId(0), values[0]).is_err() {
            return invalid(60);
        }
        *initial_emitted = true;
        return StepOutcome::Progress;
    }
    if let Some(value) = io.input(PortId(0)) {
        if value.byte_len != conduit_time::TICK_ENCODED_LEN || *next != 0 {
            return invalid(60);
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err() || io.send(PortId(0), values[1]).is_err() {
            return invalid(60);
        }
        *next = 1;
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(60);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn step_key_tee(io: &mut StepIo<PORTS>) -> StepOutcome {
    if let Some(value) = io.input(PortId(0)) {
        if !conduit_semantic_catalog::key_event_tee_accepts_encoded_len(value.byte_len) {
            return invalid(60);
        }
        if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err()
            || io.send(PortId(0), value).is_err()
            || io.send(PortId(1), value).is_err()
        {
            return invalid(60);
        }
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(60);
        }
        return StepOutcome::Complete;
    }
    StepOutcome::Await
}

fn step_sink(
    maximum_bytes: u32,
    pending: &mut bool,
    next_request: &mut u32,
    io: &mut StepIo<PORTS>,
) -> StepOutcome {
    if *pending {
        let Some((_request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if outcome.disposition != HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
            || io.consume_host_completion().is_err()
        {
            return invalid(60);
        }
        *pending = false;
        return StepOutcome::Progress;
    }
    if let Some(value) = io.input(PortId(0)) {
        let Ok(input) = BoundedValueRef::new(value, maximum_bytes) else {
            return invalid(61);
        };
        let request = RequestId(*next_request);
        if io.consume(PortId(0)).is_err()
            || io.request_host_call(request, HostCallId(0), input).is_err()
        {
            return invalid(60);
        }
        *next_request = next_request.saturating_add(1);
        *pending = true;
        return StepOutcome::Progress;
    }
    if io.input_closed(PortId(0)) {
        if io.consume_closed(PortId(0)).is_err() {
            return invalid(60);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_releases_pending_sink_state() {
        let mut back = PortableStateInputBack::Sink {
            maximum_bytes: 8,
            pending: true,
            next_request: 1,
        };
        StepOperation::<PORTS>::cancel(&mut back);
        assert!(matches!(
            back,
            PortableStateInputBack::Sink { pending: false, .. }
        ));
    }

    #[test]
    fn key_tee_keeps_the_exact_portable_value_shape() {
        assert!(conduit_semantic_catalog::key_event_tee_accepts_encoded_len(
            conduit_human::KEY_EVENT_ENCODED_LEN as u32
        ));
        assert!(!conduit_semantic_catalog::key_event_tee_accepts_encoded_len(1));
    }
}
