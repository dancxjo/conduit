use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ConfigurationValue, InfoBool, PlannedGear, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef,
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

impl<const PORTS: usize> StepOperation<PORTS> for FlowGateScalarOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending_enable.map(|pending| pending.0) != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
            {
                return StepOutcome::Fail(gate_failure());
            }
            let (_, input) = self
                .pending_enable
                .expect("matching gate request has an input");
            match outcome.output {
                None => self.enabled = false,
                Some(output)
                    if output.value == input
                        && output.admitted_bytes == BOOL_ENCODED_LEN as u32 =>
                {
                    self.enabled = true;
                }
                Some(_) => return StepOutcome::Fail(gate_failure()),
            }
            io.consume_host_completion()
                .expect("observed gate Host Call completion");
            self.pending_enable = None;
            self.next_request = self.next_request.saturating_add(1);
            return StepOutcome::Progress;
        }
        if self.pending_enable.is_some() {
            return StepOutcome::Await;
        }
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len != SCALAR_ENCODED_LEN as u32 {
                return StepOutcome::Fail(gate_failure());
            }
            if self.enabled && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).expect("present gated data input");
            if self.enabled {
                io.send(PortId(0), value).expect("ready gated data output");
            }
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(1)) {
            if value.byte_len != BOOL_ENCODED_LEN as u32
                || self.next_request >= self.maximum_enable_updates
            {
                return StepOutcome::Fail(gate_failure());
            }
            let Ok(input) = BoundedValueRef::new(value, BOOL_ENCODED_LEN as u32) else {
                return StepOutcome::Fail(gate_failure());
            };
            let request = RequestId(self.next_request);
            io.consume(PortId(1)).expect("present gate enable input");
            io.request_host_call(request, HostCallId(0), input)
                .expect("single gate Host Call");
            self.pending_enable = Some((request, value));
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && !self.data_closed {
            io.consume_closed(PortId(0))
                .expect("observed gate data closure");
            self.data_closed = true;
            return if self.enable_closed {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            };
        }
        if io.input_closed(PortId(1)) && !self.enable_closed {
            io.consume_closed(PortId(1))
                .expect("observed gate enable closure");
            self.enable_closed = true;
            return if self.data_closed {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            };
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending_enable = None;
    }
}

fn gate_failure() -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail: 16,
    }
}

impl FlowGateScalarOperation {}

pub(super) fn decode_bool(input: &[u8]) -> Result<bool, String> {
    InfoBool::decode(input)
        .map(InfoBool::get)
        .map_err(|error| format!("flow/gate enable is not canonical value/bool: {error:?}"))
}

fn maximum_enable_updates(placement: &PlannedGear) -> Result<u32, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-enable-updates", ConfigurationValue::U64(value))
                if (1..=u64::from(conduit_semantic_catalog::FLOW_STATE_MAXIMUM_VALUES))
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
    if placement.kind_id.as_str() != conduit_semantic_catalog::GATE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::FLOW_GATE_SCALAR_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::FLOW_GATE_SCALAR_EXECUTION_PROFILE
        || placement.implementation_id.as_str()
            != conduit_std_offers::FLOW_GATE_SCALAR_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::FLOW_GATE_SCALAR_ARTIFACT
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.configuration.len() != 1
    {
        return Err("planned flow/gate scalar identity does not match its installation".into());
    }
    maximum_enable_updates(placement).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostCallOutcome;

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
            OperationAction::RequestHostCall {
                request: RequestId(0),
                ..
            }
        ));
        assert_eq!(
            gate.resume(OperationInput::HostCallCompleted {
                request: RequestId(0),
                outcome: HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
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
        gate.resume(OperationInput::HostCallCompleted {
            request: RequestId(1),
            outcome: HostCallOutcome {
                disposition: HostCallDisposition::Completed,
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
            gate.resume(OperationInput::HostCallCompleted {
                request: RequestId(0),
                outcome: HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
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
