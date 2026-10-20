//! Prepared fixed-capacity fixture endpoints and numeric timing observer.
use super::PORTS;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId as KPort, ValueRef,
};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};
#[derive(Default, Debug)]
pub struct WorkTiming {
    pub numeric: Duration,
    pub numeric_steps: u64,
    pub kernel_steps: Duration,
    pub steps: u64,
    pub expressions: Duration,
    pub expression_calls: u64,
    pub local_expressions: Duration,
    pub local_expression_calls: u64,
    pub signs: usize,
    pub scheduler_bytes: usize,
    pub canonical_values: u64,
    pub canonical_bytes: u64,
    pub compact_values: u64,
    pub compact_bytes: u64,
    pub compact_logical_bytes: u64,
    pub logical_invocations: u64,
    pub physical_decisions: u32,
    pub compact_binding_bytes: usize,
}
pub(super) struct TimedNumeric<B: StepBack<PORTS> + ?Sized> {
    pub(super) back: Box<B>,
    pub(super) timing: Rc<std::cell::RefCell<WorkTiming>>,
}
impl<B: StepBack<PORTS> + ?Sized> StepBack<PORTS> for TimedNumeric<B> {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let start = Instant::now();
        let result = self.back.step(io, bytes);
        let elapsed = start.elapsed();
        let mut timing = self.timing.borrow_mut();
        timing.numeric += elapsed;
        timing.numeric_steps += 1;
        result
    }
    fn compact_f32_input(
        &self,
        port: KPort,
    ) -> Option<conduit_kernel::scheduler::CompactF32Layout> {
        self.back.compact_f32_input(port)
    }
    fn compact_f32_output(&self) -> Option<(KPort, conduit_kernel::scheduler::CompactF32Layout)> {
        self.back.compact_f32_output()
    }
    fn prepared_output(&self, port: KPort) -> Option<&[u8]> {
        self.back.prepared_output(port)
    }
    fn step_committed(&mut self) {
        self.back.step_committed();
    }
    fn cancel(&mut self) {
        self.back.cancel();
    }
}
pub(super) enum Driver {
    Source {
        reference: ValueRef,
        staged: bool,
        sent: bool,
    },
    Operation(Box<dyn StepBack<PORTS>>),
    ReusableOperation(
        Box<dyn conduit_kernel::scheduler::prepared_session::ReusableStepBack<PORTS, Next = ()>>,
    ),
    Sink {
        received: Rc<std::cell::RefCell<Option<Vec<u8>>>>,
        staged: Option<Vec<u8>>,
    },
    Inactive,
}
impl StepBack<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source {
                reference,
                staged,
                sent,
            } => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(KPort(0)) {
                    return StepOutcome::Await;
                }
                io.send(KPort(0), *reference).unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Operation(back) => back.step(io, bytes),
            Self::ReusableOperation(back) => back.step(io, bytes),
            Self::Sink { staged, .. } => {
                let Some(input) = bytes.input(KPort(0)) else {
                    return StepOutcome::Await;
                };
                let buffer = staged.as_mut().expect("prepared result storage");
                assert!(input.len() <= buffer.capacity());
                buffer.clear();
                buffer.extend_from_slice(input);
                io.consume(KPort(0)).unwrap();
                StepOutcome::Complete
            }
            Self::Inactive => StepOutcome::Complete,
        }
    }
    fn compact_f32_input(
        &self,
        port: KPort,
    ) -> Option<conduit_kernel::scheduler::CompactF32Layout> {
        match self {
            Self::Operation(back) => back.compact_f32_input(port),
            Self::ReusableOperation(back) => back.compact_f32_input(port),
            _ => None,
        }
    }
    fn compact_f32_output(&self) -> Option<(KPort, conduit_kernel::scheduler::CompactF32Layout)> {
        match self {
            Self::Operation(back) => back.compact_f32_output(),
            Self::ReusableOperation(back) => back.compact_f32_output(),
            _ => None,
        }
    }
    fn prepared_output(&self, p: KPort) -> Option<&[u8]> {
        match self {
            Self::Operation(back) => back.prepared_output(p),
            Self::ReusableOperation(back) => back.prepared_output(p),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Operation(back) => back.step_committed(),
            Self::ReusableOperation(back) => back.step_committed(),
            Self::Sink {
                received, staged, ..
            } => {
                *received.borrow_mut() = staged.take();
            }
            Self::Source { staged, sent, .. } if *staged => {
                *sent = true;
                *staged = false;
            }
            _ => {}
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Operation(back) => back.cancel(),
            Self::ReusableOperation(back) => back.cancel(),
            _ => {}
        }
    }
}

#[path = "runtime_rearm.rs"]
mod rearm;
pub(super) use rearm::Next;
