use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ConfigurationValue, InfoBool, PlannedGear, Scalar, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{
    HostedValueStore, OperationAction, OperationInput, PortId, ValueRef, ValueStorage,
};

pub(super) static LOGIC_COMPARE_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
    budget: compare_budget,
    prepare: prepare_compare,
};

pub(super) static LOGIC_NOT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::LOGIC_NOT_IMPLEMENTATION,
    budget: not_budget,
    prepare: prepare_not,
};

pub(super) static LOGIC_SELECT_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_catalog::LOGIC_SELECT_SCALAR_IMPLEMENTATION,
    budget: select_budget,
    prepare: prepare_select,
};

use conduit_std_catalog::ScalarComparison as CompareOperator;

struct DecisionValues {
    values: [Option<ValueRef>; 2],
    released: [Option<ValueRef>; 2],
}

impl DecisionValues {
    fn prepare(store: &mut HostedValueStore) -> Result<Self, String> {
        Ok(Self {
            values: [
                Some(store_bool(store, InfoBool::FALSE)?),
                Some(store_bool(store, InfoBool::TRUE)?),
            ],
            released: [None; 2],
        })
    }

    fn decide(&mut self, decision: bool) -> OperationAction {
        let selected = usize::from(decision);
        let unused = usize::from(!decision);
        let Some(value) = self.values[selected].take() else {
            return InstalledOperation::fail(20);
        };
        self.released[0] = self.values[unused].take();
        OperationAction::Emit {
            port: PortId(0),
            value,
        }
    }

    fn complete_without_decision(&mut self) -> OperationAction {
        self.released = [self.values[0].take(), self.values[1].take()];
        OperationAction::Complete
    }

    fn take_released(&mut self) -> Option<ValueRef> {
        self.released.iter_mut().find_map(Option::take)
    }

    fn cancel(&mut self) {
        self.values = [None; 2];
        self.released = [None; 2];
    }
}

pub(super) struct LogicCompareScalarOperation {
    operator: CompareOperator,
    operands: [Option<Scalar>; 2],
    decisions: DecisionValues,
}

impl LogicCompareScalarOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        let index = usize::from(port.0);
        if index >= self.operands.len()
            || self.operands[index].is_some()
            || value.byte_len != SCALAR_ENCODED_LEN as u32
        {
            return InstalledOperation::fail(20);
        }
        let Ok(scalar) = Scalar::decode(canonical) else {
            return InstalledOperation::fail(20);
        };
        self.operands[index] = Some(scalar);
        match self.operands {
            [Some(left), Some(right)] => self.decisions.decide(self.operator.evaluate(left, right)),
            _ => OperationAction::Await,
        }
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port } => {
                let index = usize::from(port.0);
                if index < self.operands.len() && self.operands[index].is_none() {
                    self.decisions.complete_without_decision()
                } else if index < self.operands.len() {
                    OperationAction::Await
                } else {
                    InstalledOperation::fail(20)
                }
            }
            _ => InstalledOperation::fail(20),
        }
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.decisions.take_released()
    }

    pub(super) fn cancel(&mut self) {
        self.operands = [None; 2];
        self.decisions.cancel();
    }
}

pub(super) struct LogicNotOperation {
    received: bool,
    decisions: DecisionValues,
}

impl LogicNotOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        if port != PortId(0) || self.received || value.byte_len != BOOL_ENCODED_LEN as u32 {
            return InstalledOperation::fail(21);
        }
        let Ok(input) = InfoBool::decode(canonical) else {
            return InstalledOperation::fail(21);
        };
        self.received = true;
        self.decisions.decide(!input.get())
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port: PortId(0) } if !self.received => {
                self.decisions.complete_without_decision()
            }
            _ => InstalledOperation::fail(21),
        }
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.decisions.take_released()
    }

    pub(super) fn cancel(&mut self) {
        self.received = false;
        self.decisions.cancel();
    }
}

pub(super) struct LogicSelectScalarOperation {
    selector: Option<bool>,
    selector_closed: bool,
    candidates: [Option<ValueRef>; 2],
    candidate_seen: [bool; 2],
    released: [Option<ValueRef>; 2],
    retain_resumed: bool,
}

