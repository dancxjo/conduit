use super::contract::{
    encode_tick, parse_tick_configuration, TICK_ARTIFACT, TICK_CONTRACT_REVISION, TICK_ENCODED_LEN,
    TICK_EXECUTION_PROFILE, TICK_IMPLEMENTATION, TICK_KIND, TICK_VALUE_KIND,
};
use super::contract::{
    MAX_TEXT_BYTES, MAX_TEXT_VALUES, TEXT_PRESENTATION_ARTIFACT,
    TEXT_PRESENTATION_CONTRACT_REVISION, TEXT_PRESENTATION_EXECUTION_PROFILE,
    TEXT_PRESENTATION_IMPLEMENTATION, TEXT_PRESENTATION_KIND, TEXT_PRESENTATION_VALUE_KIND,
};
use conduit_core::{PlannedOperation, PortDirection};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) struct OperationBudget {
    pub(super) value_items: u16,
    pub(super) value_bytes: u32,
    pub(super) host_requests: usize,
    pub(super) evidence_items: u16,
    pub(super) maximum_value_bytes: u32,
}

pub(super) struct InstalledFactory {
    pub(super) implementation_id: &'static str,
    pub(super) budget: fn(&PlannedOperation) -> Result<OperationBudget, String>,
    pub(super) prepare: fn(
        &PlannedOperation,
        &mut conduit_kernel::HostedValueStore,
    ) -> Result<InstalledOperation, String>,
}

pub(super) static TICK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TICK_IMPLEMENTATION,
    budget: tick_budget,
    prepare: prepare_tick,
};

pub(super) static TEXT_PRESENTATION_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TEXT_PRESENTATION_IMPLEMENTATION,
    budget: text_presentation_budget,
    prepare: prepare_text_presentation,
};

#[cfg(test)]
pub(super) const TEST_OBSERVER_KIND: &str = "conduit.test/tick-observer";
#[cfg(test)]
pub(super) const TEST_OBSERVER_IMPLEMENTATION: &str = "conduit.test/tick-observer@1";

#[cfg(test)]
pub(super) static TEST_OBSERVER_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TEST_OBSERVER_IMPLEMENTATION,
    budget: |_| {
        Ok(OperationBudget {
            value_items: 0,
            value_bytes: 0,
            host_requests: 0,
            evidence_items: 16,
            maximum_value_bytes: TICK_ENCODED_LEN,
        })
    },
    prepare: prepare_test_observer,
};

pub(super) enum InstalledOperation {
    Tick(TickOperation),
    TextPresentation(TextPresentationOperation),
    #[cfg(test)]
    TestTextSource(super::test_text_source::TestTextSourceOperation),
    #[cfg(test)]
    TestObserver(TestObserverOperation),
    Inactive,
}

