//! Browser production realization of the reusable finite semantic stroke capture.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PlannedGear, StructuredInfoValue,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedValueStore, Operation, OperationAction, OperationInput, PortId,
    RequestId, ValueRef, ValueStorage,
};

pub(crate) const OPERATIONS: [&str; 2] = [
    "conduit.host/browser-stroke-capture-append@1",
    "conduit.host/browser-stroke-capture-finish@1",
];
const IMPLEMENTATION: &str = "browser/bounded-stroke-capture@1";
const MAXIMUM_POINTS: u32 = 4;
const FINISH_INPUT: &[u8] = b"conduit.stroke-capture/finish@1";
const FINISH_REQUEST: RequestId = RequestId(MAXIMUM_POINTS + 1);

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

pub(crate) struct PreparedStrokeCapture {
    capture: Option<conduit_presentation::BoundedStrokeCapture>,
}

impl PreparedStrokeCapture {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        Ok(Some(Self {
            capture: Some(
                conduit_presentation::BoundedStrokeCapture::new(4, 4)
                    .map_err(|error| format!("prepare bounded stroke capture: {error:?}"))?,
            ),
        }))
    }

    pub(crate) fn execute(
        &mut self,
        contract: &str,
        canonical: &[u8],
    ) -> Result<Option<Vec<u8>>, Failure> {
        match contract {
            value if value == OPERATIONS[0] => {
                let capture = self.capture.as_mut().ok_or_else(|| failure(1))?;
                let point =
                    StructuredInfoValue::from_canonical_bytes(canonical).map_err(|_| failure(2))?;
                capture.push(point).map_err(capture_failure)?;
                Ok(None)
            }
            value if value == OPERATIONS[1] && canonical == FINISH_INPUT => {
                let capture = self.capture.take().ok_or_else(|| failure(3))?;
                let output = capture
                    .finish()
                    .map_err(capture_failure)?
                    .canonical_bytes()
                    .map_err(|_| failure(4))?;
                if output.len() > MAXIMUM_BROWSER_VALUE_BYTES {
                    return Err(Failure {
                        code: FailureCode::StorageExhausted,
                        detail: 5,
                    });
                }
                Ok(Some(output))
            }
            _ => Err(failure(6)),
        }
    }
}

fn offer() -> CapabilityOffer {
    let definition = conduit_presentation::capture_bounded_stroke_kind_definition();
    let kind = definition.kind_id.clone();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(conduit_presentation::GEOMETRY_REVISION),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-presentation/bounded-stroke-capture@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
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
                maximum_input_bytes: FINISH_INPUT.len() as u32,
                maximum_output_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
            },
        ],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: MAXIMUM_POINTS as u16,
            max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    PreparedStrokeCapture::for_placement(placement)?
        .ok_or_else(|| "bounded stroke capture selected another implementation".to_string())?;
    let finish = values
        .store(FINISH_INPUT)
        .map_err(|error| format!("prepare bounded stroke finish: {error:?}"))?;
    Ok(BrowserOperation::installed(StrokeCaptureOperation::new(
        finish,
    )))
}

struct StrokeCaptureOperation {
    pending: Option<RequestId>,
    next_point: u32,
    input_closed: bool,
    finish: Option<ValueRef>,
    emitted: bool,
}

impl StrokeCaptureOperation {
    const fn new(finish: ValueRef) -> Self {
        Self {
            pending: None,
            next_point: 0,
            input_closed: false,
            finish: Some(finish),
            emitted: false,
        }
    }

