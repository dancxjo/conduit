//! Exact selected Quantity normalization through an admitted browser operation.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallRequirement, ImplementationId,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, Operation,
    OperationAction, OperationInput, PortId, RequestId,
};
use conduit_semantic_catalog::{NormalizedQuantityRefusal, PreparedNormalizedQuantity};
use std::sync::OnceLock;

pub(crate) const HOST_CALL: &str = "conduit.host/normalized-quantity-scalar@1";
const IMPLEMENTATION: &str = "browser/kernel-normalized-quantity-scalar@1";
static CONVERTER: OnceLock<PreparedNormalizedQuantity> = OnceLock::new();

pub(super) static NORMALIZE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::normalized_quantity_semantic_contract();
    let target_kind = Some(contract.kind_id.clone());
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/normalized-quantity-scalar@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HOST_CALL.into(),
                target_kind,
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
                maximum_output_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
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
        next_request: 0,
        cancelled: false,
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
    next_request: u32,
    cancelled: bool,
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
            } if !self.pending && !self.cancelled => {
                let Ok(input) = BoundedValueRef::new(
                    value,
                    conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
                ) else {
                    return OperationAction::Fail(failure(11));
                };
                let request = RequestId(self.next_request);
                let Some(next_request) = self.next_request.checked_add(1) else {
                    return OperationAction::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 15,
                    });
                };
                self.next_request = next_request;
                self.pending = true;
                OperationAction::RequestHostCall {
                    request,
                    operation: HostCallId(0),
                    input,
                }
            }
            OperationInput::HostCallCompleted { request, outcome }
                if self.pending && request.0.checked_add(1) == Some(self.next_request) =>
            {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostCallDisposition::Completed, Some(output), None)
                        if output.admitted_bytes == 8 && output.value.byte_len == 8 =>
                    {
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostCallDisposition::Failed, None, Some(reason)) => {
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
        self.cancelled = true;
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
                next_request: 1,
                cancelled: false,
            };
            assert_eq!(
                operation.resume(OperationInput::HostCallCompleted {
                    request: RequestId(0),
                    outcome: conduit_kernel::HostCallOutcome {
                        disposition: HostCallDisposition::Failed,
                        output: None,
                        failure: Some(reason),
                    },
                }),
                OperationAction::Fail(reason)
            );
        }
    }
}