pub(super) struct TickOperation {
    values: Vec<ValueRef>,
    waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

pub(super) struct TextPresentationOperation {
    pending: Option<RequestId>,
    next: u32,
    maximum_values: u32,
}

#[cfg(test)]
pub(super) struct TestObserverOperation {
    pending: Option<RequestId>,
    next: u32,
}

impl InstalledOperation {
    pub(super) fn inactive() -> Self {
        Self::Inactive
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        match self {
            Self::Tick(operation) => operation.values.capacity() + operation.waits.capacity(),
            Self::TextPresentation(_) => 0,
            #[cfg(test)]
            Self::TestTextSource(operation) => operation.values.capacity(),
            #[cfg(test)]
            Self::TestObserver(_) => 0,
            Self::Inactive => 0,
        }
    }

    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
}

impl Operation for InstalledOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Tick(operation) => operation
                .request_wait()
                .unwrap_or(OperationAction::Complete),
            Self::TextPresentation(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestTextSource(operation) => operation.emit_or_complete(),
            #[cfg(test)]
            Self::TestObserver(_) => OperationAction::Await,
            Self::Inactive => OperationAction::Complete,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Tick(operation),
                OperationInput::HostOperationCompleted { request, outcome },
            ) if operation.pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                operation.pending = None;
                operation.values.get(operation.next).copied().map_or_else(
                    || Self::fail(1),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            (
                Self::TextPresentation(operation),
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if operation.pending.is_none() && operation.next < operation.maximum_values => {
                let request = RequestId(operation.next);
                operation.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_TEXT_BYTES) {
                        Ok(input) => input,
                        Err(_) => return Self::fail(5),
                    },
                }
            }
            (
                Self::TextPresentation(operation),
                OperationInput::HostOperationCompleted { request, outcome },
            ) if operation.pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                operation.pending = None;
                operation.next = operation.next.saturating_add(1);
                OperationAction::Await
            }
            (Self::TextPresentation(operation), OperationInput::Closed { port: PortId(0) })
                if operation.pending.is_none() =>
            {
                OperationAction::Complete
            }
            #[cfg(test)]
            (
                Self::TestObserver(operation),
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if operation.pending.is_none() => {
                let request = RequestId(0x8000_0000 | operation.next);
                operation.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, TICK_ENCODED_LEN)
                        .expect("typed tick is exactly eight bytes"),
                }
            }
            #[cfg(test)]
            (
                Self::TestObserver(operation),
                OperationInput::HostOperationCompleted { request, outcome },
            ) if operation.pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                operation.pending = None;
                operation.next = operation.next.saturating_add(1);
                OperationAction::Await
            }
            #[cfg(test)]
            (Self::TestObserver(operation), OperationInput::Closed { port: PortId(0) })
                if operation.pending.is_none() =>
            {
                OperationAction::Complete
            }
            (Self::Tick(_), _) => Self::fail(2),
            (Self::TextPresentation(_), _) => Self::fail(5),
            #[cfg(test)]
            (Self::TestTextSource(_), _) => Self::fail(6),
            #[cfg(test)]
            (Self::TestObserver(_), _) => Self::fail(3),
            (Self::Inactive, _) => Self::fail(4),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Tick(operation) => {
                operation.next += 1;
                operation
                    .request_wait()
                    .unwrap_or(OperationAction::Complete)
            }
            Self::TextPresentation(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestTextSource(operation) => {
                operation.next += 1;
                operation.emit_or_complete()
            }
            #[cfg(test)]
            Self::TestObserver(_) => OperationAction::Await,
            Self::Inactive => OperationAction::Complete,
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Tick(operation) => operation.pending = None,
            Self::TextPresentation(operation) => operation.pending = None,
            #[cfg(test)]
            Self::TestTextSource(_) => {}
            #[cfg(test)]
            Self::TestObserver(operation) => operation.pending = None,
            Self::Inactive => {}
        }
    }
}

impl TickOperation {
    fn request_wait(&mut self) -> Option<OperationAction> {
        let wait = self.waits.get(self.next).copied()?;
        let request = RequestId(u32::try_from(self.next).ok()?);
        self.pending = Some(request);
        Some(OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(wait, TICK_ENCODED_LEN)
                .expect("wait duration is exactly eight bytes"),
        })
    }
}

fn tick_budget(placement: &PlannedOperation) -> Result<OperationBudget, String> {
    validate_tick_placement(placement)?;
    let configuration =
        parse_tick_configuration(&placement.configuration).map_err(|error| error.to_string())?;
    let value_items = configuration
        .count
        .checked_mul(2)
        .and_then(|count| u16::try_from(count.max(1)).ok())
        .ok_or_else(|| "tick value item budget overflow".to_string())?;
    let value_bytes = configuration
        .count
        .checked_mul(u64::from(TICK_ENCODED_LEN) * 2)
        .and_then(|bytes| u32::try_from(bytes.max(1)).ok())
        .ok_or_else(|| "tick value byte budget overflow".to_string())?;
    let evidence_items = configuration
        .count
        .checked_mul(15)
        .and_then(|items| items.checked_add(64))
        .and_then(|items| u16::try_from(items).ok())
        .ok_or_else(|| "tick evidence item budget overflow".to_string())?;
    Ok(OperationBudget {
        value_items,
        value_bytes,
        host_requests: usize::try_from(configuration.count)
            .map_err(|_| "tick request budget overflow".to_string())?,
        evidence_items,
        maximum_value_bytes: TICK_ENCODED_LEN,
    })
}

fn text_presentation_budget(placement: &PlannedOperation) -> Result<OperationBudget, String> {
    validate_text_presentation_placement(placement)?;
    let maximum_values = text_maximum_values(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: usize::try_from(maximum_values)
            .map_err(|_| "text presentation request budget overflow".to_string())?,
        evidence_items: 64,
        maximum_value_bytes: MAX_TEXT_BYTES,
    })
}

fn prepare_text_presentation(
    placement: &PlannedOperation,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_text_presentation_placement(placement)?;
    let maximum_values = u32::try_from(text_maximum_values(placement)?)
        .map_err(|_| "text presentation maximum does not fit its implementation".to_string())?;
    Ok(InstalledOperation::TextPresentation(
        TextPresentationOperation {
            pending: None,
            next: 0,
            maximum_values,
        },
    ))
}

