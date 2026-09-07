//! Finite two-input address detection and per-Play host state.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::PlannedGear;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::ADDRESS_DETECT_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct AddressDetectOperation {
    seen: [bool; 2],
    pending: Option<RequestId>,
    deferred: Option<(u16, ValueRef)>,
    next_request: u32,
    emitted: bool,
}

impl AddressDetectOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(port),
                value,
            } if port < 2 && !self.seen[usize::from(port)] => {
                self.seen[usize::from(port)] = true;
                if self.pending.is_some() {
                    if self.deferred.replace((port, value)).is_some() {
                        return fail(FailureCode::InvalidLifecycle, 14);
                    }
                    OperationAction::Await
                } else {
                    self.request(port, value)
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) => self
                        .deferred
                        .take()
                        .map_or(OperationAction::Await, |(port, value)| {
                            self.request(port, value)
                        }),
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 2)
                    }
                    _ => fail(FailureCode::HostOperationFailed, 13),
                }
            }
            OperationInput::Closed { port: PortId(port) }
                if port < 2 && self.seen[usize::from(port)] =>
            {
                OperationAction::Await
            }
            _ => fail(FailureCode::InvalidLifecycle, 14),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.deferred = None;
    }

    fn request(&mut self, port: u16, value: ValueRef) -> OperationAction {
        let request = RequestId(self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        self.pending = Some(request);
        let maximum = if port == 0 {
            conduit_text::MAX_TEXT_BYTES
        } else {
            conduit_text::MAX_ADDRESS_SET_VALUE_BYTES as u32
        };
        let Ok(input) = BoundedValueRef::new(value, maximum) else {
            return fail(FailureCode::InvalidInput, 1);
        };
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(if port == 0 { 1 } else { 0 }),
            input,
        }
    }
}

pub(super) enum HostCompletion<'a> {
    Stored,
    Output(&'a [u8]),
}

pub(super) struct AddressDetectHost {
    recognized: Option<String>,
    addresses: Option<conduit_text::AddressSet>,
    output: Vec<u8>,
}

impl AddressDetectHost {
    fn new() -> Self {
        Self {
            recognized: None,
            addresses: None,
            output: Vec::with_capacity(conduit_text::MAX_ADDRESS_DETECTION_VALUE_BYTES),
        }
    }

    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<HostCompletion<'_>, String> {
        match contract {
            conduit_std_offers::ADDRESS_DETECT_RECOGNIZED_OPERATION => {
                if self.recognized.is_some() {
                    return Err("duplicate recognized text".into());
                }
                let recognized = core::str::from_utf8(input)
                    .map_err(|_| "recognized text is not UTF-8".to_string())?;
                if recognized.len() > conduit_text::MAX_TEXT_BYTES as usize {
                    return Err("recognized text exceeds its bound".into());
                }
                self.recognized = Some(recognized.into());
            }
            conduit_std_offers::ADDRESS_DETECT_ADDRESSES_OPERATION => {
                if self.addresses.is_some() {
                    return Err("duplicate address set".into());
                }
                self.addresses = Some(
                    conduit_text::decode_address_set(input)
                        .map_err(|error| format!("address set: {error:?}"))?,
                );
            }
            _ => return Err("unknown address-detect host operation".into()),
        }
        let (Some(recognized), Some(addresses)) = (&self.recognized, &self.addresses) else {
            return Ok(HostCompletion::Stored);
        };
        let detection = addresses
            .detect(recognized)
            .map_err(|error| format!("address detection: {error:?}"))?;
        self.output = conduit_text::encode_address_detection(&detection)
            .map_err(|error| format!("encode address detection: {error:?}"))?;
        Ok(HostCompletion::Output(&self.output))
    }
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Vec<Option<AddressDetectHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::ADDRESS_DETECT_IMPLEMENTATION)
                .then(AddressDetectHost::new)
        })
        .collect()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::address_detect_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::ADDRESS_DETECT_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::ADDRESS_DETECT_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::ADDRESS_DETECT_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
    {
        return Err("planned address detection does not match installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: conduit_text::MAX_ADDRESS_DETECTION_VALUE_BYTES as u32,
        host_requests: 2,
        sign_items: 32,
        maximum_value_bytes: 4_096,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::AddressDetect(AddressDetectOperation {
        seen: [false; 2],
        pending: None,
        deferred: None,
        next_request: 0,
        emitted: false,
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addresses() -> Vec<u8> {
        conduit_text::encode_address_set(
            &conduit_text::AddressSet::new(&["Rosehip House", "Rosehip"]).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn exact_inputs_produce_the_same_detection_in_either_arrival_order() {
        for recognized_first in [true, false] {
            let mut host = AddressDetectHost::new();
            let addresses = addresses();
            let first = if recognized_first {
                host.execute(
                    conduit_std_offers::ADDRESS_DETECT_RECOGNIZED_OPERATION,
                    b"Rosehip House, status",
                )
            } else {
                host.execute(
                    conduit_std_offers::ADDRESS_DETECT_ADDRESSES_OPERATION,
                    &addresses,
                )
            };
            assert!(matches!(first.unwrap(), HostCompletion::Stored));
            let second = if recognized_first {
                host.execute(
                    conduit_std_offers::ADDRESS_DETECT_ADDRESSES_OPERATION,
                    &addresses,
                )
            } else {
                host.execute(
                    conduit_std_offers::ADDRESS_DETECT_RECOGNIZED_OPERATION,
                    b"Rosehip House, status",
                )
            };
            let HostCompletion::Output(output) = second.unwrap() else {
                panic!("second exact input did not produce detection")
            };
            assert_eq!(
                conduit_text::decode_address_detection(output).unwrap(),
                conduit_text::AddressDetection::Addressed {
                    matched_name_index: 0,
                    utterance: "status".into(),
                }
            );
        }
    }

    #[test]
    fn unaddressed_text_is_an_explicit_value_and_malformed_addresses_refuse() {
        let mut host = AddressDetectHost::new();
        assert!(matches!(
            host.execute(
                conduit_std_offers::ADDRESS_DETECT_RECOGNIZED_OPERATION,
                b"what is the status?",
            )
            .unwrap(),
            HostCompletion::Stored
        ));
        let HostCompletion::Output(output) = host
            .execute(
                conduit_std_offers::ADDRESS_DETECT_ADDRESSES_OPERATION,
                &addresses(),
            )
            .unwrap()
        else {
            panic!("complete inputs did not produce detection")
        };
        assert_eq!(
            conduit_text::decode_address_detection(output).unwrap(),
            conduit_text::AddressDetection::NotAddressed
        );

        let mut malformed = AddressDetectHost::new();
        assert!(malformed
            .execute(
                conduit_std_offers::ADDRESS_DETECT_ADDRESSES_OPERATION,
                b"not canonical addresses",
            )
            .is_err());
    }
}
