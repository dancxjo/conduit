//! Exact selected Quantity normalization through an admitted browser operation.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallRequirement, ImplementationId,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};
use conduit_semantic_catalog::{NormalizedQuantityRefusal, PreparedNormalizedQuantity};
use std::sync::OnceLock;

pub(crate) const HOST_CALL: &str = "conduit.host/normalized-quantity-scalar@1";
const IMPLEMENTATION: &str = "browser/kernel-normalized-quantity-scalar@1";
pub(crate) const RATIO_HOST_CALL: &str = "conduit.host/normalized-ratio-scalar@1";
const RATIO_IMPLEMENTATION: &str = "browser/kernel-normalized-ratio-scalar@1";
static RATIO_CONVERTER: OnceLock<PreparedNormalizedQuantity> = OnceLock::new();
static CONVERTER: OnceLock<PreparedNormalizedQuantity> = OnceLock::new();

pub(super) static NORMALIZE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

pub(super) static NORMALIZE_RATIO: BrowserInstallation = BrowserInstallation {
    implementation_id: RATIO_IMPLEMENTATION,
    offer: ratio_offer,
    prepare: prepare_ratio,
    perform: None,
};
fn offer() -> CapabilityOffer {
    offer_for(false)
}
fn ratio_offer() -> CapabilityOffer {
    offer_for(true)
}
fn offer_for(ratio: bool) -> CapabilityOffer {
    let implementation = if ratio {
        RATIO_IMPLEMENTATION
    } else {
        IMPLEMENTATION
    };
    let operation = if ratio { RATIO_HOST_CALL } else { HOST_CALL };
    let contract = if ratio {
        conduit_semantic_catalog::normalized_ratio_semantic_contract()
    } else {
        conduit_semantic_catalog::normalized_quantity_semantic_contract()
    };
    let target_kind = Some(contract.kind_id.clone());
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-browser-runtime/normalized-quantity-scalar@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: operation.into(),
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
) -> Result<BrowserBack, String> {
    prepare_for(placement, false)
}
fn prepare_ratio(
    placement: &conduit_core::PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    prepare_for(placement, true)
}
fn prepare_for(placement: &conduit_core::PlannedGear, ratio: bool) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer_for(ratio))?;
    if !placement.configuration.is_empty() {
        return Err("normalized Quantity conversion accepts no configuration".into());
    }
    if ratio {
        RATIO_CONVERTER.get_or_init(PreparedNormalizedQuantity::ratio);
    } else {
        CONVERTER.get_or_init(PreparedNormalizedQuantity::new);
    }
    Ok(BrowserBack::installed_step(NormalizeBack {
        pending: false,
        next_request: 0,
        cancelled: false,
    }))
}

pub(crate) fn transform(input: &[u8]) -> Result<[u8; conduit_core::SCALAR_ENCODED_LEN], Failure> {
    transform_with(input, &CONVERTER)
}
pub(crate) fn transform_ratio(
    input: &[u8],
) -> Result<[u8; conduit_core::SCALAR_ENCODED_LEN], Failure> {
    transform_with(input, &RATIO_CONVERTER)
}
fn transform_with(
    input: &[u8],
    converter: &OnceLock<PreparedNormalizedQuantity>,
) -> Result<[u8; conduit_core::SCALAR_ENCODED_LEN], Failure> {
    let converter = converter.get().ok_or(failure(14))?;
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

struct NormalizeBack {
    pending: bool,
    next_request: u32,
    cancelled: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for NormalizeBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request.0.checked_add(1) != Some(self.next_request) {
                return StepOutcome::Fail(failure(11));
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None)
                    if output.admitted_bytes == 8 && output.value.byte_len == 8 =>
                {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed normalized Quantity completion");
                    io.send(PortId(0), output.value)
                        .expect("ready normalized Quantity output");
                    self.pending = false;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Failed, None, Some(reason)) => {
                    return StepOutcome::Fail(reason)
                }
                _ => return StepOutcome::Fail(failure(11)),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if !self.pending && !self.cancelled {
                let Ok(input) = BoundedValueRef::new(
                    value,
                    conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
                ) else {
                    return StepOutcome::Fail(failure(11));
                };
                let request = RequestId(self.next_request);
                let Some(next_request) = self.next_request.checked_add(1) else {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 15,
                    });
                };
                io.consume(PortId(0))
                    .expect("present normalized Quantity input");
                io.request_host_call(request, HostCallId(0), input)
                    .expect("normalized Quantity Host Call");
                self.next_request = next_request;
                self.pending = true;
                return StepOutcome::Progress;
            }
            return StepOutcome::Fail(failure(11));
        }
        if io.input_closed(PortId(0)) && !self.pending {
            io.consume_closed(PortId(0))
                .expect("observed normalized Quantity closure");
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
            transform(&leaf(250_000, conduit_core::Unit::Millionth)),
            Ok(conduit_core::Scalar::from_raw_microunits(250_000).encode())
        );
        for (bytes, detail) in [
            (Vec::new(), 11),
            (leaf(50, conduit_core::Unit::Percent), 12),
            (leaf(-1, conduit_core::Unit::Millionth), 13),
        ] {
            let reason = transform(&bytes).unwrap_err();
            assert_eq!(reason, failure(detail));
            let mut operation = NormalizeBack {
                pending: true,
                next_request: 1,
                cancelled: false,
            };
            let mut io = StepIo::test_frame(
                [None],
                [false],
                [Some(8)],
                Some((
                    RequestId(0),
                    conduit_kernel::HostCallOutcome {
                        disposition: HostCallDisposition::Failed,
                        output: None,
                        failure: Some(reason),
                    },
                )),
                4,
            );
            assert_eq!(
                operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
                StepOutcome::Fail(reason)
            );
        }
    }
}
