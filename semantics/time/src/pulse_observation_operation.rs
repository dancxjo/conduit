//! Shared finite kernel operation for ordered nominal pulse observations.

use conduit_kernel::{
    CanonicalValue, Failure, FailureCode, Operation, OperationAction, OperationInput, PortId,
    ValueRef,
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
    next: u32,
    lifecycle: Lifecycle,
}

impl PulseObservationOperation {
    pub fn new(configuration: PulseObservationConfiguration) -> Self {
        Self {
            configuration,
            next: 0,
            lifecycle: Lifecycle::Prepared,
        }
    }

    pub fn allocation_capacity(&self) -> usize {
        0
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
            Err(PulseObservationRefusal::UnexpectedSequence { .. }) => {
                return failure(FailureCode::InvalidInput, 482)
            }
            Err(PulseObservationRefusal::Configuration) => {
                return failure(FailureCode::InvalidInput, 485)
            }
        }
        self.lifecycle = Lifecycle::Emitting;
        let observation = self
            .configuration
            .observe(self.next, sequence)
            .expect("validated ordered pulse");
        OperationAction::EmitCanonical {
            port: PortId(0),
            value: CanonicalValue::new(&crate::encode_pulse_observation(observation))
                .expect("pulse observation has a fixed encoding"),
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
        let Some(next) = self.next.checked_add(1) else {
            return failure(FailureCode::IdentityCapacityExhausted, 483);
        };
        self.next = next;
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
