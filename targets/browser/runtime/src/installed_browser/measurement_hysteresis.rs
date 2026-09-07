//! Browser production realization of explicitly initialized measurement hysteresis.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PlannedGear,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId,
};

pub(crate) const OPERATIONS: [&str; 2] = [
    "conduit.host/browser-measurement-hysteresis-0-profile@1",
    "conduit.host/browser-measurement-hysteresis-1-evaluate@1",
];
const IMPLEMENTATION: &str = "browser/kernel-measurement-hysteresis@2";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

pub(crate) struct PreparedHysteresis {
    hysteresis: Option<conduit_data::MeasurementHysteresis>,
}

impl PreparedHysteresis {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        Ok(Some(Self { hysteresis: None }))
    }

    pub(crate) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<Vec<u8>>, Failure> {
        match contract {
            value if value == OPERATIONS[0] => {
                if self.hysteresis.is_some() {
                    return Err(failure(1));
                }
                let payload =
                    exact_leaf(input, &conduit_data::measurement_hysteresis_profile_type())
                        .ok_or(failure(2))?;
                let profile = conduit_data::decode_measurement_hysteresis_profile(payload)
                    .map_err(|_| failure(3))?;
                self.hysteresis = Some(
                    conduit_data::MeasurementHysteresis::new(profile.policy, profile.initial_state)
                        .map_err(|_| failure(4))?,
                );
                Ok(None)
            }
            value if value == OPERATIONS[1] => {
                let hysteresis = self.hysteresis.as_mut().ok_or(failure(5))?;
                let payload = exact_leaf(input, &conduit_data::measurement_summary_type())
                    .ok_or(failure(6))?;
                let summary =
                    conduit_data::decode_measurement_summary(payload).map_err(|_| failure(7))?;
                let decision = hysteresis.evaluate(&summary).map_err(|error| {
                    use conduit_data::MeasurementThresholdRefusal::*;
                    failure(match error {
                        PolicyUnitMismatch => 8,
                        InvalidPolicyOrder => 9,
                        SummaryUnitMismatch => 10,
                    })
                })?;
                let payload = conduit_data::encode_measurement_threshold_decision(&decision)
                    .map_err(|_| failure(11))?;
                let value = conduit_core::StructuredInfoValue::leaf(
                    conduit_data::measurement_threshold_decision_type(),
                    payload,
                )
                .map_err(|_| failure(11))?;
                let bytes = value.canonical_bytes().map_err(|_| failure(11))?;
                if bytes.len() > MAXIMUM_BROWSER_VALUE_BYTES {
                    return Err(Failure {
                        code: FailureCode::StorageExhausted,
                        detail: 12,
                    });
                }
                Ok(Some(bytes))
            }
            _ => Err(failure(13)),
        }
    }
}

fn offer() -> CapabilityOffer {
    let contract = conduit_data::measurement_hysteresis_kind_definition();
    let kind = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_data::MEASUREMENT_HYSTERESIS_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/measurement-hysteresis@2"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: OPERATIONS
            .iter()
            .enumerate()
            .map(|(index, contract_id)| HostOperationRequirement {
                contract_id: (*contract_id).into(),
                target_kind: Some(kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: if index == 0 {
                    0
                } else {
                    MAXIMUM_BROWSER_VALUE_BYTES as u32
                },
            })
            .collect(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    PreparedHysteresis::for_placement(placement)?
        .ok_or_else(|| "measurement hysteresis selected another implementation".to_string())?;
    Ok(BrowserOperation::installed(HysteresisOperation::new()))
}

struct HysteresisOperation {
    profile_ready: bool,
    pending: Option<(RequestId, HostOperationId)>,
    emitted: bool,
}

impl HysteresisOperation {
    const fn new() -> Self {
        Self {
            profile_ready: false,
            pending: None,
            emitted: false,
        }
    }
}

impl Operation for HysteresisOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.profile_ready && self.pending.is_none() => {
                let Ok(input) = BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32)
                else {
                    return OperationAction::Fail(failure(20));
                };
                self.pending = Some((RequestId(0), HostOperationId(0)));
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::Value {
                port: PortId(1),
                value,
            } if self.profile_ready && self.pending.is_none() && !self.emitted => {
                let Ok(input) = BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32)
                else {
                    return OperationAction::Fail(failure(21));
                };
                self.pending = Some((RequestId(1), HostOperationId(1)));
                OperationAction::RequestHostOperation {
                    request: RequestId(1),
                    operation: HostOperationId(1),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending.map(|pending| pending.0) == Some(request) =>
            {
                let Some((_, operation)) = self.pending.take() else {
                    return OperationAction::Fail(failure(22));
                };
                match (
                    operation,
                    outcome.disposition,
                    outcome.output,
                    outcome.failure,
                ) {
                    (HostOperationId(0), HostOperationDisposition::Completed, None, None) => {
                        self.profile_ready = true;
                        OperationAction::Await
                    }
                    (
                        HostOperationId(1),
                        HostOperationDisposition::Completed,
                        Some(output),
                        None,
                    ) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (_, HostOperationDisposition::Failed, None, Some(reason)) => {
                        OperationAction::Fail(reason)
                    }
                    _ => OperationAction::Fail(failure(22)),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.profile_ready => {
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(1) } if self.emitted => OperationAction::Complete,
            _ => OperationAction::Fail(failure(23)),
        }
    }
    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
    fn cancel(&mut self) {
        self.pending = None;
    }
}

fn exact_leaf<'a>(
    canonical: &'a [u8],
    value_type: &conduit_core::StructuredInfoType,
) -> Option<&'a [u8]> {
    let type_bytes = value_type.canonical_bytes().ok()?;
    let node = canonical.strip_prefix(type_bytes.as_slice())?;
    if node.first() != Some(&0) || node.len() < 5 {
        return None;
    }
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().ok()?)).ok()?;
    (node.len() == 5 + length).then_some(&node[5..])
}

fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        ConfigurationEntry, OfferGeneration, Quantity, QuantityUnit, StructuredInfoValue,
        TemporalInstant, TemporalScale,
    };

    fn placement() -> PlannedGear {
        let offered = offer();
        PlannedGear {
            placement_id: "hysteresis-placement".into(),
            gear_id: "hysteresis".into(),
            kind_id: offered.kind_id,
            kind_contract_revision: offered.kind_contract_revision,
            execution_profile_id: offered.implementation.execution_profile_id,
            configuration: Vec::<ConfigurationEntry>::new(),
            host_id: "browser/hysteresis".into(),
            boot_id: "browser-boot/hysteresis".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offered.capability_id,
            implementation_id: offered.implementation.implementation_id,
            artifact_id: offered.implementation.artifact_id,
            realization_characteristics: Vec::new(),
            limits: offered.limits,
            inputs: offered.inputs,
            outputs: offered.outputs,
            host_operations: offered.host_operations,
            resources: Vec::new(),
            authority: Vec::new(),
            pool_references: Vec::new(),
        }
    }

    fn leaf(value_type: conduit_core::StructuredInfoType, payload: Vec<u8>) -> Vec<u8> {
        StructuredInfoValue::leaf(value_type, payload)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    }

    fn profile(initial_state: conduit_data::MeasurementThresholdState) -> Vec<u8> {
        leaf(
            conduit_data::measurement_hysteresis_profile_type(),
            conduit_data::encode_measurement_hysteresis_profile(
                conduit_data::MeasurementHysteresisProfile {
                    policy: conduit_data::MeasurementThresholdPolicy {
                        lower: Quantity::new(40, QuantityUnit::Millivolt),
                        upper: Quantity::new(60, QuantityUnit::Millivolt),
                    },
                    initial_state,
                },
            )
            .unwrap(),
        )
    }

    fn summary(value: i64, unit: QuantityUnit) -> Vec<u8> {
        let instant = TemporalInstant {
            ticks: 1,
            scale: TemporalScale::Milliseconds,
            clock_basis: "hysteresis-clock".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        };
        leaf(
            conduit_data::measurement_summary_type(),
            conduit_data::encode_measurement_summary(&conduit_data::MeasurementSummary {
                unit,
                sample_count: 1,
                first_observed_at: instant.clone(),
                last_observed_at: instant,
                minimum: Quantity::new(value, unit),
                maximum: Quantity::new(value, unit),
                range: Quantity::new(0, unit),
                mean: Quantity::new(value, unit),
            })
            .unwrap(),
        )
    }

    #[test]
    fn browser_hysteresis_uses_the_explicit_initial_state_and_thresholds() {
        let mut prepared = PreparedHysteresis::for_placement(&placement())
            .unwrap()
            .unwrap();
        assert_eq!(
            prepared.execute(
                OPERATIONS[0],
                &profile(conduit_data::MeasurementThresholdState::Above)
            ),
            Ok(None)
        );
        let output = prepared
            .execute(OPERATIONS[1], &summary(40, QuantityUnit::Millivolt))
            .unwrap()
            .unwrap();
        let payload = exact_leaf(
            &output,
            &conduit_data::measurement_threshold_decision_type(),
        )
        .unwrap();
        let decision = conduit_data::decode_measurement_threshold_decision(payload).unwrap();
        assert_eq!(
            decision.transition,
            Some(conduit_data::MeasurementThresholdTransition::FellBelow)
        );
    }

    #[test]
    fn browser_hysteresis_keeps_profile_type_payload_and_summary_unit_failures_distinct() {
        let mut prepared = PreparedHysteresis::for_placement(&placement())
            .unwrap()
            .unwrap();
        assert_eq!(prepared.execute(OPERATIONS[1], b"summary"), Err(failure(5)));
        assert_eq!(prepared.execute(OPERATIONS[0], b"profile"), Err(failure(2)));
        prepared
            .execute(
                OPERATIONS[0],
                &profile(conduit_data::MeasurementThresholdState::Below),
            )
            .unwrap();
        assert_eq!(
            prepared.execute(OPERATIONS[1], &summary(50, QuantityUnit::Millimeter)),
            Err(failure(10))
        );
    }
}
