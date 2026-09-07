//! Finite two-input House prompt projection and its per-Play host state.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_ai::WiredHouseContextItem;
use conduit_core::PlannedGear;
use conduit_kernel::{
    Failure, FailureCode, HostOperationDisposition, HostOperationId, OperationAction,
    OperationInput, PortId, RequestId,
};
use conduit_text::AddressDetection;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::HOUSE_PROMPT_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct HousePromptOperation {
    seen: [bool; 2],
    pending: Option<RequestId>,
    emitted: bool,
}

impl HousePromptOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(port),
                value,
            } if port < 2 && !self.seen[usize::from(port)] && self.pending.is_none() => {
                self.seen[usize::from(port)] = true;
                let request = RequestId(u32::from(port));
                self.pending = Some(request);
                let Ok(input) = conduit_kernel::BoundedValueRef::new(
                    value,
                    if port == 0 {
                        conduit_tongues::MAXIMUM_ADDRESS_DETECTION_VALUE_BYTES as u32
                    } else {
                        conduit_tongues::MAXIMUM_WIRED_HOUSE_CONTEXT_VALUE_BYTES as u32
                    },
                ) else {
                    return fail(FailureCode::InvalidInput, 4);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(if port == 0 { 1 } else { 0 }),
                    input,
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
                    (HostOperationDisposition::Completed, None, None) => OperationAction::Await,
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 1)
                    }
                    _ => fail(FailureCode::HostOperationFailed, 2),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 3),
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
    }
}

pub(super) enum HostCompletion<'a> {
    Stored,
    Output(&'a [u8]),
    NotAddressed,
}

pub(super) struct HousePromptHost {
    detection: Option<AddressDetection>,
    context: Option<Vec<WiredHouseContextItem>>,
    output: Vec<u8>,
}

impl HousePromptHost {
    fn new() -> Self {
        Self {
            detection: None,
            context: None,
            output: Vec::with_capacity(conduit_tongues::MAXIMUM_HOUSE_PROMPT_BYTES),
        }
    }

    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<HostCompletion<'_>, String> {
        match contract {
            conduit_std_offers::HOUSE_PROMPT_DETECTION_OPERATION => {
                if self.detection.is_some() {
                    return Err("duplicate House address detection".into());
                }
                self.detection = Some(
                    conduit_tongues::decode_address_detection(input)
                        .map_err(|error| format!("House address detection: {error:?}"))?,
                );
            }
            conduit_std_offers::HOUSE_PROMPT_CONTEXT_OPERATION => {
                if self.context.is_some() {
                    return Err("duplicate House context".into());
                }
                self.context = Some(
                    conduit_tongues::decode_wired_house_context(input)
                        .map_err(|error| format!("House context: {error:?}"))?,
                );
            }
            _ => return Err("unknown House prompt operation".into()),
        }
        let (Some(detection), Some(context)) = (&self.detection, &self.context) else {
            return Ok(HostCompletion::Stored);
        };
        let request =
            match conduit_tongues::prepare_house_generation_request(detection, context, 2_048) {
                Ok(request) => request,
                Err(conduit_tongues::HousePromptRefusal::NotAddressed) => {
                    return Ok(HostCompletion::NotAddressed)
                }
                Err(error) => return Err(format!("House prompt projection: {error:?}")),
            };
        self.output.clear();
        self.output
            .extend_from_slice(request.encoded_request.as_bytes());
        Ok(HostCompletion::Output(&self.output))
    }
}

pub(super) fn prepare_hosts(fragment: &conduit_core::PlanFragment) -> Vec<Option<HousePromptHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::HOUSE_PROMPT_STD_IMPLEMENTATION)
                .then(HousePromptHost::new)
        })
        .collect()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::house_prompt_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id.as_str() != conduit_std_offers::HOUSE_PROMPT_STD_PROFILE
        || placement.artifact_id.as_str() != conduit_std_offers::HOUSE_PROMPT_STD_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations.len() != 2
        || placement.host_operations[0].contract_id.as_str()
            != conduit_std_offers::HOUSE_PROMPT_CONTEXT_OPERATION
        || placement.host_operations[1].contract_id.as_str()
            != conduit_std_offers::HOUSE_PROMPT_DETECTION_OPERATION
    {
        return Err("planned House prompt identity does not match installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: conduit_tongues::MAXIMUM_HOUSE_PROMPT_BYTES as u32,
        host_requests: 2,
        sign_items: 16,
        maximum_value_bytes: conduit_tongues::MAXIMUM_WIRED_HOUSE_CONTEXT_VALUE_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::HousePrompt(HousePromptOperation {
        seen: [false; 2],
        pending: None,
        emitted: false,
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_ai::HouseContextProvenanceClass;

    fn context() -> Vec<WiredHouseContextItem> {
        vec![WiredHouseContextItem {
            item_identity: "context/temperature".into(),
            value_kind: "temperature/summary@1".into(),
            canonical_value: b"21 C".to_vec(),
            provenance: HouseContextProvenanceClass::ObservedSign,
            source_identity: "sign/temperature/1".into(),
        }]
    }

    #[test]
    fn either_input_order_emits_only_after_both_canonical_values() {
        let detection = conduit_tongues::encode_address_detection(&AddressDetection::Addressed {
            matched_name_index: 0,
            utterance: "temperature?".into(),
        })
        .unwrap();
        let context = conduit_tongues::encode_wired_house_context(&context()).unwrap();
        for inputs in [
            [
                (
                    conduit_std_offers::HOUSE_PROMPT_DETECTION_OPERATION,
                    detection.as_slice(),
                ),
                (
                    conduit_std_offers::HOUSE_PROMPT_CONTEXT_OPERATION,
                    context.as_slice(),
                ),
            ],
            [
                (
                    conduit_std_offers::HOUSE_PROMPT_CONTEXT_OPERATION,
                    context.as_slice(),
                ),
                (
                    conduit_std_offers::HOUSE_PROMPT_DETECTION_OPERATION,
                    detection.as_slice(),
                ),
            ],
        ] {
            let mut host = HousePromptHost::new();
            assert!(matches!(
                host.execute(inputs[0].0, inputs[0].1),
                Ok(HostCompletion::Stored)
            ));
            let output = host.execute(inputs[1].0, inputs[1].1).unwrap();
            let HostCompletion::Output(output) = output else {
                panic!("prompt absent")
            };
            let prompt = core::str::from_utf8(output).unwrap();
            assert!(prompt.contains("temperature?"));
            assert!(prompt.contains("sign/temperature/1"));
        }
    }

    #[test]
    fn not_addressed_refuses_after_context_without_output() {
        let detection =
            conduit_tongues::encode_address_detection(&AddressDetection::NotAddressed).unwrap();
        let context = conduit_tongues::encode_wired_house_context(&context()).unwrap();
        let mut host = HousePromptHost::new();
        assert!(matches!(
            host.execute(conduit_std_offers::HOUSE_PROMPT_CONTEXT_OPERATION, &context),
            Ok(HostCompletion::Stored)
        ));
        assert!(matches!(
            host.execute(
                conduit_std_offers::HOUSE_PROMPT_DETECTION_OPERATION,
                &detection
            ),
            Ok(HostCompletion::NotAddressed)
        ));
    }
}
