//! Tick bytes are validated at the kernel value boundary; all outputs exist before Play.
use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    Failure, FailureCode, HostedValueStore, OperationAction, OperationInput, PortId, ValueRef,
    ValueStorage,
};
use conduit_time::{
    PulseObservationConfiguration, PulseObservationRefusal, PULSE_OBSERVATION_ENCODED_LEN,
    TICK_ENCODED_LEN,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::PULSE_OBSERVE_IMPLEMENTATION,
    budget,
    prepare,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Prepared,
    Ready,
    Emitting,
    Terminal,
    Cancelled,
}

pub(super) struct PulseObservationOperation {
    configuration: PulseObservationConfiguration,
    outputs: Vec<ValueRef>,
    next: u32,
    lifecycle: Lifecycle,
}

impl PulseObservationOperation {
    pub(super) fn allocation_capacity(&self) -> usize {
        self.outputs.capacity()
    }

    pub(super) fn start(&mut self) -> OperationAction {
        if self.lifecycle != Lifecycle::Prepared {
            return failure(FailureCode::InvalidLifecycle, 0);
        }
        self.lifecycle = Lifecycle::Ready;
        OperationAction::Await
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
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
        let sequence = conduit_time::decode_tick(canonical).expect("exact tick length checked");
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

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
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
            // Value bytes must cross resume_value; a ValueRef alone is not semantic proof.
            _ => failure(FailureCode::InvalidLifecycle, 2),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.lifecycle == Lifecycle::Cancelled {
            return failure(FailureCode::Cancelled, 484);
        }
        if self.lifecycle != Lifecycle::Emitting {
            return failure(FailureCode::InvalidLifecycle, 3);
        }
        // The driver stages this output once. Its pending transaction retains
        // the output under pressure and prevents another input from being consumed.
        self.next += 1;
        self.lifecycle = Lifecycle::Ready;
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.lifecycle = Lifecycle::Cancelled;
    }
}

fn failure(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

fn validate(placement: &PlannedGear) -> Result<PulseObservationConfiguration, String> {
    let offer = conduit_std_offers::pulse_observe_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.limits != offer.limits
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned pulse observation identity does not match installation".into());
    }
    PulseObservationConfiguration::parse(&placement.configuration)
        .map_err(|error| format!("invalid pulse observation configuration: {error:?}"))
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let configuration = validate(placement)?;
    Ok(OperationBudget {
        value_items: configuration.maximum_pulses,
        value_bytes: u32::from(configuration.maximum_pulses) * PULSE_OBSERVATION_ENCODED_LEN as u32,
        host_requests: 0,
        sign_items: configuration.maximum_pulses * 8 + 16,
        maximum_value_bytes: TICK_ENCODED_LEN,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let configuration = validate(placement)?;
    let mut outputs = Vec::with_capacity(configuration.maximum_pulses.into());
    for sequence in 0..u32::from(configuration.maximum_pulses) {
        let observation = configuration
            .observe(sequence, sequence.into())
            .expect("admitted sequence");
        outputs.push(
            values
                .store(&conduit_time::encode_pulse_observation(observation))
                .map_err(|error| format!("store pulse observation: {error:?}"))?,
        );
    }
    Ok(InstalledOperation::PulseObserve(
        PulseObservationOperation {
            configuration,
            outputs,
            next: 0,
            lifecycle: Lifecycle::Prepared,
        },
    ))
}

#[cfg(test)]
#[path = "pulse_observation_tests.rs"]
mod tests;
