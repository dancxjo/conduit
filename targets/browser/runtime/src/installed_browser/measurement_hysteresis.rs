//! Browser production realization of explicitly initialized measurement hysteresis.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallRequirement, ImplementationId, PlannedGear,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostedValueStore,
    PortId, RequestId,
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
    let contract = conduit_data::measurement_hysteresis_semantic_contract();
    let kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/measurement-hysteresis@2"),
            host_calls: OPERATIONS
                .iter()
                .enumerate()
                .map(|(index, contract_id)| HostCallRequirement {
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
        },
    )
    .build()
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    PreparedHysteresis::for_placement(placement)?
        .ok_or_else(|| "measurement hysteresis selected another implementation".to_string())?;
    Ok(BrowserOperation::installed_step(HysteresisOperation::new()))
}

struct HysteresisOperation {
    profile_ready: bool,
    profile_closed: bool,
    pending: Option<(RequestId, HostCallId)>,
    emitted: bool,
}

impl HysteresisOperation {
    const fn new() -> Self {
        Self {
            profile_ready: false,
            profile_closed: false,
            pending: None,
            emitted: false,
        }
    }
}

impl<const PORTS: usize> StepOperation<PORTS> for HysteresisOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            let Some((pending_request, operation)) = self.pending else {
                return StepOutcome::Fail(failure(22));
            };
            if request != pending_request {
                return StepOutcome::Fail(failure(22));
            }
            match (
                operation,
                outcome.disposition,
                outcome.output,
                outcome.failure,
            ) {
                (HostCallId(0), HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed hysteresis profile completion");
                    self.pending = None;
                    self.profile_ready = true;
                    return StepOutcome::Progress;
                }
                (HostCallId(1), HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed hysteresis evaluation completion");
                    io.send(PortId(0), output.value)
                        .expect("ready hysteresis decision output");
                    self.pending = None;
                    self.emitted = true;
                    return StepOutcome::Complete;
                }
                (_, HostCallDisposition::Failed, None, Some(reason)) => {
                    return StepOutcome::Fail(reason)
                }
                _ => return StepOutcome::Fail(failure(22)),
            }
        }

        if let Some(value) = io.input(PortId(0)) {
            if self.profile_ready || self.pending.is_some() {
                return StepOutcome::Fail(failure(23));
            }
            let input = match BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32) {
                Ok(input) => input,
                Err(_) => return StepOutcome::Fail(failure(20)),
            };
            io.consume(PortId(0)).expect("present hysteresis profile");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("hysteresis profile Host Call");
            self.pending = Some((RequestId(0), HostCallId(0)));
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(1)) {
            if !self.profile_ready || self.pending.is_some() || self.emitted {
                return StepOutcome::Fail(failure(23));
            }
            let input = match BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32) {
                Ok(input) => input,
                Err(_) => return StepOutcome::Fail(failure(21)),
            };
            io.consume(PortId(1)).expect("present measurement summary");
            io.request_host_call(RequestId(1), HostCallId(1), input)
                .expect("hysteresis evaluation Host Call");
            self.pending = Some((RequestId(1), HostCallId(1)));
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.profile_closed {
            if !self.profile_ready {
                return StepOutcome::Fail(failure(23));
            }
            io.consume_closed(PortId(0))
                .expect("observed hysteresis profile closure");
            self.profile_closed = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(1)) {
            if !self.emitted {
                return StepOutcome::Fail(failure(23));
            }
            io.consume_closed(PortId(1))
                .expect("observed measurement summary closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
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
    use conduit_kernel::{HostCallOutcome, ValueRef};

    fn value(slot: u16, byte_len: u32) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len,
        }
    }

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
            base: None,
            realization_characteristics: Vec::new(),
            limits: offered.limits,
            inputs: offered.inputs,
            outputs: offered.outputs,
            host_calls: offered.host_calls,
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
        let offer = offer();
        let semantic = conduit_data::measurement_hysteresis_semantic_contract();
        assert_eq!(offer.startup_parameters, semantic.startup_parameters);
        assert_eq!(offer.kind_id, semantic.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            semantic.kind_contract_revision
        );
        assert_eq!(offer.inputs, semantic.inputs);
        assert_eq!(offer.outputs, semantic.outputs);
        assert_eq!(offer.limits, semantic.limits);
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

    #[test]
    fn browser_hysteresis_step_preserves_pending_evaluation_under_output_pressure() {
        let mut operation = HysteresisOperation::new();
        let profile = value(1, 40);
        let mut profile_io = StepIo::test_frame(
            [Some(profile), None],
            [false; 2],
            [Some(MAXIMUM_BROWSER_VALUE_BYTES as u32), None],
            None,
            4,
        );
        assert_eq!(
            operation.step(
                &mut profile_io,
                &StepInputBytes::test_frame([None, None], None),
            ),
            StepOutcome::Progress
        );
        assert_eq!(
            profile_io.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );

        let profile_completion = HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: None,
            failure: None,
        };
        let mut completion_io = StepIo::test_frame(
            [None; 2],
            [false; 2],
            [Some(MAXIMUM_BROWSER_VALUE_BYTES as u32), None],
            Some((RequestId(0), profile_completion)),
            4,
        );
        assert_eq!(
            operation.step(
                &mut completion_io,
                &StepInputBytes::test_frame([None, None], None),
            ),
            StepOutcome::Progress
        );

        let summary = value(2, 80);
        let mut summary_io = StepIo::test_frame(
            [None, Some(summary)],
            [false; 2],
            [Some(MAXIMUM_BROWSER_VALUE_BYTES as u32), None],
            None,
            4,
        );
        assert_eq!(
            operation.step(
                &mut summary_io,
                &StepInputBytes::test_frame([None, None], None),
            ),
            StepOutcome::Progress
        );
        assert_eq!(
            summary_io.test_host_request().map(|request| request.0),
            Some(RequestId(1))
        );

        let output = value(3, 16);
        let evaluation = HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(BoundedValueRef::new(output, 16).unwrap()),
            failure: None,
        };
        let mut blocked = StepIo::test_frame(
            [None; 2],
            [false; 2],
            [None; 2],
            Some((RequestId(1), evaluation)),
            4,
        );
        assert_eq!(
            operation.step(
                &mut blocked,
                &StepInputBytes::test_frame([None, None], None),
            ),
            StepOutcome::Await
        );
        assert_eq!(operation.pending, Some((RequestId(1), HostCallId(1))));
        assert!(!operation.emitted);

        let mut ready = StepIo::test_frame(
            [None; 2],
            [false; 2],
            [Some(16), None],
            Some((RequestId(1), evaluation)),
            4,
        );
        assert_eq!(
            operation.step(&mut ready, &StepInputBytes::test_frame([None, None], None),),
            StepOutcome::Complete
        );
        assert_eq!(ready.test_output(PortId(0)), Some(output));
    }
}
