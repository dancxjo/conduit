//! Browser installations for recurring timer-driven current count.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{
    kind_id, resource_requirement, wait_host_operation_requirement, ConfigurationValue,
    PlannedGear, PRESENTATION_RESOURCE_CLASS, TIMER_RESOURCE_CLASS,
};
use conduit_kernel::{
    BoundedValueRef, CanonicalValue, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

const ARTIFACT: &str = "conduit-browser-runtime/installed-state-time@1";
const TIME_EVERY_IMPLEMENTATION: &str = "browser/kernel-time-every@1";
const STATE_COUNT_IMPLEMENTATION: &str = "browser/kernel-state-count@1";
const COUNT_PRESENTATION_IMPLEMENTATION: &str = "browser/presentation-count@1";
const COUNT_PRESENTATION_OPERATION: &str = "conduit.host/browser-present-count@1";
pub(crate) const BROWSER_TIMER_MAXIMUM_MILLIS: u64 = 10_000;

pub(super) static TIME_EVERY: BrowserInstallation = BrowserInstallation {
    implementation_id: TIME_EVERY_IMPLEMENTATION,
    offer: time_every_offer,
    prepare: prepare_time_every,
    perform: None,
};

pub(super) static STATE_COUNT: BrowserInstallation = BrowserInstallation {
    implementation_id: STATE_COUNT_IMPLEMENTATION,
    offer: state_count_offer,
    prepare: prepare_state_count,
    perform: None,
};

pub(super) static COUNT_PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: COUNT_PRESENTATION_IMPLEMENTATION,
    offer: count_presentation_offer,
    prepare: prepare_count_presentation,
    perform: Some(perform_count_presentation),
};

fn time_every_offer() -> conduit_core::CapabilityOffer {
    let mut offer = conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::time_every_contract(),
        conduit_time::TIME_EVERY_CONTRACT_REVISION,
        identity(TIME_EVERY_IMPLEMENTATION),
        vec![wait_host_operation_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        Vec::new(),
    );
    offer.startup_parameters[0].value_type = "Duration".into();
    offer.startup_parameters[0].has_default = false;
    offer
}

fn state_count_offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::state_count_contract(),
        conduit_semantic_catalog::STATE_COUNT_CONTRACT_REVISION,
        identity(STATE_COUNT_IMPLEMENTATION),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn count_presentation_offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::count_presentation_contract(),
        conduit_semantic_catalog::COUNT_PRESENTATION_CONTRACT_REVISION,
        identity(COUNT_PRESENTATION_IMPLEMENTATION),
        vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(COUNT_PRESENTATION_OPERATION),
            target_kind: Some(kind_id("presentation/browser-count")),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_semantic_catalog::COUNT_ENCODED_LEN,
            maximum_output_bytes: 0,
        }],
        vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        Vec::new(),
    )
}

fn identity(
    implementation: &'static str,
) -> conduit_semantic_catalog::RealizationOfferIdentity<'static> {
    conduit_semantic_catalog::RealizationOfferIdentity {
        capability: implementation,
        execution_profile: implementation,
        implementation,
        artifact: ARTIFACT,
    }
}

fn prepare_time_every(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &time_every_offer())?;
    let period_millis = configuration(placement, "freq", BROWSER_TIMER_MAXIMUM_MILLIS)?;
    let wait = values
        .store(&period_millis.to_le_bytes())
        .map_err(debug_error)?;
    Ok(BrowserOperation::installed(TimeEveryOperation {
        wait,
        sequence: 0,
        next_request: 0,
        pending: None,
    }))
}

fn prepare_state_count(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &state_count_offer())?;
    let start = configuration(placement, "start", u64::MAX)?;
    Ok(BrowserOperation::installed(StateCountOperation {
        current: start,
        initial_emitted: false,
    }))
}

fn prepare_count_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &count_presentation_offer())?;
    Ok(BrowserOperation::presentation(
        conduit_semantic_catalog::COUNT_ENCODED_LEN,
        1,
    ))
}

