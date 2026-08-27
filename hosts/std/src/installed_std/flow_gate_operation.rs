use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ConfigurationValue, InfoBool, PlannedGear, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef,
};

pub(super) static FLOW_GATE_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::FLOW_GATE_SCALAR_IMPLEMENTATION,
    budget: flow_gate_budget,
    prepare: prepare_flow_gate,
};

pub(super) struct FlowGateScalarOperation {
    enabled: bool,
    pending_enable: Option<(RequestId, ValueRef)>,
    next_request: u32,
    maximum_enable_updates: u32,
    data_closed: bool,
    enable_closed: bool,
}

impl FlowGateScalarOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if value.byte_len == SCALAR_ENCODED_LEN as u32 && self.pending_enable.is_none() => {
                if self.enabled {
                    OperationAction::Emit {
                        port: PortId(0),
                        value,
                    }
                } else {
                    OperationAction::Await
                }
            }
            OperationInput::Value {
                port: PortId(1),
                value,
            } if value.byte_len == BOOL_ENCODED_LEN as u32
                && self.pending_enable.is_none()
                && self.next_request < self.maximum_enable_updates =>
            {
                let request = RequestId(self.next_request);
                self.pending_enable = Some((request, value));
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, BOOL_ENCODED_LEN as u32) {
                        Ok(input) => input,
                        Err(_) => return InstalledOperation::fail(16),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending_enable.map(|pending| pending.0) == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                let (_, input) = self
                    .pending_enable
                    .take()
                    .expect("matching gate request has an input");
                self.next_request = self.next_request.saturating_add(1);
                match outcome.output {
                    None => self.enabled = false,
                    Some(output)
                        if output.value == input
                            && output.admitted_bytes == BOOL_ENCODED_LEN as u32 =>
                    {
                        self.enabled = true;
                    }
                    Some(_) => return InstalledOperation::fail(16),
                }
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending_enable.is_none() => {
                self.data_closed = true;
                self.terminal_or_await()
            }
            OperationInput::Closed { port: PortId(1) } if self.pending_enable.is_none() => {
                self.enable_closed = true;
                self.terminal_or_await()
            }
            _ => InstalledOperation::fail(16),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.pending_enable = None;
    }

    fn terminal_or_await(&self) -> OperationAction {
        if self.data_closed && self.enable_closed {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
}

pub(super) fn decode_bool(input: &[u8]) -> Result<bool, String> {
    InfoBool::decode(input)
        .map(InfoBool::get)
        .map_err(|error| format!("flow/gate enable is not canonical value/bool@1: {error:?}"))
}

fn maximum_enable_updates(placement: &PlannedGear) -> Result<u32, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-enable-updates", ConfigurationValue::U64(value))
                if (1..=u64::from(conduit_std_catalog::FLOW_STATE_MAXIMUM_VALUES))
                    .contains(value) =>
            {
                u32::try_from(*value).ok()
            }
            _ => None,
        })
        .ok_or_else(|| "flow/gate maximum-enable-updates is missing or invalid".into())
}

fn flow_gate_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_flow_gate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: maximum_enable_updates(placement)? as usize,
        sign_items: 128,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare_flow_gate(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_flow_gate(placement)?;
    Ok(InstalledOperation::FlowGateScalar(
        FlowGateScalarOperation {
            enabled: false,
            pending_enable: None,
            next_request: 0,
            maximum_enable_updates: maximum_enable_updates(placement)?,
            data_closed: false,
            enable_closed: false,
        },
    ))
}

fn validate_flow_gate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::flow_gate_scalar_offer();
    if placement.kind_id.as_str() != conduit_std_catalog::GATE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_std_catalog::FLOW_GATE_SCALAR_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::FLOW_GATE_SCALAR_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::FLOW_GATE_SCALAR_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::FLOW_GATE_SCALAR_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.configuration.len() != 1
    {
        return Err("planned flow/gate scalar identity does not match its installation".into());
    }
    maximum_enable_updates(placement).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostOperationOutcome;

    fn value(slot: u16, byte_len: u32) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len,
        }
    }

    #[test]
    fn gate_defaults_closed_then_tracks_exact_false_and_true_completions() {
        let mut gate = FlowGateScalarOperation {
            enabled: false,
            pending_enable: None,
            next_request: 0,
            maximum_enable_updates: 2,
            data_closed: false,
            enable_closed: false,
        };
        let scalar = value(1, SCALAR_ENCODED_LEN as u32);
        assert_eq!(
            gate.resume(OperationInput::Value {
                port: PortId(0),
                value: scalar,
            }),
            OperationAction::Await
        );

        let enabled = value(2, BOOL_ENCODED_LEN as u32);
        assert!(matches!(
            gate.resume(OperationInput::Value {
                port: PortId(1),
                value: enabled,
            }),
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                ..
            }
        ));
        assert_eq!(
            gate.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(BoundedValueRef::new(enabled, 1).unwrap()),
                    failure: None,
                },
            }),
            OperationAction::Await
        );
        assert!(matches!(
            gate.resume(OperationInput::Value {
                port: PortId(0),
                value: scalar,
            }),
            OperationAction::Emit { value, .. } if value == scalar
        ));

        let disabled = value(3, BOOL_ENCODED_LEN as u32);
        gate.resume(OperationInput::Value {
            port: PortId(1),
            value: disabled,
        });
        gate.resume(OperationInput::HostOperationCompleted {
            request: RequestId(1),
            outcome: HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        });
        assert_eq!(
            gate.resume(OperationInput::Value {
                port: PortId(0),
                value: scalar,
            }),
            OperationAction::Await
        );
    }

    #[test]
    fn gate_waits_for_both_closures_and_bool_codec_rejects_noncanonical_bytes() {
        let mut gate = FlowGateScalarOperation {
            enabled: false,
            pending_enable: None,
            next_request: 0,
            maximum_enable_updates: 1,
            data_closed: false,
            enable_closed: false,
        };
        assert_eq!(
            gate.resume(OperationInput::Closed { port: PortId(1) }),
            OperationAction::Await
        );
        assert_eq!(
            gate.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );
        assert!(!decode_bool(&[0]).unwrap());
        assert!(decode_bool(&[1]).unwrap());
        assert!(decode_bool(&[2]).is_err());
        assert!(decode_bool(&[]).is_err());
    }

    #[test]
    fn gate_cancellation_clears_pending_decode_and_update_bound_fails_closed() {
        let mut gate = FlowGateScalarOperation {
            enabled: false,
            pending_enable: None,
            next_request: 0,
            maximum_enable_updates: 1,
            data_closed: false,
            enable_closed: false,
        };
        let boolean = value(4, BOOL_ENCODED_LEN as u32);
        gate.resume(OperationInput::Value {
            port: PortId(1),
            value: boolean,
        });
        gate.cancel();
        assert!(gate.pending_enable.is_none());
        assert!(matches!(
            gate.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            }),
            OperationAction::Fail(_)
        ));

        let mut bounded = FlowGateScalarOperation {
            enabled: false,
            pending_enable: None,
            next_request: 1,
            maximum_enable_updates: 1,
            data_closed: false,
            enable_closed: false,
        };
        assert!(matches!(
            bounded.resume(OperationInput::Value {
                port: PortId(1),
                value: boolean,
            }),
            OperationAction::Fail(_)
        ));
    }
}
