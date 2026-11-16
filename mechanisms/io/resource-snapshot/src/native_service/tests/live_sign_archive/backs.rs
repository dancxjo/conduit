//! Synthetic sampling/sink Backs around the unchanged actual service Back.
use super::*;

pub(super) struct Sampling {
    // Owns APPEND_OPERATIONS-next unused references plus this input's exact
    // current pending owner; all references are prepared before kernel use.
    pub(super) empty: BoundedValueRef,
    pub(super) next: u32,
    pub(super) pending: bool,
    pub(super) candidate: Option<bool>,
    pub(super) completed: u32,
}
pub(super) struct Sink {
    pub(super) candidate: Option<u32>,
    pub(super) last_index: Option<u32>,
    pub(super) completed: u32,
}
pub(super) enum Back {
    Store(Box<RecordServiceBack<2, READ_BYTES>>),
    Sampling(Sampling),
    Sink(Sink),
}
impl StepBack<2> for Back {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; 2] {
        match self {
            Self::Store(back) => back.terminal_transductions(),
            _ => [None; 2],
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Store(back) => back.prepared_output(port),
            _ => None,
        }
    }
    fn step(&mut self, io: &mut StepIo<2>, bytes: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Store(back) => back.step(io, bytes),
            Self::Sampling(back) => {
                back.candidate = None;
                if let Some((request, outcome)) = io.host_completion() {
                    assert_eq!(request.0 + 1, back.next);
                    assert_eq!(outcome.disposition, HostCallDisposition::Completed);
                    let output = outcome.output.unwrap().value;
                    if !io.output_ready_for(PortId(0), output.byte_len) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion().unwrap();
                    io.send(PortId(0), output).unwrap();
                    back.candidate = Some(false);
                    StepOutcome::Progress
                } else if back.pending {
                    StepOutcome::Await
                } else if back.next == APPEND_OPERATIONS {
                    StepOutcome::Complete
                } else {
                    io.request_host_call(RequestId(back.next), HostCallId(2), back.empty)
                        .unwrap();
                    back.candidate = Some(true);
                    StepOutcome::Progress
                }
            }
            Self::Sink(back) => {
                back.candidate = None;
                if io.input(PortId(0)).is_none() {
                    return StepOutcome::Await;
                }
                let decoded =
                    crate::append_result::decode(bytes.input(PortId(0)).unwrap()).unwrap();
                let crate::append_result::DecodedAppendResult::Recorded(index) = decoded else {
                    panic!("unexpected append outcome {decoded:?}");
                };
                io.consume(PortId(0)).unwrap();
                back.candidate = Some(index);
                StepOutcome::Progress
            }
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Store(back) => back.step_committed(),
            Self::Sampling(back) => {
                if let Some(pending) = back.candidate.take() {
                    back.pending = pending;
                    if pending {
                        back.next += 1;
                    } else {
                        back.completed += 1;
                    }
                }
            }
            Self::Sink(back) => {
                if let Some(index) = back.candidate.take() {
                    back.last_index = Some(index);
                    back.completed += 1;
                }
            }
        }
    }
}
