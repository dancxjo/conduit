use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub(super) enum NucleusBack {
    Source {
        value: ValueRef,
        emitted: bool,
    },
    Transform {
        maximum_input_bytes: u32,
        pending: bool,
        emitted: bool,
    },
    Sink {
        maximum_input_bytes: u32,
        pending: bool,
        complete: bool,
    },
}

impl StepBack<PORTS> for NucleusBack {
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
                    return fail(4);
                }
                *emitted = true;
                StepOutcome::Complete
            }
            Self::Transform {
                maximum_input_bytes,
                pending,
                emitted,
            } => {
                if *pending {
                    let Some((RequestId(0), outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                    {
                        return fail(4);
                    }
                    let Some(output) = outcome.output else {
                        return fail(3);
                    };
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    if io.consume_host_completion().is_err()
                        || io.send(PortId(0), output.value).is_err()
                    {
                        return fail(4);
                    }
                    *pending = false;
                    *emitted = true;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    if *emitted {
                        return fail(4);
                    }
                    let input = BoundedValueRef::new(value, *maximum_input_bytes)
                        .expect("portable presentation value is bounded");
                    if io.consume(PortId(0)).is_err()
                        || io
                            .request_host_call(RequestId(0), HostCallId(0), input)
                            .is_err()
                    {
                        return fail(4);
                    }
                    *pending = true;
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    if io.consume_closed(PortId(0)).is_err() {
                        return fail(4);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
            Self::Sink {
                maximum_input_bytes,
                pending,
                complete,
            } => {
                if *pending {
                    let Some((RequestId(0), outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                        || outcome.output.is_some()
                        || io.consume_host_completion().is_err()
                    {
                        return fail(4);
                    }
                    *pending = false;
                    *complete = true;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    let input = BoundedValueRef::new(value, *maximum_input_bytes)
                        .expect("fixture manifestation value is bounded");
                    if io.consume(PortId(0)).is_err()
                        || io
                            .request_host_call(RequestId(0), HostCallId(0), input)
                            .is_err()
                    {
                        return fail(4);
                    }
                    *pending = true;
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) && *complete {
                    if io.consume_closed(PortId(0)).is_err() {
                        return fail(4);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
