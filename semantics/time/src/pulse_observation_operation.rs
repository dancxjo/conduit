//! Shared finite kernel operation for ordered nominal pulse observations.

use alloc::vec::Vec;
use conduit_kernel::{
    Failure, FailureCode, Operation, OperationAction, OperationInput, PortId, ValueRef,
};

use crate::{
    decode_tick, PulseObservationConfiguration, PulseObservationRefusal, TICK_ENCODED_LEN,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Prepared,
    Ready,
    Emitting,
    Terminal,
    Cancelled,
}

/// A host-neutral lifecycle whose output values were admitted before Play.
pub struct PulseObservationOperation {
    configuration: PulseObservationConfiguration,
    outputs: Vec<ValueRef>,
    next: u32,
    lifecycle: Lifecycle,
}

impl PulseObservationOperation {
    pub fn from_prepared_outputs(
        configuration: PulseObservationConfiguration,
        outputs: Vec<ValueRef>,
    ) -> Result<Self, &'static str> {
        if outputs.len() != usize::from(configuration.maximum_pulses) {
            return Err("prepared pulse outputs differ from configured capacity");
        }
        Ok(Self {
            configuration,
            outputs,
            next: 0,
            lifecycle: Lifecycle::Prepared,
        })
    }

    pub fn allocation_capacity(&self) -> usize {
        self.outputs.capacity()
    }

    pub fn next_sequence(&self) -> u32 {
        self.next
    }
}

impl Operation for PulseObservationOperation {
    fn start(&mut self) -> OperationAction {
        if self.lifecycle != Lifecycle::Prepared {
            return failure(FailureCode::InvalidLifecycle, 0);
        }
        self.lifecycle = Lifecycle::Ready;
        OperationAction::Await
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        if self.lifecycle == Lifecycle::Cancelled {
            return failure(FailureCode::Cancelled, 484);
        }
        if self.lifecycle != Lifecycle::Ready {
            return failure(FailureCode::InvalidLifecycle, 1);
        }
        if port != PortId(0) {
            return failure(FailureCode::InvalidPort, 480);
        }
        if value.byte_len != TICK_ENCODED_LEN || canonical.len() != TICK_ENCODED_LEN as usize {
            return failure(FailureCode::InvalidInput, 481);
        }
        let sequence = decode_tick(canonical).expect("exact tick length checked");
        match self.configuration.observe(self.next, sequence) {
            Ok(_) => {}
            Err(PulseObservationRefusal::Exhausted) => {
                return failure(FailureCode::StorageExhausted, 483)
            }
            Err(PulseObservationRefusal::UnexpectedSequence { .. }) => {
                return failure(FailureCode::InvalidInput, 482)
            }
            Err(PulseObservationRefusal::Configuration) => {
                return failure(FailureCode::InvalidInput, 485)
            }
        }
        self.lifecycle = Lifecycle::Emitting;
        OperationAction::Emit {
            port: PortId(0),
            value: self.outputs[self.next as usize],
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        if self.lifecycle == Lifecycle::Cancelled {
            return failure(FailureCode::Cancelled, 484);
        }
        if self.lifecycle != Lifecycle::Ready {
            return failure(FailureCode::InvalidLifecycle, 1);
        }
        match input {
            OperationInput::Closed { port: PortId(0) } => {
                self.lifecycle = Lifecycle::Terminal;
                OperationAction::Complete
            }
            _ => failure(FailureCode::InvalidLifecycle, 2),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.lifecycle == Lifecycle::Cancelled {
            return failure(FailureCode::Cancelled, 484);
        }
        if self.lifecycle != Lifecycle::Emitting {
            return failure(FailureCode::InvalidLifecycle, 3);
        }
        self.next += 1;
        self.lifecycle = Lifecycle::Ready;
        OperationAction::Await
    }

    fn cancel(&mut self) {
        self.lifecycle = Lifecycle::Cancelled;
    }
}

fn failure(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
