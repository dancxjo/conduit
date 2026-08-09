use super::contract::{
    encode_tick, parse_every_configuration, parse_tick_configuration, TICK_ARTIFACT,
    TICK_CONTRACT_REVISION, TICK_ENCODED_LEN, TICK_EXECUTION_PROFILE, TICK_IMPLEMENTATION,
    TICK_KIND, TICK_VALUE_KIND, TIME_EVERY_ARTIFACT, TIME_EVERY_CONTRACT_REVISION,
    TIME_EVERY_EXECUTION_PROFILE, TIME_EVERY_IMPLEMENTATION, TIME_EVERY_KIND,
};
use super::count_operations::{CountPresentationOperation, StateCountOperation};
use super::generate_text::GenerateTextOperation;
use super::text_operations::{
    TextLiteralOperation, TextPresentationOperation, TextTransformOperation,
};
use super::tick_presentation::TickPresentationOperation;
use conduit_core::{PlannedGear, PortDirection};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) struct OperationBudget {
    pub(super) value_items: u16,
    pub(super) value_bytes: u32,
    pub(super) host_requests: usize,
    pub(super) sign_items: u16,
    pub(super) maximum_value_bytes: u32,
}

pub(super) struct InstalledFactory {
    pub(super) implementation_id: &'static str,
    pub(super) budget: fn(&PlannedGear) -> Result<OperationBudget, String>,
    pub(super) prepare: fn(
        &PlannedGear,
        &mut conduit_kernel::HostedValueStore,
    ) -> Result<InstalledOperation, String>,
}

pub(super) static TICK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TICK_IMPLEMENTATION,
    budget: tick_budget,
    prepare: prepare_tick,
};

pub(super) static EVERY_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: TIME_EVERY_IMPLEMENTATION,
    budget: every_budget,
    prepare: prepare_every,
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
            sign_items: 16,
            maximum_value_bytes: TICK_ENCODED_LEN,
        })
    },
    prepare: prepare_test_observer,
};

pub(super) enum InstalledOperation {
    Tick(TickOperation),
    TickPresentation(TickPresentationOperation),
    TextLiteral(TextLiteralOperation),
    TextUpper(TextTransformOperation),
    TextJoin(TextTransformOperation),
    TextPresentation(TextPresentationOperation),
    StateCount(StateCountOperation),
    CountPresentation(CountPresentationOperation),
    ExternalWebSocketListener(super::external_websocket::ExternalWebSocketListenerOperation),
    GenerateText(GenerateTextOperation),
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
            Self::TickPresentation(_) => 0,
            Self::TextLiteral(_) | Self::TextUpper(_) | Self::TextJoin(_) => 0,
            Self::TextPresentation(_) => 0,
            Self::StateCount(operation) => operation.allocation_capacity(),
            Self::CountPresentation(_) => 0,
            Self::ExternalWebSocketListener(_) => 0,
            Self::GenerateText(_) => 0,
            #[cfg(test)]
            Self::TestTextSource(operation) => operation.values.capacity(),
            #[cfg(test)]
            Self::TestObserver(_) => 0,
            Self::Inactive => 0,
        }
    }

    pub(super) fn fail(detail: u16) -> OperationAction {
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
            Self::TickPresentation(operation) => operation.start(),
            Self::TextLiteral(operation) => operation.start(),
            Self::TextUpper(operation) => operation.start(),
            Self::TextJoin(operation) => operation.start(),
            Self::TextPresentation(operation) => operation.start(),
            Self::StateCount(operation) => operation.start(),
            Self::CountPresentation(operation) => operation.start(),
            Self::ExternalWebSocketListener(operation) => operation.start(),
            Self::GenerateText(operation) => operation.start(),
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
            (Self::TextLiteral(operation), input) => operation.resume(input),
            (Self::TextUpper(operation), input) => operation.resume(input),
            (Self::TextJoin(operation), input) => operation.resume(input),
            (Self::TextPresentation(operation), input) => operation.resume(input),
            (Self::TickPresentation(operation), input) => operation.resume(input),
            (Self::StateCount(operation), input) => operation.resume(input),
            (Self::CountPresentation(operation), input) => operation.resume(input),
            (Self::ExternalWebSocketListener(operation), input) => operation.resume(input),
            (Self::GenerateText(operation), input) => operation.resume(input),
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
            Self::TickPresentation(_) => OperationAction::Await,
            Self::TextLiteral(operation) => operation.advance(),
            Self::TextUpper(_) => OperationAction::Await,
            Self::TextJoin(_) => OperationAction::Await,
            Self::TextPresentation(_) => OperationAction::Await,
            Self::StateCount(operation) => operation.advance(),
            Self::CountPresentation(_) => OperationAction::Await,
            Self::ExternalWebSocketListener(operation) => operation.advance(),
            Self::GenerateText(operation) => operation.advance(),
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
            Self::TickPresentation(operation) => operation.cancel(),
            Self::TextLiteral(_) => {}
            Self::TextUpper(operation) => operation.cancel(),
            Self::TextJoin(operation) => operation.cancel(),
            Self::TextPresentation(operation) => operation.cancel(),
            Self::StateCount(_) => {}
            Self::CountPresentation(operation) => operation.cancel(),
            Self::ExternalWebSocketListener(operation) => operation.cancel(),
            Self::GenerateText(operation) => operation.cancel(),
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

fn tick_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_tick_placement(placement)?;
    let configuration =
        parse_tick_configuration(&placement.configuration).map_err(|error| error.to_string())?;
    tick_budget_for(&configuration)
}

fn every_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_every_placement(placement)?;
    tick_budget_for(&parse_every_configuration(&placement.configuration)?)
}

fn tick_budget_for(
    configuration: &super::contract::TickConfiguration,
) -> Result<OperationBudget, String> {
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
    let sign_items = configuration
        .count
        .checked_mul(15)
        .and_then(|items| items.checked_add(64))
        .and_then(|items| u16::try_from(items).ok())
        .ok_or_else(|| "tick sign item budget overflow".to_string())?;
    Ok(OperationBudget {
        value_items,
        value_bytes,
        host_requests: usize::try_from(configuration.count)
            .map_err(|_| "tick request budget overflow".to_string())?,
        sign_items,
        maximum_value_bytes: TICK_ENCODED_LEN,
    })
}

fn prepare_tick(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_tick_placement(placement)?;
    let configuration =
        parse_tick_configuration(&placement.configuration).map_err(|error| error.to_string())?;
    prepare_tick_values(configuration, values)
}

fn prepare_every(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_every_placement(placement)?;
    prepare_tick_values(parse_every_configuration(&placement.configuration)?, values)
}

fn prepare_tick_values(
    configuration: super::contract::TickConfiguration,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
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

fn validate_tick_placement(placement: &PlannedGear) -> Result<(), String> {
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

fn validate_every_placement(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != TIME_EVERY_KIND
        || placement.kind_contract_revision.as_str() != TIME_EVERY_CONTRACT_REVISION
        || placement.execution_profile_id.as_str() != TIME_EVERY_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != TIME_EVERY_IMPLEMENTATION
        || placement.artifact_id.as_str() != TIME_EVERY_ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs.len() != 1
        || placement.outputs[0].port_id.as_str() != "tick"
        || placement.outputs[0].value_kind.as_str() != TICK_VALUE_KIND
        || placement.outputs[0].direction != PortDirection::Output
    {
        return Err("planned time/every identity does not match its installation".to_string());
    }
    Ok(())
}

#[cfg(test)]
fn prepare_test_observer(
    placement: &PlannedGear,
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
