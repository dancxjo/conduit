//! Exact Quantity-to-MeasurementSample work in the ordinary browser kernel.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ConfigurationValue, PlannedGear, StructuredInfoValue, TemporalInstant, TemporalScale,
};
use conduit_data::MeasurementSample;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId,
};

pub(crate) const HOST_OPERATION: &str = "conduit.host/measurement-observation@1";
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
                conduit_semantic_catalog::StandardConfigurationField {
                    key: "clock-basis".into(),
                    default_value: ConfigurationValue::Text("control-occurrence".into()),
                    rule: conduit_semantic_catalog::StandardConfigurationRule::TextBytes {
                        maximum: conduit_data::MAXIMUM_MEASUREMENT_CLOCK_BASIS_BYTES,
                    },
                },
            ],
            limits: conduit_core::CapabilityLimits {
                max_active_instances: 8,
                max_queue_items: 1,
                max_queue_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            },
            terminal_behavior: conduit_semantic_catalog::TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
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
        vec![conduit_core::HostOperationRequirement {
            contract_id: HOST_OPERATION.into(),
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
    Ok(BrowserOperation::installed(ObservationOperation {
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

impl Operation for ObservationOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending
                && !self.completed
                && value.byte_len == conduit_core::QUANTITY_ENCODED_LEN as u32 =>
            {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, conduit_core::QUANTITY_ENCODED_LEN as u32)
                        .expect("exact Quantity"),
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending => {
                self.pending = false;
                self.completed = true;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Failed, None, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(),
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => fail(),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.completed = true;
    }
}

fn fail() -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: 61,
    })
}
