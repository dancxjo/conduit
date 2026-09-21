//! Exact Quantity-to-MeasurementSample work in the ordinary browser kernel.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ConfigurationValue, PlannedGear, StructuredInfoValue, TemporalInstant, TemporalScale,
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
    let definition = conduit_data::measurement_observation_definition();
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::StandardKindContract {
            kind_id: definition.kind_id,
            plain_name: "Observe an exact measurement".into(),
            summary: "Bind one exact quantity to an explicit semantic observation instant.".into(),
            inputs: definition.inputs,
            outputs: definition.outputs,
            configuration: vec![
                conduit_semantic_catalog::KindConfigurationField {
                    key: "clock-basis".into(),
                    default_value: ConfigurationValue::Text("control-occurrence".into()),
                    rule: conduit_semantic_catalog::KindConfigurationRule::TextBytes {
                        maximum: conduit_data::MAXIMUM_MEASUREMENT_CLOCK_BASIS_BYTES,
                    },
                },
            ],
            limits: conduit_core::CapabilityLimits {
                max_active_instances: 8,
                max_queue_items: 1,
                max_queue_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            },
            terminal_behavior: conduit_semantic_catalog::KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "observe: data/measurement-observation".into(),
        },
        conduit_data::MEASUREMENT_OBSERVATION_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/measurement-observation@1",
        },
        vec![conduit_core::HostCallRequirement {
            contract_id: HOST_CALL.into(),
            target_kind: Some(conduit_core::kind_id(conduit_data::MEASUREMENT_OBSERVATION_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
            maximum_output_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    configuration(placement)?;
    Ok(BrowserOperation::installed_step(ObservationOperation {
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

struct ObservationOperation {
    pending: bool,
    completed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for ObservationOperation {
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