impl LogicSelectScalarOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        self.retain_resumed = false;
        match port {
            PortId(0) if self.selector.is_none() && value.byte_len == BOOL_ENCODED_LEN as u32 => {
                let Ok(selector) = InfoBool::decode(canonical) else {
                    return InstalledOperation::fail(22);
                };
                self.selector = Some(selector.get());
            }
            PortId(1) | PortId(2) if value.byte_len == SCALAR_ENCODED_LEN as u32 => {
                let index = usize::from(port.0 - 1);
                if self.candidate_seen[index] || Scalar::decode(canonical).is_err() {
                    return InstalledOperation::fail(22);
                }
                self.candidate_seen[index] = true;
                self.candidates[index] = Some(value);
                self.retain_resumed = true;
            }
            _ => return InstalledOperation::fail(22),
        }
        self.decide_or_await()
    }

    fn decide_or_await(&mut self) -> OperationAction {
        if !self.candidate_seen.into_iter().all(|seen| seen) {
            return OperationAction::Await;
        }
        let Some(selector) = self.selector else {
            return if self.selector_closed {
                self.complete_without_decision()
            } else {
                OperationAction::Await
            };
        };
        let selected = usize::from(selector);
        let unselected = usize::from(!selector);
        let Some(value) = self.candidates[selected].take() else {
            return self.complete_without_decision();
        };
        self.released[0] = self.candidates[unselected].take();
        self.retain_resumed = false;
        OperationAction::Emit {
            port: PortId(0),
            value,
        }
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Closed { port: PortId(0) } if self.selector.is_none() => {
                self.selector_closed = true;
                self.decide_or_await()
            }
            OperationInput::Closed { port } if matches!(port, PortId(1) | PortId(2)) => {
                let index = usize::from(port.0 - 1);
                if self.candidate_seen[index] {
                    return InstalledOperation::fail(22);
                }
                self.candidate_seen[index] = true;
                self.decide_or_await()
            }
            _ => InstalledOperation::fail(22),
        }
    }

    fn complete_without_decision(&mut self) -> OperationAction {
        self.released = [self.candidates[0].take(), self.candidates[1].take()];
        OperationAction::Complete
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.iter_mut().find_map(Option::take)
    }

    pub(super) fn cancel(&mut self) {
        self.selector = None;
        self.selector_closed = false;
        self.candidates = [None; 2];
        self.candidate_seen = [false; 2];
        self.released = [None; 2];
        self.retain_resumed = false;
    }
}

fn store_bool(store: &mut HostedValueStore, value: InfoBool) -> Result<ValueRef, String> {
    store
        .store(&value.encode())
        .map_err(|error| format!("store logic decision: {error:?}"))
}

fn compare_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(
        placement,
        &conduit_std_catalog::logic_compare_scalar_offer(),
        true,
    )?;
    comparison_operator(placement)?;
    Ok(decision_budget())
}

fn not_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_catalog::logic_not_offer(), false)?;
    Ok(decision_budget())
}

fn select_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(
        placement,
        &conduit_std_catalog::logic_select_scalar_offer(),
        false,
    )?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn decision_budget() -> OperationBudget {
    OperationBudget {
        value_items: 2,
        value_bytes: (BOOL_ENCODED_LEN * 2) as u32,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    }
}

fn prepare_compare(
    placement: &PlannedGear,
    store: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(
        placement,
        &conduit_std_catalog::logic_compare_scalar_offer(),
        true,
    )?;
    Ok(InstalledOperation::LogicCompareScalar(
        LogicCompareScalarOperation {
            operator: comparison_operator(placement)?,
            operands: [None; 2],
            decisions: DecisionValues::prepare(store)?,
        },
    ))
}

fn prepare_not(
    placement: &PlannedGear,
    store: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, &conduit_std_catalog::logic_not_offer(), false)?;
    Ok(InstalledOperation::LogicNot(LogicNotOperation {
        received: false,
        decisions: DecisionValues::prepare(store)?,
    }))
}

fn prepare_select(
    placement: &PlannedGear,
    _store: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(
        placement,
        &conduit_std_catalog::logic_select_scalar_offer(),
        false,
    )?;
    Ok(InstalledOperation::LogicSelectScalar(
        LogicSelectScalarOperation {
            selector: None,
            selector_closed: false,
            candidates: [None; 2],
            candidate_seen: [false; 2],
            released: [None; 2],
            retain_resumed: false,
        },
    ))
}

fn comparison_operator(placement: &PlannedGear) -> Result<CompareOperator, String> {
    match placement.configuration.as_slice() {
        [entry] if entry.key == conduit_std_catalog::COMPARE_OPERATOR_KEY => match &entry.value {
            ConfigurationValue::Text(value) => CompareOperator::parse(value),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| "planned logic/compare operator is missing or unsupported".to_string())
}

fn validate(
    placement: &PlannedGear,
    offer: &conduit_core::CapabilityOffer,
    configured: bool,
) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
        || placement.configuration.len() != usize::from(configured)
    {
        return Err("planned logic executable identity does not match its installation".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "logic_operations_tests.rs"]
mod tests;
