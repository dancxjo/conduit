use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ConfigurationValue, InfoBool, PlannedGear, Scalar, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    HostedValueStore, OperationAction, OperationInput, PortId, ValueRef, ValueStorage,
};

pub(super) static LOGIC_COMPARE_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
    budget: compare_budget,
    prepare: prepare_compare,
};

pub(super) static LOGIC_NOT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::LOGIC_NOT_IMPLEMENTATION,
    budget: not_budget,
    prepare: prepare_not,
};

pub(super) static LOGIC_SELECT_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::LOGIC_SELECT_SCALAR_IMPLEMENTATION,
    budget: select_budget,
    prepare: prepare_select,
};

use conduit_semantic_catalog::ScalarComparison as CompareOperator;

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

impl<const PORTS: usize> StepOperation<PORTS> for LogicCompareScalarOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let [Some(left), Some(right)] = self.operands {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let selected = usize::from(self.operator.evaluate(left, right));
            let unused = usize::from(selected == 0);
            let Some(output) = self.decisions.values[selected].take() else {
                return StepOutcome::Fail(logic_failure(20));
            };
            let Some(discard) = self.decisions.values[unused].take() else {
                return StepOutcome::Fail(logic_failure(20));
            };
            io.send(PortId(0), output)
                .expect("ready scalar-comparison output");
            io.discard(discard)
                .expect("unused scalar-comparison decision");
            return StepOutcome::Complete;
        }
        for port in [PortId(0), PortId(1)] {
            let index = usize::from(port.0);
            if let Some(value) = io.input(port) {
                if self.operands[index].is_some() || value.byte_len != SCALAR_ENCODED_LEN as u32 {
                    return StepOutcome::Fail(logic_failure(20));
                }
                let Some(canonical) = input_bytes.input(port) else {
                    return StepOutcome::Fail(logic_failure(20));
                };
                let Ok(scalar) = Scalar::decode(canonical) else {
                    return StepOutcome::Fail(logic_failure(20));
                };
                io.consume(port).expect("present scalar-comparison input");
                self.operands[index] = Some(scalar);
                return StepOutcome::Progress;
            }
            if io.input_closed(port) && self.operands[index].is_none() {
                let [first, second] = &mut self.decisions.values;
                let Some(first) = first.take() else {
                    return StepOutcome::Fail(logic_failure(20));
                };
                let Some(second) = second.take() else {
                    return StepOutcome::Fail(logic_failure(20));
                };
                io.consume_closed(port)
                    .expect("observed scalar-comparison closure");
                io.discard(first).expect("unused comparison false value");
                io.discard(second).expect("unused comparison true value");
                return StepOutcome::Complete;
            }
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.operands = [None; 2];
        self.decisions.cancel();
    }
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

impl<const PORTS: usize> StepOperation<PORTS> for LogicNotOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.received {
            return StepOutcome::Complete;
        }
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len != BOOL_ENCODED_LEN as u32 {
                return StepOutcome::Fail(logic_failure(21));
            }
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return StepOutcome::Fail(logic_failure(21));
            };
            let Ok(input) = InfoBool::decode(canonical) else {
                return StepOutcome::Fail(logic_failure(21));
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let selected = usize::from(!input.get());
            let unused = usize::from(input.get());
            let Some(output) = self.decisions.values[selected].take() else {
                return StepOutcome::Fail(logic_failure(21));
            };
            let Some(discard) = self.decisions.values[unused].take() else {
                return StepOutcome::Fail(logic_failure(21));
            };
            io.consume(PortId(0)).expect("present logic/not input");
            io.send(PortId(0), output).expect("ready logic/not output");
            io.discard(discard).expect("unused logic/not decision");
            self.received = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            let [first, second] = &mut self.decisions.values;
            let Some(first) = first.take() else {
                return StepOutcome::Fail(logic_failure(21));
            };
            let Some(second) = second.take() else {
                return StepOutcome::Fail(logic_failure(21));
            };
            io.consume_closed(PortId(0))
                .expect("observed logic/not input closure");
            io.discard(first).expect("first unused logic/not decision");
            io.discard(second)
                .expect("second unused logic/not decision");
            self.received = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.received = false;
        self.decisions.cancel();
    }
}

