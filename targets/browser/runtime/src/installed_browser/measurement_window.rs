//! Browser production realization of an explicitly profiled finite measurement window.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PlannedGear,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedValueStore, Operation, OperationAction, OperationInput, PortId,
    RequestId, ValueRef, ValueStorage,
};

pub(crate) const OPERATIONS: [&str; 2] = [
    "conduit.host/browser-measurement-window-profile@1",
    "conduit.host/browser-measurement-window-push@1",
];
const IMPLEMENTATION: &str = "browser/kernel-measurement-window@2";
const FINALIZE_INPUT: &[u8] = b"conduit.measurement-window/final@1";
const FINALIZE_REQUEST: RequestId =
    RequestId(conduit_data::MAXIMUM_MEASUREMENT_WINDOW_SAMPLES as u32 + 1);

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

pub(crate) struct PreparedWindow {
    window: Option<conduit_data::BoundedMeasurementWindow>,
}

impl PreparedWindow {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        Ok(Some(Self { window: None }))
    }

    pub(crate) fn execute(
        &mut self,
        contract: &str,
        canonical: &[u8],
    ) -> Result<Option<Vec<u8>>, Failure> {
        match contract {
            value if value == OPERATIONS[0] => {
                if self.window.is_some() {
                    return Err(failure(1));
                }
                let payload =
                    exact_leaf(canonical, &conduit_data::measurement_window_profile_type())
                        .ok_or(failure(2))?;
                let profile = conduit_data::decode_measurement_window_profile(payload)
                    .map_err(|_| failure(3))?;
                self.window = Some(
                    conduit_data::BoundedMeasurementWindow::new(profile).map_err(|_| failure(4))?,
                );
                Ok(None)
            }
            value if value == OPERATIONS[1] => {
                let window = self.window.as_mut().ok_or(failure(5))?;
                if canonical == FINALIZE_INPUT {
                    let payload =
                        conduit_data::encode_measurement_window(window).map_err(|_| failure(22))?;
                    let value = conduit_core::StructuredInfoValue::leaf(
                        conduit_data::measurement_window_type(),
                        payload,
                    )
                    .map_err(|_| failure(22))?;
                    let bytes = value.canonical_bytes().map_err(|_| failure(22))?;
                    if bytes.len() > MAXIMUM_BROWSER_VALUE_BYTES {
                        return Err(Failure {
                            code: FailureCode::StorageExhausted,
                            detail: 23,
                        });
                    }
                    return Ok(Some(bytes));
                }
                let payload = exact_leaf(canonical, &conduit_data::measurement_sample_type())
                    .ok_or(failure(6))?;
                let sample =
                    conduit_data::decode_measurement_sample(payload).map_err(|_| failure(7))?;
                window.push(sample).map_err(|error| {
                    use conduit_data::MeasurementWindowRefusal::*;
                    failure(match error {
                        CapacityOutOfBounds => 10,
                        InvalidClockProfile => 11,
                        InvalidRange => 12,
                        InvalidTimestamp => 13,
                        UnitMismatch => 14,
                        UncertaintyUnitMismatch => 15,
                        NegativeUncertainty => 16,
                        ClockMismatch => 17,
                        TimestampRegression => 18,
                        OutOfRange => 19,
                        Full => 20,
                        DiscardCountOverflow => 21,
                    })
                })?;
                Ok(None)
            }
            _ => Err(failure(24)),
        }
    }
}

