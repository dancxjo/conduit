use conduit_environment_runtime::CaptureBack;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, PortId,
};
use std::cell::{Cell, RefCell};
pub struct Observed {
    pub count: Cell<usize>,
    pub bytes: RefCell<([u8; 4096], usize)>,
}
impl Observed {
    pub fn new() -> Self {
        Self {
            count: Cell::new(0),
            bytes: RefCell::new(([0; 4096], 0)),
        }
    }
}
pub enum Back<'a> {
    Native(CaptureBack),
    Begin {
        sent: bool,
        staged: bool,
    },
    Observe {
        observed: &'a Observed,
        pending: Box<[u8; 4096]>,
        length: usize,
        staged: bool,
    },
}
impl<const P: usize> StepBack<P> for Back<'_> {
    fn step(&mut self, io: &mut StepIo<P>, input: &StepInputBytes<'_, P>) -> StepOutcome {
        match self {
            Self::Native(back) => back.step(io, input),
            Self::Begin { sent, staged } => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send_canonical(PortId(0), CanonicalValue::new(&[]).unwrap())
                    .unwrap();
                *staged = true;
                StepOutcome::Progress
            }
            Self::Observe {
                pending,
                length,
                staged,
                ..
            } => {
                if io.input(PortId(0)).is_some() {
                    let bytes = input.input(PortId(0)).unwrap();
                    *length = bytes.len();
                    pending[..*length].copy_from_slice(bytes);
                    io.consume(PortId(0)).unwrap();
                    *staged = true;
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match self {
            Self::Native(back) => <CaptureBack as StepBack<P>>::prepared_output(back, port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Native(back) => <CaptureBack as StepBack<P>>::step_committed(back),
            Self::Begin { sent, staged } => {
                if *staged {
                    *sent = true;
                    *staged = false
                }
            }
            Self::Observe {
                observed,
                pending,
                length,
                staged,
            } => {
                if *staged {
                    let mut output = observed.bytes.borrow_mut();
                    output.0[..*length].copy_from_slice(&pending[..*length]);
                    output.1 = *length;
                    observed.count.set(observed.count.get() + 1);
                    *staged = false
                }
            }
        }
    }
    fn cancel(&mut self) {
        if let Self::Native(back) = self {
            <CaptureBack as StepBack<P>>::cancel(back)
        }
    }
}
