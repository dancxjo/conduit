//! Exact, bounded Scalar-to-Quantity work through the browser host-operation waist.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{ConfigurationValue, PlannedGear, QuantityUnit, Scalar};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId,
};
use conduit_semantic_catalog::{
    QuantityMapping, QuantityMappingRefusal, QuantizationPolicy, RangePolicy,
};

pub(crate) const HOST_OPERATION: &str = "conduit.host/map-quantity@1";
const IMPLEMENTATION: &str = "browser/kernel-map-quantity@1";
pub(super) static MAP: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    // The fixed-size completion path does not use the allocating generic performer.
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    let contract = conduit_semantic_catalog::quantity_map_contract();
    let target_kind = Some(contract.kind_id.clone());
    let mut offer = conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::QUANTITY_MAP_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: "conduit.browser/map-quantity-kernel@1",
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/map-quantity@1",
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: HOST_OPERATION.into(),
            target_kind,
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
            maximum_output_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
        }],
        Vec::new(),
        Vec::new(),
    );
    offer.limits.max_queue_bytes = conduit_core::QUANTITY_ENCODED_LEN as u32;
    offer
}

pub(crate) fn configuration(placement: &PlannedGear) -> Result<QuantityMapping, String> {
    let number = |key: &str| {
        placement
            .configuration
            .iter()
            .find_map(|field| match &field.value {
                ConfigurationValue::I64(value) if field.key == key => Some(*value),
                _ => None,
            })
            .ok_or_else(|| format!("quantity mapping requires integer '{key}'"))
    };
    let text = |key: &str| {
        placement
            .configuration
            .iter()
            .find_map(|field| match &field.value {
                ConfigurationValue::Text(value) if field.key == key => Some(value.as_str()),
                _ => None,
            })
            .ok_or_else(|| format!("quantity mapping requires text '{key}'"))
    };
    QuantityMapping {
        source_minimum: Scalar::from_raw_microunits(number("source-minimum")?),
        source_maximum: Scalar::from_raw_microunits(number("source-maximum")?),
        target_minimum: number("target-minimum")?,
        target_maximum: number("target-maximum")?,
        target_granularity: number("target-granularity")?,
        target_unit: QuantityUnit::from_form_suffix(text("unit")?)
            .map_err(|error| format!("quantity unit: {error:?}"))?,
        range_policy: match text("range-policy")? {
            "refuse" => RangePolicy::Refuse,
            "clamp" => RangePolicy::Clamp,
            _ => return Err("unknown quantity range policy".into()),
        },
        quantization: match text("quantization")? {
            "exact" => QuantizationPolicy::Exact,
            "nearest" => QuantizationPolicy::Nearest,
            _ => return Err("unknown quantity quantization policy".into()),
        },
    }
    .validate()
    .map_err(|error| format!("quantity configuration: {error:?}"))
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    if placement.configuration.len() != 8 {
        return Err("quantity mapping requires exactly eight configuration fields".into());
    }
    configuration(placement)?;
    Ok(BrowserOperation::installed(QuantityOperation {
        pending: false,
        completed: false,
    }))
}

pub(crate) fn transform(
    mapping: QuantityMapping,
    input: &[u8],
) -> Result<Result<[u8; conduit_core::QUANTITY_ENCODED_LEN], Failure>, String> {
    Ok(Scalar::decode(input)
        .map_err(|_| failure(1))
        .and_then(|scalar| {
            mapping
                .map(scalar)
                .map(|quantity| quantity.encode())
                .map_err(|error| {
                    failure(match error {
                        QuantityMappingRefusal::InvalidRange => 2,
                        QuantityMappingRefusal::OutOfRange => 3,
                        QuantityMappingRefusal::Inexact => 4,
                        QuantityMappingRefusal::Overflow => 5,
                    })
                })
        }))
}

fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

struct QuantityOperation {
    pending: bool,
    completed: bool,
}

impl Operation for QuantityOperation {
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
                && value.byte_len == conduit_core::SCALAR_ENCODED_LEN as u32 =>
            {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, conduit_core::SCALAR_ENCODED_LEN as u32)
                        .expect("exact Scalar"),
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending => {
                self.pending = false;
                self.completed = true;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None)
                        if output.admitted_bytes == conduit_core::QUANTITY_ENCODED_LEN as u32
                            && output.value.byte_len
                                == conduit_core::QUANTITY_ENCODED_LEN as u32 =>
                    {
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Failed, None, Some(reason)) => {
                        OperationAction::Fail(reason)
                    }
                    _ => OperationAction::Fail(failure(1)),
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(failure(1)),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.completed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostOperationOutcome, ValueRef};

    #[test]
    fn browser_quantity_operation_requires_exact_ports_requests_and_output() {
        let mut operation = QuantityOperation {
            pending: false,
            completed: false,
        };
        let input = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 8,
        };
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: input
            }),
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                ..
            }
        ));
        let output = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: 9,
        };
        assert_eq!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(BoundedValueRef::new(output, 9).unwrap()),
                    failure: None,
                },
            }),
            OperationAction::Emit {
                port: PortId(0),
                value: output
            }
        );
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );
    }

    #[test]
    fn browser_quantity_failure_and_cancellation_never_become_output() {
        for detail in 1..=5 {
            let mut operation = QuantityOperation {
                pending: true,
                completed: false,
            };
            let outcome = HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(failure(detail)),
            };
            assert_eq!(
                operation.resume(OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome
                }),
                OperationAction::Fail(failure(detail))
            );
        }
        let mut operation = QuantityOperation {
            pending: true,
            completed: false,
        };
        operation.cancel();
        assert!(matches!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(
                            ValueRef {
                                slot: 0,
                                generation: 1,
                                byte_len: 9
                            },
                            9
                        )
                        .unwrap()
                    ),
                    failure: None,
                },
            }),
            OperationAction::Fail(_)
        ));
    }
}