fn perform_count_presentation(
    _placement: &PlannedGear,
    input: &[u8],
) -> Result<BrowserHostResult, String> {
    let encoded: [u8; conduit_semantic_catalog::COUNT_ENCODED_LEN as usize] = input
        .try_into()
        .map_err(|_| "count manifestation is not an exact Count")?;
    let _count = u64::from_le_bytes(encoded);
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::COUNT_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn configuration(placement: &PlannedGear, key: &str, maximum: u64) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            (found, ConfigurationValue::U64(value)) if found == key && *value <= maximum => {
                Some(*value)
            }
            _ => None,
        })
        .ok_or_else(|| {
            format!(
                "{} configuration '{key}' is missing or exceeds the browser bound",
                placement.kind_id.as_str()
            )
        })
}

struct TimeEveryOperation {
    wait: ValueRef,
    sequence: u64,
    next_request: u32,
    pending: Option<RequestId>,
}

impl TimeEveryOperation {
    fn request_wait(&mut self) -> OperationAction {
        let request = RequestId(self.next_request);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.wait, conduit_time::TICK_ENCODED_LEN)
                .expect("browser timer duration is exactly eight bytes"),
        }
    }
}

impl Operation for TimeEveryOperation {
    fn start(&mut self) -> OperationAction {
        self.request_wait()
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                let encoded = conduit_time::encode_tick(self.sequence);
                let Ok(value) = CanonicalValue::new(&encoded) else {
                    return fail(20);
                };
                OperationAction::EmitCanonical {
                    port: PortId(0),
                    value,
                }
            }
            _ => fail(20),
        }
    }

    fn advance(&mut self) -> OperationAction {
        let Some(sequence) = self.sequence.checked_add(1) else {
            return OperationAction::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::IdentityCapacityExhausted,
                detail: 20,
            });
        };
        let Some(next_request) = self.next_request.checked_add(1) else {
            return OperationAction::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::IdentityCapacityExhausted,
                detail: 20,
            });
        };
        self.sequence = sequence;
        self.next_request = next_request;
        self.request_wait()
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

struct StateCountOperation {
    current: u64,
    initial_emitted: bool,
}

impl Operation for StateCountOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::EmitCanonical {
            port: PortId(0),
            value: CanonicalValue::new(&self.current.to_le_bytes()).expect("Count is eight bytes"),
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.initial_emitted && value.byte_len == conduit_time::TICK_ENCODED_LEN => {
                let Some(current) = self.current.checked_add(1) else {
                    return OperationAction::Fail(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::IdentityCapacityExhausted,
                        detail: 21,
                    });
                };
                self.current = current;
                OperationAction::EmitCanonical {
                    port: PortId(0),
                    value: CanonicalValue::new(&current.to_le_bytes())
                        .expect("Count is eight bytes"),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.initial_emitted => {
                OperationAction::Complete
            }
            _ => fail(21),
        }
    }

    fn advance(&mut self) -> OperationAction {
        self.initial_emitted = true;
        OperationAction::Await
    }
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostOperationOutcome;

    #[test]
    fn every_rearms_one_wait_and_emits_tick_five_and_later() {
        let wait = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: conduit_time::TICK_ENCODED_LEN,
        };
        let mut operation = TimeEveryOperation {
            wait,
            sequence: 0,
            next_request: 0,
            pending: None,
        };
        let mut action = operation.start();
        for sequence in 0..7_u64 {
            assert!(matches!(
                action,
                OperationAction::RequestHostOperation {
                    request: RequestId(found),
                    ..
                } if u64::from(found) == sequence
            ));
            let emitted = operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(sequence as u32),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            });
            let OperationAction::EmitCanonical { port, value } = emitted else {
                panic!("recurring interval did not emit tick {sequence}");
            };
            assert_eq!(port, PortId(0));
            assert_eq!(value.as_slice(), conduit_time::encode_tick(sequence));
            action = operation.advance();
        }
        assert!(matches!(
            action,
            OperationAction::RequestHostOperation { .. }
        ));
    }
}
