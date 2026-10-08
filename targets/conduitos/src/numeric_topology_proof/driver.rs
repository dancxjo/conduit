//! Bounded prepared boundary drivers; graph and policy come from checked Source.
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use conduit_kernel::{
    Failure, FailureCode, PortId, ValueRef,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};
use core::cell::RefCell;
pub(super) const PORTS: usize =
    conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub(super) struct Output {
    pub bytes: Vec<u8>,
    pub committed: usize,
    pub staged: bool,
}
impl Output {
    pub fn prepare() -> Self {
        Self {
            bytes: Vec::with_capacity(super::storage::CELL_BYTES),
            committed: 0,
            staged: false,
        }
    }
}
pub(super) enum Driver {
    Source {
        value: ValueRef,
        sent: bool,
        staged: bool,
    },
    Operation(Box<dyn StepBack<PORTS>>),
    Sink(Rc<RefCell<Output>>),
    Inactive,
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source {
                value,
                sent,
                staged,
            } => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.send(PortId(0), *value).is_err() {
                    return refused(FailureCode::InvalidPort);
                }
                *staged = true;
                StepOutcome::Progress
            }
            Self::Operation(back) => back.step(io, bytes),
            Self::Sink(output) => {
                let mut output = output.borrow_mut();
                if output.committed != 0 {
                    if bytes.input(PortId(0)).is_some() {
                        return refused(FailureCode::InvalidLifecycle);
                    }
                    return if io.input_closed(PortId(0)) {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Await
                    };
                }
                let Some(input) = bytes.input(PortId(0)) else {
                    return if io.input_closed(PortId(0)) {
                        refused(FailureCode::InvalidInput)
                    } else {
                        StepOutcome::Await
                    };
                };
                if input.len() > output.bytes.capacity() {
                    return refused(FailureCode::StorageExhausted);
                }
                output.bytes.clear();
                output.bytes.extend_from_slice(input);
                if io.consume(PortId(0)).is_err() {
                    return refused(FailureCode::InvalidPort);
                }
                output.staged = true;
                StepOutcome::Progress
            }
            Self::Inactive => StepOutcome::Complete,
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Operation(back) => back.prepared_output(port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source { sent, staged, .. } if *staged => {
                *sent = true;
                *staged = false;
            }
            Self::Operation(back) => back.step_committed(),
            Self::Sink(output) => {
                let mut output = output.borrow_mut();
                if output.staged {
                    output.committed += 1;
                    output.staged = false;
                }
            }
            _ => {}
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Operation(back) => back.cancel(),
            Self::Sink(output) => output.borrow_mut().staged = false,
            _ => {}
        }
    }
}

fn refused(code: FailureCode) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail: 0 })
}
