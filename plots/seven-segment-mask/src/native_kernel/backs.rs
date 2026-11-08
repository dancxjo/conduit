use super::*;
/// Workload-specific Backs. Routing, fan-out retention, pressure, Host Calls and
/// terminal propagation are exclusively performed by the shared scheduler.
pub enum MaskBack {
    Tee,
    Renderer {
        state: u8,
        maximum_input_bytes: u32,
        next_request: u32,
    },
    ReadOnly {
        face: bool,
    },
}
impl StepBack<2> for MaskBack {
    fn step(&mut self, io: &mut StepIo<2>, _: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Tee => {
                if let Some(value) = io.input(k::PortId(0)) {
                    if !io.output_ready_for(k::PortId(0), value.byte_len) {
                        return StepOutcome::Await;
                    }
                    if io.consume(k::PortId(0)).is_err() || io.send(k::PortId(0), value).is_err() {
                        return failed();
                    }
                    StepOutcome::Progress
                } else if io.input_closed(k::PortId(0)) {
                    if io.consume_closed(k::PortId(0)).is_err() {
                        return failed();
                    }
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::ReadOnly { face } => {
                if !*face && io.input(k::PortId(0)).is_some() {
                    if io.consume(k::PortId(0)).is_err() {
                        return failed();
                    }
                    *face = true;
                    StepOutcome::Progress
                } else if *face && io.input(k::PortId(1)).is_some() {
                    if io.consume(k::PortId(1)).is_err() {
                        return failed();
                    }
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Renderer {
                state,
                maximum_input_bytes,
                next_request,
            } => match *state {
                0 => {
                    let Some(value) = io.input(k::PortId(0)) else {
                        return StepOutcome::Await;
                    };
                    let Ok(input) = k::BoundedValueRef::new(value, *maximum_input_bytes) else {
                        return failed();
                    };
                    if io.consume(k::PortId(0)).is_err()
                        || io
                            .request_host_call(k::RequestId(*next_request), k::HostCallId(0), input)
                            .is_err()
                    {
                        return failed();
                    }
                    *state = 1;
                    StepOutcome::Progress
                }
                1 => {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    let Some(output) = outcome.output else {
                        return failed();
                    };
                    if !io.output_ready_for(k::PortId(0), output.value.byte_len) {
                        return StepOutcome::Await;
                    }
                    if request != k::RequestId(*next_request)
                        || outcome.disposition != k::HostCallDisposition::Completed
                        || outcome.failure.is_some()
                        || io.consume_host_completion().is_err()
                        || io.send(k::PortId(0), output.value).is_err()
                    {
                        return failed();
                    }
                    let Some(next) = next_request.checked_add(1) else {
                        return failed();
                    };
                    *next_request = next;
                    *state = 2;
                    StepOutcome::Progress
                }
                _ => StepOutcome::Complete,
            },
        }
    }
    fn cancel(&mut self) {}
}
fn failed() -> StepOutcome {
    StepOutcome::Fail(k::Failure {
        code: k::FailureCode::InvalidLifecycle,
        detail: 60,
    })
}

// Only the pure routing/closure state is rearmed. Renderer request generations
// survive. The session owner separately requires drained queues, accepted output,
// transferred evidence, and unchanged exact Plan/provider custody.
impl prepared_session::ReusableStepBack<2> for MaskBack {
    type Next = ();
    fn preflight_next(&self, _: &(), _: &dyn k::ValueStorage) -> bool {
        match self {
            Self::Tee => true,
            Self::Renderer {
                state,
                next_request,
                ..
            } => *state == 2 && *next_request < u32::MAX,
            Self::ReadOnly { face } => *face,
        }
    }
    fn begin_next(&mut self, _: ()) {
        match self {
            Self::Tee => (),
            Self::Renderer { state, .. } => *state = 0,
            Self::ReadOnly { face } => *face = false,
        }
    }
}
