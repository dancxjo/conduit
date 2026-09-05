//! Exact selected Quantity normalization through an admitted browser operation.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId,
};
use conduit_semantic_catalog::{NormalizedQuantityRefusal, PreparedNormalizedQuantity};
use std::sync::OnceLock;

pub(crate) const HOST_OPERATION: &str = "conduit.host/normalized-quantity-scalar@1";
const IMPLEMENTATION: &str = "browser/kernel-normalized-quantity-scalar@1";
static CONVERTER: OnceLock<PreparedNormalizedQuantity> = OnceLock::new();

pub(super) static NORMALIZE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    let contract = conduit_semantic_catalog::normalized_quantity_contract();
    let target_kind = Some(contract.kind_id.clone());
    conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::NORMALIZED_QUANTITY_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/normalized-quantity-scalar@1",
        },
        vec![conduit_core::HostOperationRequirement {
            contract_id: HOST_OPERATION.into(),
            target_kind,
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
            maximum_output_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
        }],
        Vec::new(),
        Vec::new(),
    )
}

fn prepare(
    placement: &conduit_core::PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    if !placement.configuration.is_empty() {
        return Err("normalized Quantity conversion accepts no configuration".into());
    }
    CONVERTER.get_or_init(PreparedNormalizedQuantity::new);
    Ok(BrowserOperation::installed(NormalizeOperation {
        pending: false,
        done: false,
    }))
}

pub(crate) fn transform(input: &[u8]) -> Result<[u8; conduit_core::SCALAR_ENCODED_LEN], Failure> {
    let converter = CONVERTER.get().ok_or(failure(14))?;
    converter
        .convert(input)
        .map(|value| value.encode())
        .map_err(|error| {
            failure(match error {
                NormalizedQuantityRefusal::MalformedOrWrongType => 11,
                NormalizedQuantityRefusal::IncompatibleUnit => 12,
                NormalizedQuantityRefusal::OutOfDomain => 13,
            })
        })
}

fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

struct NormalizeOperation {
    pending: bool,
    done: bool,
}

impl Operation for NormalizeOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.done => {
                let Ok(input) = BoundedValueRef::new(
                    value,
                    conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
                ) else {
                    return OperationAction::Fail(failure(11));
                };
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending => {
                self.pending = false;
                self.done = true;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None)
                        if output.admitted_bytes == 8 && output.value.byte_len == 8 =>
                    {
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Failed, None, Some(reason)) => {
                        OperationAction::Fail(reason)
                    }
                    _ => OperationAction::Fail(failure(11)),
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(failure(11)),
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.done = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_quantity_host_preserves_refusals_and_exact_output() {
        CONVERTER.get_or_init(PreparedNormalizedQuantity::new);
        let leaf = |value, unit| {
            conduit_core::StructuredInfoValue::leaf(
                conduit_semantic_catalog::wrapped_quantity_type(),
                conduit_core::Quantity::new(value, unit).encode().to_vec(),
            )
            .unwrap()
            .canonical_bytes()
            .unwrap()
        };
        assert_eq!(
            transform(&leaf(250_000, conduit_core::QuantityUnit::Millionth)),
            Ok(conduit_core::Scalar::from_raw_microunits(250_000).encode())
        );
        for (bytes, detail) in [
            (Vec::new(), 11),
            (leaf(50, conduit_core::QuantityUnit::Percent), 12),
            (leaf(-1, conduit_core::QuantityUnit::Millionth), 13),
        ] {
            let reason = transform(&bytes).unwrap_err();
            assert_eq!(reason, failure(detail));
            let mut operation = NormalizeOperation {
                pending: true,
                done: false,
            };
            assert_eq!(
                operation.resume(OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome: conduit_kernel::HostOperationOutcome {
                        disposition: HostOperationDisposition::Failed,
                        output: None,
                        failure: Some(reason),
                    },
                }),
                OperationAction::Fail(reason)
            );
        }
    }
}
