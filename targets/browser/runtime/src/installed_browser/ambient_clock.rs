//! Browser realization of the standing clock/weather observation Fore.
use super::{
    factory::{validate_placement, BrowserInstallation},
    BrowserBack,
};
use crate::browser_civil_clock::{observation::PreparedObservation, *};
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, Back, BackOfferBuilder, CapabilityId,
    CapabilityOffer, ConfigurationValue, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId, PlannedGear, RUNTIME_MEMORY_RESOURCE_CLASS,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostCallDisposition, HostCallId,
    HostedValueStore, PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_semantic_catalog as catalog;

const IMPLEMENTATION: &str = "browser/ambient-clock-weather@1";
const PREPARED_BYTES: u32 = 16_384;

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        catalog::ambient_clock_kind().expect("checked ambient clock Kind"),
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/installed-ambient-clock@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(CIVIL_CLOCK_OPERATION),
                target_kind: Some(kind_id(catalog::AMBIENT_CLOCK_KIND)),
                maximum_in_flight: 1,
                maximum_input_bytes: 0,
                maximum_output_bytes: CIVIL_CLOCK_REPLY_BYTES as u32,
            }],
            resource_requirements: vec![
                resource_requirement(CIVIL_CLOCK_RESOURCE, 1),
                resource_requirement(RUNTIME_MEMORY_RESOURCE_CLASS, PREPARED_BYTES),
            ],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn prepare(placement: &PlannedGear, values: &mut HostedValueStore) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    let timezone = placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            ("timezone", ConfigurationValue::Text(value)) if !value.is_empty() => {
                Some(value.clone())
            }
            _ => None,
        })
        .ok_or("missing planned clock timezone")?;
    let observation = PreparedObservation::prepare(timezone)?;
    if observation.prepared_heap_bytes() + core::mem::size_of::<ClockBack>()
        > PREPARED_BYTES as usize
    {
        return Err("clock observation preparation exceeded admitted storage".into());
    }
    let empty = values
        .store(&[])
        .map_err(|error| format!("prepare clock request: {error:?}"))?;
    Ok(BrowserBack::installed_step(ClockBack {
        observation,
        empty,
        sequence: 0,
        pending: false,
    }))
}

struct ClockBack {
    observation: PreparedObservation,
    empty: ValueRef,
    sequence: u32,
    pending: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for ClockBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let request = RequestId(self.sequence);
        if let Some((completed, outcome)) = io.host_completion() {
            if !self.pending
                || request != completed
                || outcome.disposition != HostCallDisposition::Completed
            {
                return outcome.failure.map_or_else(invalid, StepOutcome::Fail);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(reply) = bytes
                .host_output()
                .and_then(|output| CivilClockReply::decode(output).ok())
            else {
                return invalid();
            };
            let Ok(value) = self.observation.compose(reply) else {
                return invalid();
            };
            let Ok(value) = CanonicalValue::new(value) else {
                return invalid();
            };
            io.consume_host_completion()
                .expect("observed clock completion");
            io.send_canonical(PortId(0), value)
                .expect("ready clock output");
            self.pending = false;
            return StepOutcome::Progress;
        }
        if self.pending {
            return StepOutcome::Await;
        }
        if let Some(tick) = io.input(PortId(0)) {
            if tick.byte_len != conduit_time::TICK_ENCODED_LEN {
                return invalid();
            }
            let Some(sequence) = self.sequence.checked_add(1) else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::IdentityCapacityExhausted,
                    detail: 223,
                });
            };
            io.consume(PortId(0)).expect("observed clock tick");
            io.request_host_call(
                RequestId(sequence),
                HostCallId(0),
                BoundedValueRef::new(self.empty, 0).expect("empty clock request"),
            )
            .expect("admitted clock Host Call");
            self.sequence = sequence;
            self.pending = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed clock input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

fn invalid() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 223,
    })
}
