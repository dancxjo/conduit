use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, PortDirection, PortTemporal};
use conduit_kernel::{
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostOperationDisposition,
    HostOperationId, OperationAction, OperationInput, PortId, RequestId,
};

pub(super) static STATE_COUNT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::STATE_COUNT_IMPLEMENTATION,
    budget: state_count_budget,
    prepare: prepare_state_count,
};

pub(super) static COUNT_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::COUNT_PRESENTATION_IMPLEMENTATION,
    budget: count_presentation_budget,
    prepare: prepare_count_presentation,
};

pub(super) struct StateCountOperation {
    current: u64,
    initial_emitted: bool,
}

pub(super) struct CountPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
}

impl StateCountOperation {
    pub(super) fn allocation_capacity(&self) -> usize {
        0
    }

    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::EmitCanonical {
            port: PortId(0),
            value: CanonicalValue::new(&self.current.to_le_bytes()).expect("Count is eight bytes"),
        }
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.initial_emitted && value.byte_len == conduit_time::TICK_ENCODED_LEN => {
                let Some(current) = self.current.checked_add(1) else {
                    return OperationAction::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 10,
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
            _ => InstalledOperation::fail(10),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.initial_emitted = true;
        OperationAction::Await
    }
}

impl CountPresentationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() => {
                let request = RequestId(self.next);
                let Some(next) = self.next.checked_add(1) else {
                    return OperationAction::Fail(Failure {
                        code: FailureCode::IdentityCapacityExhausted,
                        detail: 11,
                    });
                };
                self.pending = Some(request);
                self.next = next;
                let Ok(input) =
                    BoundedValueRef::new(value, conduit_semantic_catalog::COUNT_ENCODED_LEN)
                else {
                    return InstalledOperation::fail(11);
                };
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(11),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

fn state_count_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_state_count(placement)?;
    Ok(OperationBudget {
        // The current value plus four admitted downstream queue entries can
        // coexist while this stateful transform remains open.
        value_items: 5,
        value_bytes: conduit_semantic_catalog::COUNT_ENCODED_LEN * 5,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: conduit_semantic_catalog::COUNT_ENCODED_LEN,
    })
}

fn prepare_state_count(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_state_count(placement)?;
    let start = count_configuration(placement, "start", u64::MAX)?;
    Ok(InstalledOperation::StateCount(StateCountOperation {
        current: start,
        initial_emitted: false,
    }))
}

fn count_presentation_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_count_presentation(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 64,
        maximum_value_bytes: conduit_semantic_catalog::COUNT_ENCODED_LEN,
    })
}

fn prepare_count_presentation(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_count_presentation(placement)?;
    Ok(InstalledOperation::CountPresentation(
        CountPresentationOperation {
            pending: None,
            next: 0,
        },
    ))
}

fn count_configuration(placement: &PlannedGear, key: &str, maximum: u64) -> Result<u64, String> {
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
                "{} configuration '{key}' is missing or invalid",
                placement.kind_id.as_str()
            )
        })
}

fn validate_state_count(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        conduit_semantic_catalog::STATE_COUNT_KIND,
        conduit_semantic_catalog::STATE_COUNT_CONTRACT_REVISION,
        conduit_std_offers::STATE_COUNT_EXECUTION_PROFILE,
        conduit_std_offers::STATE_COUNT_IMPLEMENTATION,
        conduit_std_offers::STATE_COUNT_ARTIFACT,
        "bump",
        conduit_time::TICK_VALUE_KIND,
        PortTemporal::Flow { closes: false },
        Some((
            "value",
            conduit_semantic_catalog::STATE_COUNT_VALUE_KIND,
            PortTemporal::Current,
        )),
    )?;
    count_configuration(placement, "start", u64::MAX).map(|_| ())
}

fn validate_count_presentation(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        conduit_semantic_catalog::COUNT_PRESENTATION_KIND,
        conduit_semantic_catalog::COUNT_PRESENTATION_CONTRACT_REVISION,
        conduit_std_offers::COUNT_PRESENTATION_EXECUTION_PROFILE,
        conduit_std_offers::COUNT_PRESENTATION_IMPLEMENTATION,
        conduit_std_offers::COUNT_PRESENTATION_ARTIFACT,
        "value",
        conduit_semantic_catalog::STATE_COUNT_VALUE_KIND,
        PortTemporal::Current,
        None,
    )?;
    placement
        .configuration
        .is_empty()
        .then_some(())
        .ok_or_else(|| "presentation/count accepts no lifetime configuration".to_string())
}

#[allow(clippy::too_many_arguments)]
fn validate_identity(
    placement: &PlannedGear,
    kind: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    input_name: &str,
    input_kind: &str,
    input_temporal: PortTemporal,
    output: Option<(&str, &str, PortTemporal)>,
) -> Result<(), String> {
    let output_matches = match output {
        Some((name, kind, temporal)) => placement.outputs.first().is_some_and(|port| {
            placement.outputs.len() == 1
                && port.port_id.as_str() == name
                && port.value_kind.as_str() == kind
                && port.direction == PortDirection::Output
                && port.temporal == temporal
        }),
        None => placement.outputs.is_empty(),
    };
    if placement.kind_id.as_str() != kind
        || placement.kind_contract_revision.as_str() != revision
        || placement.execution_profile_id.as_str() != profile
        || placement.implementation_id.as_str() != implementation
        || placement.artifact_id.as_str() != artifact
        || placement.inputs.len() != 1
        || placement.inputs[0].port_id.as_str() != input_name
        || placement.inputs[0].value_kind.as_str() != input_kind
        || placement.inputs[0].direction != PortDirection::Input
        || placement.inputs[0].temporal != input_temporal
        || !output_matches
    {
        return Err(format!(
            "planned {kind} executable identity does not match its installation"
        ));
    }
    Ok(())
}

pub(super) fn decode_count(bytes: &[u8]) -> Result<u64, String> {
    let encoded: [u8; conduit_semantic_catalog::COUNT_ENCODED_LEN as usize] = bytes
        .try_into()
        .map_err(|_| "count presentation input is not an exact Count".to_string())?;
    Ok(u64::from_le_bytes(encoded))
}
