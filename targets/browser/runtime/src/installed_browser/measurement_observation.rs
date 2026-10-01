//! Exact Quantity-to-MeasurementSample work in the ordinary browser kernel.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, ConfigurationValue, ExecutionProfileId,
    HostCallRequirement, ImplementationId, PlannedGear, StructuredInfoValue, TemporalInstant,
    TemporalScale,
};
use conduit_data::MeasurementSample;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(crate) const HOST_CALL: &str = "conduit.host/measurement-observation@1";
const IMPLEMENTATION: &str = "browser/kernel-measurement-observation@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: Some(perform),
};

fn offer() -> conduit_core::CapabilityOffer {
    let contract = conduit_data::measurement_observation_semantic_contract();
    let kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/measurement-observation@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HOST_CALL.into(),
                target_kind: Some(kind),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
                maximum_output_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    configuration(placement)?;
    Ok(BrowserBack::installed_step(ObservationBack {
        pending: false,
        completed: false,
    }))
}

fn configuration(placement: &PlannedGear) -> Result<TemporalInstant, String> {
    let text = |key: &str| {
        placement
            .configuration
            .iter()
            .find_map(|field| match &field.value {
                ConfigurationValue::Text(value) if field.key == key => Some(value.clone()),
                _ => None,
            })
    };
    let clock_basis = text("clock-basis").ok_or("measurement observation lacks clock basis")?;
    if clock_basis.is_empty()
        || clock_basis.len() > conduit_data::MAXIMUM_MEASUREMENT_CLOCK_BASIS_BYTES as usize
    {
        return Err("measurement observation clock basis is outside its bound".into());
    }
    let instant = TemporalInstant {
        ticks: 1,
        scale: TemporalScale::Milliseconds,
        clock_basis,
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    instant
        .validate()
        .map_err(|error| format!("measurement observation instant: {error:?}"))?;
    Ok(instant)
}

fn perform(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let quantity = conduit_core::Quantity::decode(input)
        .map_err(|error| format!("measurement observation quantity: {error:?}"))?;
    let sample = MeasurementSample {
        value: quantity,
        observed_at: configuration(placement)?,
        uncertainty: None,
    };
    let payload = conduit_data::encode_measurement_sample(&sample)
        .map_err(|error| format!("encode measurement observation: {error:?}"))?;
    let output = StructuredInfoValue::leaf(conduit_data::measurement_sample_type(), payload)
        .map_err(|error| format!("construct measurement observation: {error:?}"))?
        .canonical_bytes()
        .map_err(|error| format!("encode measurement observation envelope: {error:?}"))?;
    Ok(BrowserHostResult {
        output: Some(output),
        manifestation: None,
    })
}

struct ObservationBack {
    pending: bool,
    completed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for ObservationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if request != RequestId(0) || !self.pending {
                return fail();
            }
            if outcome.disposition == HostCallDisposition::Completed
                && outcome.output.is_some()
                && !io.output_ready(PortId(0))
            {
                return StepOutcome::Await;
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    io.consume_host_completion()
                        .expect("observed measurement completion");
                    io.send(PortId(0), output.value)
                        .expect("ready measurement output");
                    self.pending = false;
                    self.completed = true;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Failed, None, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return fail(),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if !self.pending
                && !self.completed
                && value.byte_len == conduit_core::QUANTITY_ENCODED_LEN as u32
            {
                let input = BoundedValueRef::new(value, conduit_core::QUANTITY_ENCODED_LEN as u32)
                    .expect("exact Quantity");
                io.consume(PortId(0)).expect("present measurement Quantity");
                io.request_host_call(RequestId(0), HostCallId(0), input)
                    .expect("measurement observation Host Call");
                self.pending = true;
                return StepOutcome::Progress;
            }
            return fail();
        }
        if io.input_closed(PortId(0)) && !self.pending {
            io.consume_closed(PortId(0))
                .expect("observed measurement closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.completed = true;
    }
}

fn fail() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: 61,
    })
}
