//! Shared finite kernel operation for ordered nominal pulse observations.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, Failure, FailureCode, PortId,
};

use crate::{
    decode_tick, PulseObservationConfiguration, PulseObservationRefusal, TICK_ENCODED_LEN,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Prepared,
    Ready,
    Terminal,
    Cancelled,
}

/// A host-neutral lifecycle whose output values were admitted before Play.
pub struct PulseObservationBack {
    configuration: PulseObservationConfiguration,
    next: u32,
    lifecycle: Lifecycle,
}

impl<const PORTS: usize> StepBack<PORTS> for PulseObservationBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.lifecycle == Lifecycle::Cancelled {
            return step_failure(FailureCode::Cancelled, 484);
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.lifecycle != Lifecycle::Prepared && self.lifecycle != Lifecycle::Ready {
                return step_failure(FailureCode::InvalidLifecycle, 1);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return step_failure(FailureCode::InvalidInput, 481);
            };
            if value.byte_len != TICK_ENCODED_LEN || canonical.len() != TICK_ENCODED_LEN as usize {
                return step_failure(FailureCode::InvalidInput, 481);
            }
            let sequence = decode_tick(canonical).expect("exact tick length checked");
            let observation = match self.configuration.observe(self.next, sequence) {
                Ok(observation) => observation,
                Err(PulseObservationRefusal::UnexpectedSequence { .. }) => {
                    return step_failure(FailureCode::InvalidInput, 482)
                }
                Err(PulseObservationRefusal::Configuration) => {
                    return step_failure(FailureCode::InvalidInput, 485)
                }
            };
            let Some(next) = self.next.checked_add(1) else {
                return step_failure(FailureCode::IdentityCapacityExhausted, 483);
            };
            io.consume(PortId(0)).expect("present pulse input");
            io.send_canonical(
                PortId(0),
                CanonicalValue::new(&crate::encode_pulse_observation(observation))
                    .expect("pulse observation has a fixed encoding"),
            )
            .expect("ready pulse-observation output");
            self.next = next;
            self.lifecycle = Lifecycle::Ready;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed pulse-observation closure");
            self.lifecycle = Lifecycle::Terminal;
            return StepOutcome::Complete;
        }
        self.lifecycle = Lifecycle::Ready;
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.lifecycle = Lifecycle::Cancelled;
    }
}

const fn step_failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl PulseObservationBack {
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