const fn logic_failure(detail: u16) -> conduit_kernel::Failure {
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    }
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

impl<const PORTS: usize> StepOperation<PORTS> for LogicSelectScalarOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.candidate_seen.into_iter().all(|seen| seen) {
            if let Some(selector) = self.selector {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                let selected = usize::from(selector);
                let unselected = usize::from(!selector);
                let Some(output) = self.candidates[selected].take() else {
                    return self.discard_candidates(io);
                };
                io.send(PortId(0), output)
                    .expect("ready scalar-selection output");
                if let Some(discard) = self.candidates[unselected].take() {
                    io.discard(discard)
                        .expect("unused scalar-selection candidate");
                }
                return StepOutcome::Complete;
            }
            if self.selector_closed {
                return self.discard_candidates(io);
            }
        }
        for port in [PortId(0), PortId(1), PortId(2)] {
            if let Some(value) = io.input(port) {
                let Some(canonical) = input_bytes.input(port) else {
                    return StepOutcome::Fail(logic_failure(22));
                };
                match port {
                    PortId(0)
                        if self.selector.is_none() && value.byte_len == BOOL_ENCODED_LEN as u32 =>
                    {
                        let Ok(selector) = InfoBool::decode(canonical) else {
                            return StepOutcome::Fail(logic_failure(22));
                        };
                        io.consume(port).expect("present scalar selector");
                        self.selector = Some(selector.get());
                    }
                    PortId(1) | PortId(2) if value.byte_len == SCALAR_ENCODED_LEN as u32 => {
                        let index = usize::from(port.0 - 1);
                        if self.candidate_seen[index] || Scalar::decode(canonical).is_err() {
                            return StepOutcome::Fail(logic_failure(22));
                        }
                        let retained = io.take_input(port).expect("present scalar candidate");
                        self.candidate_seen[index] = true;
                        self.candidates[index] = Some(retained);
                    }
                    _ => return StepOutcome::Fail(logic_failure(22)),
                }
                return StepOutcome::Progress;
            }
            if io.input_closed(port) {
                match port {
                    PortId(0) if self.selector.is_none() && !self.selector_closed => {
                        io.consume_closed(port).expect("observed selector closure");
                        self.selector_closed = true;
                    }
                    PortId(1) | PortId(2) => {
                        let index = usize::from(port.0 - 1);
                        if self.candidate_seen[index] {
                            continue;
                        }
                        io.consume_closed(port).expect("observed candidate closure");
                        self.candidate_seen[index] = true;
                    }
                    _ => continue,
                }
                return StepOutcome::Progress;
            }
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        LogicSelectScalarOperation::cancel(self);
    }
}

impl LogicSelectScalarOperation {
    fn discard_candidates<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        for candidate in &mut self.candidates {
            if let Some(value) = candidate.take() {
                io.discard(value)
                    .expect("at most two unused scalar candidates");
            }
        }
        StepOutcome::Complete
    }
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
        &conduit_std_offers::logic_compare_scalar_offer(),
        true,
    )?;
    comparison_operator(placement)?;
    Ok(decision_budget())
}

fn not_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &conduit_std_offers::logic_not_offer(), false)?;
    Ok(decision_budget())
}

fn select_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(
        placement,
        &conduit_std_offers::logic_select_scalar_offer(),
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
        &conduit_std_offers::logic_compare_scalar_offer(),
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
    validate(placement, &conduit_std_offers::logic_not_offer(), false)?;
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
        &conduit_std_offers::logic_select_scalar_offer(),
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
        [entry] if entry.key == conduit_semantic_catalog::COMPARE_OPERATOR_KEY => {
            match &entry.value {
                ConfigurationValue::Text(value) => CompareOperator::parse(value),
                _ => None,
            }
        }
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
        || placement.host_calls != offer.host_calls
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