fn text_maximum_values(placement: &PlannedOperation) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find(|entry| entry.key == "maximum-values")
        .and_then(|entry| match entry.value {
            conduit_core::ConfigurationValue::U64(value)
                if (1..=MAX_TEXT_VALUES).contains(&value) =>
            {
                Some(value)
            }
            _ => None,
        })
        .ok_or_else(|| "text presentation maximum-values is invalid".to_string())
}

fn validate_text_presentation_placement(placement: &PlannedOperation) -> Result<(), String> {
    if placement.kind_id.as_str() != TEXT_PRESENTATION_KIND
        || placement.kind_contract_revision.as_str() != TEXT_PRESENTATION_CONTRACT_REVISION
        || placement.execution_profile_id.as_str() != TEXT_PRESENTATION_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != TEXT_PRESENTATION_IMPLEMENTATION
        || placement.artifact_id.as_str() != TEXT_PRESENTATION_ARTIFACT
        || placement.inputs.len() != 1
        || !placement.outputs.is_empty()
        || placement.inputs[0].port_id.as_str() != "text"
        || placement.inputs[0].value_kind.as_str() != TEXT_PRESENTATION_VALUE_KIND
        || placement.inputs[0].direction != PortDirection::Input
    {
        return Err(
            "planned text presentation executable identity does not match its installation"
                .to_string(),
        );
    }
    Ok(())
}

fn prepare_tick(
    placement: &PlannedOperation,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_tick_placement(placement)?;
    let configuration =
        parse_tick_configuration(&placement.configuration).map_err(|error| error.to_string())?;
    let count = usize::try_from(configuration.count)
        .map_err(|_| "tick count does not fit hosted preparation".to_string())?;
    let mut ticks = Vec::with_capacity(count);
    let mut waits = Vec::with_capacity(count);
    for sequence in 0..configuration.count {
        ticks.push(
            values
                .store(&encode_tick(sequence))
                .map_err(|error| format!("store typed tick: {error:?}"))?,
        );
        waits.push(
            values
                .store(&configuration.period_ms.to_le_bytes())
                .map_err(|error| format!("store tick wait: {error:?}"))?,
        );
    }
    Ok(InstalledOperation::Tick(TickOperation {
        values: ticks,
        waits,
        next: 0,
        pending: None,
    }))
}

fn validate_tick_placement(placement: &PlannedOperation) -> Result<(), String> {
    if placement.kind_id.as_str() != TICK_KIND
        || placement.kind_contract_revision.as_str() != TICK_CONTRACT_REVISION
        || placement.execution_profile_id.as_str() != TICK_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != TICK_IMPLEMENTATION
        || placement.artifact_id.as_str() != TICK_ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs.len() != 1
        || placement.outputs[0].port_id.as_str() != "tick"
        || placement.outputs[0].value_kind.as_str() != TICK_VALUE_KIND
        || placement.outputs[0].direction != PortDirection::Output
    {
        return Err("planned tick executable identity does not match its installation".to_string());
    }
    Ok(())
}

#[cfg(test)]
fn prepare_test_observer(
    placement: &PlannedOperation,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    if placement.kind_id.as_str() != TEST_OBSERVER_KIND
        || placement.implementation_id.as_str() != TEST_OBSERVER_IMPLEMENTATION
        || placement.inputs.len() != 1
        || !placement.outputs.is_empty()
        || placement.inputs[0].port_id.as_str() != "in"
        || placement.inputs[0].value_kind.as_str() != TICK_VALUE_KIND
        || placement.inputs[0].direction != PortDirection::Input
    {
        return Err("test observer executable identity does not match its fixture".to_string());
    }
    Ok(InstalledOperation::TestObserver(TestObserverOperation {
        pending: None,
        next: 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::HostOperationOutcome;

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: TICK_ENCODED_LEN,
        }
    }

    #[test]
    fn cancelled_tick_rejects_a_late_timer_completion() {
        let mut operation = InstalledOperation::Tick(TickOperation {
            values: vec![value(0)],
            waits: vec![value(1)],
            next: 0,
            pending: None,
        });
        assert!(matches!(
            operation.start(),
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                operation: HostOperationId(0),
                ..
            }
        ));
        operation.cancel();
        assert!(matches!(
            operation.resume(OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome: HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 2,
            })
        ));
    }
}
