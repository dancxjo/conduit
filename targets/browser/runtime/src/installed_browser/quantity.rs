//! Exact, bounded Scalar-to-Quantity work through the browser Host Call waist.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{ConfigurationValue, PlannedGear, QuantityUnit, Scalar};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};
use conduit_semantic_catalog::{
    QuantityMapping, QuantityMappingRefusal, QuantizationPolicy, RangePolicy,
};

pub(crate) const HOST_CALL: &str = "conduit.host/map-quantity@1";
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
        vec![conduit_core::HostCallRequirement {
            contract_id: HOST_CALL.into(),
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
    Ok(BrowserOperation::installed_step(QuantityOperation {
        pending: false,
        next_request: 0,
        cancelled: false,
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
    next_request: u32,
    cancelled: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for QuantityOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request.0.checked_add(1) != Some(self.next_request) {
                return StepOutcome::Fail(failure(1));
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None)
                    if output.admitted_bytes == conduit_core::QUANTITY_ENCODED_LEN as u32
                        && output.value.byte_len == conduit_core::QUANTITY_ENCODED_LEN as u32 =>
                {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed Quantity completion");
                    io.send(PortId(0), output.value)
                        .expect("ready Quantity output");
                    self.pending = false;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Failed, None, Some(reason)) => {
                    return StepOutcome::Fail(reason)
                }
                _ => return StepOutcome::Fail(failure(1)),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if !self.pending
                && !self.cancelled
                && value.byte_len == conduit_core::SCALAR_ENCODED_LEN as u32
            {
                let request = RequestId(self.next_request);
                let Some(next_request) = self.next_request.checked_add(1) else {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 6,
                    });
                };
                let input = BoundedValueRef::new(value, conduit_core::SCALAR_ENCODED_LEN as u32)
                    .expect("exact Scalar");
                io.consume(PortId(0)).expect("present Scalar input");
                io.request_host_call(request, HostCallId(0), input)
                    .expect("Quantity mapping Host Call");
                self.next_request = next_request;
                self.pending = true;
                return StepOutcome::Progress;
            }
            return StepOutcome::Fail(failure(1));
        }
        if io.input_closed(PortId(0)) && !self.pending {
            io.consume_closed(PortId(0))
                .expect("observed Scalar input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.cancelled = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostCallOutcome, ValueRef};

    fn completion(
        operation: &mut QuantityOperation,
        outcome: HostCallOutcome,
    ) -> (StepOutcome, StepIo<1>) {
        let mut io =
            StepIo::test_frame([None], [false], [Some(9)], Some((RequestId(0), outcome)), 4);
        let result = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
        (result, io)
    }

    #[test]
    fn browser_quantity_operation_requires_exact_ports_requests_and_output() {
        let mut operation = QuantityOperation {
            pending: false,
            next_request: 0,
            cancelled: false,
        };
        let input = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 8,
        };
        let mut io = StepIo::test_frame([Some(input)], [false], [Some(9)], None, 4);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );
        let output = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: 9,
        };
        let (result, io) = completion(
            &mut operation,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, 9).unwrap()),
                failure: None,
            },
        );
        assert_eq!(result, StepOutcome::Progress);
        assert_eq!(io.test_output(PortId(0)), Some(output));
        let mut io = StepIo::test_frame([None], [true], [None], None, 4);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
    }

    #[test]
    fn browser_quantity_failure_and_cancellation_never_become_output() {
        for detail in 1..=5 {
            let mut operation = QuantityOperation {
                pending: true,
                next_request: 1,
                cancelled: false,
            };
            let outcome = HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure(detail)),
            };
            assert_eq!(
                completion(&mut operation, outcome).0,
                StepOutcome::Fail(failure(detail))
            );
        }
        let mut operation = QuantityOperation {
            pending: true,
            next_request: 1,
            cancelled: false,
        };
        StepBack::<1>::cancel(&mut operation);
        assert!(matches!(
            completion(
                &mut operation,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
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
                }
            )
            .0,
            StepOutcome::Fail(_)
        ));
    }
}