    fn complete_host(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
    ) -> OperationAction {
        if self.pending != Some(request) {
            return OperationAction::Fail(failure(20));
        }
        self.pending = None;
        if let (HostOperationDisposition::Failed, None, Some(failure)) =
            (outcome.disposition, outcome.output, outcome.failure)
        {
            return OperationAction::Fail(failure);
        }
        match (
            request,
            outcome.disposition,
            outcome.output,
            outcome.failure,
        ) {
            (FINISH_REQUEST, HostOperationDisposition::Completed, Some(output), None) => {
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (_, HostOperationDisposition::Completed, None, None) if request != FINISH_REQUEST => {
                self.next_point = self.next_point.saturating_add(1);
                OperationAction::Await
            }
            _ => OperationAction::Fail(failure(21)),
        }
    }
}

impl Operation for StrokeCaptureOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && self.next_point < MAXIMUM_POINTS => {
                let Ok(input) = BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32)
                else {
                    return OperationAction::Fail(failure(22));
                };
                let request = RequestId(self.next_point);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome } => {
                self.complete_host(request, outcome)
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                self.input_closed = true;
                let Some(value) = self.finish.take() else {
                    return OperationAction::Fail(failure(23));
                };
                let Ok(input) = BoundedValueRef::new(value, FINISH_INPUT.len() as u32) else {
                    return OperationAction::Fail(failure(23));
                };
                self.pending = Some(FINISH_REQUEST);
                OperationAction::RequestHostOperation {
                    request: FINISH_REQUEST,
                    operation: HostOperationId(1),
                    input,
                }
            }
            _ => OperationAction::Fail(failure(24)),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted && self.input_closed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.finish = None;
    }
}

fn capture_failure(error: conduit_presentation::StrokeCaptureRefusal) -> Failure {
    use conduit_presentation::StrokeCaptureRefusal::*;
    match error {
        InvalidCapacity => failure(30),
        MalformedPoint => failure(31),
        FrameMismatch { .. } => failure(32),
        Pressure => Failure {
            code: FailureCode::StateCapacityExhausted,
            detail: 33,
        },
        PartialStroke => Failure {
            code: FailureCode::InvalidLifecycle,
            detail: 34,
        },
        Geometry(_) => failure(35),
    }
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
    use conduit_core::{OfferGeneration, Quantity, QuantityUnit};

    fn placement() -> PlannedGear {
        let offered = offer();
        PlannedGear {
            placement_id: "capture-placement".into(),
            gear_id: "capture".into(),
            kind_id: offered.kind_id,
            kind_contract_revision: offered.kind_contract_revision,
            execution_profile_id: offered.implementation.execution_profile_id,
            configuration: Vec::new(),
            host_id: "browser/capture".into(),
            boot_id: "browser-boot/capture".into(),
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

    fn point(frame: &str, x: i64) -> Vec<u8> {
        conduit_presentation::point2_value(
            frame,
            Quantity::new(x, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Millimeter),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    }

    #[test]
    fn prepared_capture_preserves_exact_order_and_frame() {
        let mut prepared = PreparedStrokeCapture::for_placement(&placement())
            .unwrap()
            .unwrap();
        for x in [2, 3, 5, 8] {
            assert_eq!(
                prepared.execute(OPERATIONS[0], &point("trail", x)),
                Ok(None)
            );
        }
        let output = prepared
            .execute(OPERATIONS[1], FINISH_INPUT)
            .unwrap()
            .unwrap();
        let path = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        assert_eq!(
            path.value_type(),
            &conduit_presentation::path2_type(4).unwrap()
        );
    }

    #[test]
    fn prepared_capture_keeps_partial_pressure_type_and_frame_failures_distinct() {
        let mut partial = PreparedStrokeCapture::for_placement(&placement())
            .unwrap()
            .unwrap();
        partial.execute(OPERATIONS[0], &point("trail", 1)).unwrap();
        assert_eq!(
            partial.execute(OPERATIONS[1], FINISH_INPUT),
            Err(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 34,
            })
        );

        let mut wrong = PreparedStrokeCapture::for_placement(&placement())
            .unwrap()
            .unwrap();
        assert_eq!(wrong.execute(OPERATIONS[0], b"point"), Err(failure(2)));
        wrong.execute(OPERATIONS[0], &point("trail-a", 1)).unwrap();
        assert_eq!(
            wrong.execute(OPERATIONS[0], &point("trail-b", 2)),
            Err(failure(32))
        );

        let mut full = PreparedStrokeCapture::for_placement(&placement())
            .unwrap()
            .unwrap();
        for x in [1, 2, 3, 4] {
            full.execute(OPERATIONS[0], &point("trail", x)).unwrap();
        }
        assert_eq!(
            full.execute(OPERATIONS[0], &point("trail", 5)),
            Err(Failure {
                code: FailureCode::StateCapacityExhausted,
                detail: 33,
            })
        );
    }
}