fn offer() -> CapabilityOffer {
    let contract = conduit_data::measurement_window_kind_definition();
    let kind = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_data::MEASUREMENT_WINDOW_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/measurement-window@2"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![
            HostOperationRequirement {
                contract_id: OPERATIONS[0].into(),
                target_kind: Some(kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: 0,
            },
            HostOperationRequirement {
                contract_id: OPERATIONS[1].into(),
                target_kind: Some(kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
            },
        ],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_data::MAXIMUM_MEASUREMENT_WINDOW_SAMPLES as u16 + 1,
            max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    PreparedWindow::for_placement(placement)?
        .ok_or_else(|| "measurement window selected another implementation".to_string())?;
    let finalize = values
        .store(FINALIZE_INPUT)
        .map_err(|error| format!("prepare measurement window finalizer: {error:?}"))?;
    Ok(BrowserOperation::installed(WindowOperation::new(finalize)))
}

struct WindowOperation {
    profile_ready: bool,
    sample_closed: bool,
    pending: Option<(RequestId, HostOperationId)>,
    next_sample: u32,
    finalize: Option<ValueRef>,
    released: Option<ValueRef>,
    emitted: bool,
}

impl WindowOperation {
    const fn new(finalize: ValueRef) -> Self {
        Self {
            profile_ready: false,
            sample_closed: false,
            pending: None,
            next_sample: 0,
            finalize: Some(finalize),
            released: None,
            emitted: false,
        }
    }

    fn complete_host(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
    ) -> OperationAction {
        let Some((expected, operation)) = self.pending else {
            return OperationAction::Fail(failure(30));
        };
        if request != expected {
            return OperationAction::Fail(failure(30));
        }
        self.pending = None;
        if let (HostOperationDisposition::Failed, None, Some(failure)) =
            (outcome.disposition, outcome.output, outcome.failure)
        {
            return OperationAction::Fail(failure);
        }
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
            (HostOperationId(1), HostOperationDisposition::Completed, None, None)
                if request != FINALIZE_REQUEST =>
            {
                self.next_sample = self.next_sample.saturating_add(1);
                OperationAction::Await
            }
            (HostOperationId(1), HostOperationDisposition::Completed, Some(output), None)
                if request == FINALIZE_REQUEST =>
            {
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            _ => OperationAction::Fail(failure(30)),
        }
    }
}

impl Operation for WindowOperation {
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
                    return OperationAction::Fail(failure(31));
                };
                let request = RequestId(0);
                let operation = HostOperationId(0);
                self.pending = Some((request, operation));
                OperationAction::RequestHostOperation {
                    request,
                    operation,
                    input,
                }
            }
            OperationInput::Value {
                port: PortId(1),
                value,
            } if self.profile_ready
                && self.pending.is_none()
                && self.next_sample < conduit_data::MAXIMUM_MEASUREMENT_WINDOW_SAMPLES as u32 =>
            {
                let Ok(input) = BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32)
                else {
                    return OperationAction::Fail(failure(32));
                };
                let request = RequestId(self.next_sample + 1);
                let operation = HostOperationId(1);
                self.pending = Some((request, operation));
                OperationAction::RequestHostOperation {
                    request,
                    operation,
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome } => {
                self.complete_host(request, outcome)
            }
            OperationInput::Closed { port: PortId(0) } if self.profile_ready => {
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(1) } if self.pending.is_none() => {
                self.sample_closed = true;
                let Some(value) = self.finalize.take() else {
                    return OperationAction::Fail(failure(33));
                };
                let Ok(input) = BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32)
                else {
                    return OperationAction::Fail(failure(33));
                };
                self.pending = Some((FINALIZE_REQUEST, HostOperationId(1)));
                OperationAction::RequestHostOperation {
                    request: FINALIZE_REQUEST,
                    operation: HostOperationId(1),
                    input,
                }
            }
            _ => OperationAction::Fail(failure(34)),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted && self.sample_closed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.take()
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.released = self.finalize.take();
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
    use conduit_data::{
        FullWindowPolicy, MeasurementRange, MeasurementSample, MeasurementWindowProfile,
    };

    fn placement() -> PlannedGear {
        let offered = offer();
        PlannedGear {
            placement_id: "window-placement".into(),
            gear_id: "window".into(),
            kind_id: offered.kind_id,
            kind_contract_revision: offered.kind_contract_revision,
            execution_profile_id: offered.implementation.execution_profile_id,
            configuration: Vec::<ConfigurationEntry>::new(),
            host_id: "browser/window".into(),
            boot_id: "browser-boot/window".into(),
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

    fn profile() -> MeasurementWindowProfile {
        MeasurementWindowProfile {
            capacity: 2,
            unit: QuantityUnit::Millivolt,
            range: MeasurementRange {
                minimum: Quantity::new(0, QuantityUnit::Millivolt),
                maximum: Quantity::new(100, QuantityUnit::Millivolt),
            },
            clock_basis: "browser-window-clock".into(),
            full_policy: FullWindowPolicy::DropOldest,
        }
    }

    fn leaf(value_type: conduit_core::StructuredInfoType, payload: Vec<u8>) -> Vec<u8> {
        StructuredInfoValue::leaf(value_type, payload)
            .unwrap()
            .canonical_bytes()
            .unwrap()
    }

    fn sample(value: i64, ticks: u64) -> MeasurementSample {
        MeasurementSample {
            value: Quantity::new(value, QuantityUnit::Millivolt),
            observed_at: TemporalInstant {
                ticks,
                scale: TemporalScale::Milliseconds,
                clock_basis: "browser-window-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            uncertainty: None,
        }
    }

    #[test]
    fn prepared_window_requires_profile_then_preserves_bounded_drop_evidence() {
        let mut prepared = PreparedWindow::for_placement(&placement())
            .unwrap()
            .unwrap();
        let profile = leaf(
            conduit_data::measurement_window_profile_type(),
            conduit_data::encode_measurement_window_profile(&profile()).unwrap(),
        );
        assert_eq!(prepared.execute(OPERATIONS[0], &profile), Ok(None));
        for (value, ticks) in [(0, 1), (50, 2), (100, 3)] {
            let sample = leaf(
                conduit_data::measurement_sample_type(),
                conduit_data::encode_measurement_sample(&sample(value, ticks)).unwrap(),
            );
            assert_eq!(prepared.execute(OPERATIONS[1], &sample), Ok(None));
        }
        let output = prepared
            .execute(OPERATIONS[1], FINALIZE_INPUT)
            .unwrap()
            .unwrap();
        let payload = exact_leaf(&output, &conduit_data::measurement_window_type()).unwrap();
        let window = conduit_data::decode_measurement_window(payload).unwrap();
        assert_eq!(window.discarded_samples(), 1);
        assert_eq!(
            window
                .samples()
                .iter()
                .map(|sample| sample.value.value())
                .collect::<Vec<_>>(),
            [50, 100]
        );
    }

    #[test]
    fn prepared_window_keeps_profile_type_unit_range_clock_and_time_failures_distinct() {
        let mut prepared = PreparedWindow::for_placement(&placement())
            .unwrap()
            .unwrap();
        assert_eq!(prepared.execute(OPERATIONS[1], b"sample"), Err(failure(5)));
        assert_eq!(prepared.execute(OPERATIONS[0], b"profile"), Err(failure(2)));
        let profile = leaf(
            conduit_data::measurement_window_profile_type(),
            conduit_data::encode_measurement_window_profile(&profile()).unwrap(),
        );
        prepared.execute(OPERATIONS[0], &profile).unwrap();
        let wrong_unit = MeasurementSample {
            value: Quantity::new(1, QuantityUnit::Volt),
            ..sample(1, 1)
        };
        let wrong_unit = leaf(
            conduit_data::measurement_sample_type(),
            conduit_data::encode_measurement_sample(&wrong_unit).unwrap(),
        );
        assert_eq!(
            prepared.execute(OPERATIONS[1], &wrong_unit),
            Err(failure(14))
        );
        let out_of_range = leaf(
            conduit_data::measurement_sample_type(),
            conduit_data::encode_measurement_sample(&sample(101, 1)).unwrap(),
        );
        assert_eq!(
            prepared.execute(OPERATIONS[1], &out_of_range),
            Err(failure(19))
        );
        let wrong_clock = MeasurementSample {
            observed_at: TemporalInstant {
                clock_basis: "other-clock".into(),
                ..sample(1, 1).observed_at
            },
            ..sample(1, 1)
        };
        let wrong_clock = leaf(
            conduit_data::measurement_sample_type(),
            conduit_data::encode_measurement_sample(&wrong_clock).unwrap(),
        );
        assert_eq!(
            prepared.execute(OPERATIONS[1], &wrong_clock),
            Err(failure(17))
        );
    }
}
