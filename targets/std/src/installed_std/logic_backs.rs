use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    ConfigurationValue, InfoBool, PlannedGear, Scalar, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostedValueStore, PortId, ValueRef, ValueStorage,
};

pub(super) static LOGIC_COMPARE_SCALAR_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::LOGIC_COMPARE_SCALAR_IMPLEMENTATION,
    budget: compare_budget,
    prepare: prepare_compare,
};

pub(super) static LOGIC_NOT_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::LOGIC_NOT_IMPLEMENTATION,
    budget: not_budget,
    prepare: prepare_not,
};

pub(super) static LOGIC_SELECT_SCALAR_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::LOGIC_SELECT_SCALAR_IMPLEMENTATION,
    budget: select_budget,
    prepare: prepare_select,
};

use conduit_semantic_catalog::ScalarComparison as CompareOperator;

struct DecisionValues {
    values: [Option<ValueRef>; 2],
}

impl DecisionValues {
    fn prepare(store: &mut HostedValueStore) -> Result<Self, String> {
        Ok(Self {
            values: [
                Some(store_bool(store, InfoBool::FALSE)?),
                Some(store_bool(store, InfoBool::TRUE)?),
            ],
        })
    }

    fn cancel(&mut self) {
        self.values = [None; 2];
    }
}

pub(super) struct LogicCompareScalarBack {
    operator: CompareOperator,
    operands: [Option<Scalar>; 2],
    decisions: DecisionValues,
}

impl<const PORTS: usize> StepBack<PORTS> for LogicCompareScalarBack {
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

impl LogicCompareScalarBack {}

pub(super) struct LogicNotBack {
    received: bool,
    decisions: DecisionValues,
}

impl<const PORTS: usize> StepBack<PORTS> for LogicNotBack {
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

impl LogicNotBack {}

pub(super) struct LogicSelectScalarBack {
    selector: Option<bool>,
    selector_closed: bool,
    candidates: [Option<ValueRef>; 2],
    candidate_seen: [bool; 2],
}

impl<const PORTS: usize> StepBack<PORTS> for LogicSelectScalarBack {
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
        self.selector = None;
        self.selector_closed = false;
        self.candidates = [None; 2];
        self.candidate_seen = [false; 2];
    }
}

impl LogicSelectScalarBack {
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

fn store_bool(store: &mut HostedValueStore, value: InfoBool) -> Result<ValueRef, String> {
    store
        .store(&value.encode())
        .map_err(|error| format!("store logic decision: {error:?}"))
}

fn compare_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(
        placement,
        &conduit_std_offers::logic_compare_scalar_offer(),
        true,
    )?;
    comparison_operator(placement)?;
    Ok(decision_budget())
}

fn not_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement, &conduit_std_offers::logic_not_offer(), false)?;
    Ok(decision_budget())
}

fn select_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(
        placement,
        &conduit_std_offers::logic_select_scalar_offer(),
        false,
    )?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn decision_budget() -> BackBudget {
    BackBudget {
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
) -> Result<InstalledBack, String> {
    validate(
        placement,
        &conduit_std_offers::logic_compare_scalar_offer(),
        true,
    )?;
    Ok(InstalledBack::LogicCompareScalar(LogicCompareScalarBack {
        operator: comparison_operator(placement)?,
        operands: [None; 2],
        decisions: DecisionValues::prepare(store)?,
    }))
}

fn prepare_not(
    placement: &PlannedGear,
    store: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement, &conduit_std_offers::logic_not_offer(), false)?;
    Ok(InstalledBack::LogicNot(LogicNotBack {
        received: false,
        decisions: DecisionValues::prepare(store)?,
    }))
}

fn prepare_select(
    placement: &PlannedGear,
    _store: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(
        placement,
        &conduit_std_offers::logic_select_scalar_offer(),
        false,
    )?;
    Ok(InstalledBack::LogicSelectScalar(LogicSelectScalarBack {
        selector: None,
        selector_closed: false,
        candidates: [None; 2],
        candidate_seen: [false; 2],
    }))
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
#[path = "logic_backs_tests.rs"]
mod tests;
