//! Exact current Boolean selector over two retained current Scalars.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    InfoBool, PlannedGear, PortDirection, Scalar, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    CanonicalValue, Failure, FailureCode, PortId,
};

pub(super) static STATE_SELECT_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::STATE_SELECT_SCALAR_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct StateSelectScalarOperation {
    selector: Option<bool>,
    candidates: [Option<[u8; SCALAR_ENCODED_LEN]>; 2],
    closed: [bool; 3],
}

impl<const PORTS: usize> StepOperation<PORTS> for StateSelectScalarOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        for index in 0..3 {
            let port = PortId(index);
            let Some(value) = io.input(port) else {
                continue;
            };
            let Some(canonical) = input_bytes.input(port) else {
                return StepOutcome::Fail(select_failure());
            };
            let emits = match port {
                PortId(0) if value.byte_len == BOOL_ENCODED_LEN as u32 && !self.closed[0] => {
                    if InfoBool::decode(canonical).is_err() {
                        return StepOutcome::Fail(select_failure());
                    }
                    self.candidates.iter().all(Option::is_some)
                }
                PortId(1) | PortId(2)
                    if value.byte_len == SCALAR_ENCODED_LEN as u32
                        && !self.closed[usize::from(port.0)] =>
                {
                    if Scalar::decode(canonical).is_err() {
                        return StepOutcome::Fail(select_failure());
                    }
                    let candidate = usize::from(port.0 - 1);
                    self.selector.is_some() && self.candidates[1 - candidate].is_some()
                }
                _ => return StepOutcome::Fail(select_failure()),
            };
            if emits && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(port).expect("present State selector input");
            match port {
                PortId(0) => {
                    self.selector = Some(
                        InfoBool::decode(canonical)
                            .expect("validated Boolean selector")
                            .get(),
                    );
                }
                PortId(1) | PortId(2) => {
                    self.candidates[usize::from(port.0 - 1)] = Some(
                        canonical
                            .try_into()
                            .expect("validated Scalar has exact canonical length"),
                    );
                }
                _ => unreachable!("validated State selector Port"),
            }
            if emits {
                let selector = self.selector.expect("emitting selector is present");
                let selected =
                    self.candidates[usize::from(selector)].expect("emitting candidate is present");
                io.send_canonical(
                    PortId(0),
                    CanonicalValue::new(&selected).expect("Scalar fits derived-value bound"),
                )
                .expect("ready State selector output");
            }
            return StepOutcome::Progress;
        }
        for index in 0..3 {
            let port = PortId(index);
            if io.input_closed(port) && !self.closed[usize::from(index)] {
                io.consume_closed(port)
                    .expect("observed State selector closure");
                self.closed[usize::from(index)] = true;
                return if self.closed.into_iter().all(|closed| closed) {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.selector = None;
        self.candidates = [None; 2];
    }
}

fn select_failure() -> Failure {
    Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 14,
    }
}

impl StateSelectScalarOperation {}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 128,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::StateSelectScalar(
        StateSelectScalarOperation {
            selector: None,
            candidates: [None; 2],
            closed: [false; 3],
        },
    ))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::state_select_scalar_offer();
    if placement.kind_id.as_str() != conduit_semantic_catalog::STATE_SELECT_KIND
        || placement.kind_contract_revision
            != conduit_semantic_catalog::STATE_SELECT_SCALAR_CONTRACT_REVISION.into()
        || placement.execution_profile_id
            != conduit_std_offers::STATE_SELECT_SCALAR_EXECUTION_PROFILE.into()
        || placement.implementation_id
            != conduit_std_offers::STATE_SELECT_SCALAR_IMPLEMENTATION.into()
        || placement.artifact_id != conduit_std_offers::STATE_SELECT_SCALAR_ARTIFACT.into()
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || !placement.configuration.is_empty()
        || placement
            .inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
        || placement
            .outputs
            .iter()
            .any(|port| port.direction != PortDirection::Output)
    {
        return Err("planned state/select scalar identity does not match its installation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bool_value(slot: u16, value: bool) -> (ValueRef, [u8; BOOL_ENCODED_LEN]) {
        (
            ValueRef {
                slot,
                generation: 1,
                byte_len: BOOL_ENCODED_LEN as u32,
            },
            InfoBool::new(value).encode(),
        )
    }

    fn scalar_value(slot: u16, value: i64) -> (ValueRef, [u8; SCALAR_ENCODED_LEN]) {
        (
            ValueRef {
                slot,
                generation: 1,
                byte_len: SCALAR_ENCODED_LEN as u32,
            },
            Scalar::from_raw_microunits(value).encode(),
        )
    }

    #[test]
    fn emits_canonical_current_values_without_retaining_input_references() {
        let mut operation = StateSelectScalarOperation {
            selector: None,
            candidates: [None; 2],
            closed: [false; 3],
        };
        let (selector_false, false_bytes) = bool_value(1, false);
        let (requested, requested_bytes) = scalar_value(2, 100_000);
        let (stopped, stopped_bytes) = scalar_value(3, 0);
        assert_eq!(
            operation.resume_value(PortId(0), selector_false, &false_bytes),
            OperationAction::Await
        );
        assert_eq!(
            operation.resume_value(PortId(1), requested, &requested_bytes),
            OperationAction::Await
        );
        assert!(matches!(
            operation.resume_value(PortId(2), stopped, &stopped_bytes),
            OperationAction::EmitCanonical { value, .. }
                if value.as_slice() == requested_bytes
        ));
        let (selector_true, true_bytes) = bool_value(4, true);
        assert!(matches!(
            operation.resume_value(PortId(0), selector_true, &true_bytes),
            OperationAction::EmitCanonical { value, .. }
                if value.as_slice() == stopped_bytes
        ));

        let (replacement, replacement_bytes) = scalar_value(5, 1);
        assert!(matches!(
            operation.resume_value(PortId(2), replacement, &replacement_bytes),
            OperationAction::EmitCanonical { value, .. }
                if value.as_slice() == replacement_bytes
        ));

        for port in [PortId(0), PortId(1)] {
            assert_eq!(
                operation.resume(OperationInput::Closed { port }),
                OperationAction::Await
            );
        }
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(2) }),
            OperationAction::Complete
        );
    }
}
