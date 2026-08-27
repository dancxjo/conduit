//! Exact current Boolean selector over two retained current Scalars.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    InfoBool, PlannedGear, PortDirection, Scalar, BOOL_ENCODED_LEN, SCALAR_ENCODED_LEN,
};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef};

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

impl StateSelectScalarOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        match port {
            PortId(0) if value.byte_len == BOOL_ENCODED_LEN as u32 && !self.closed[0] => {
                let Ok(selector) = InfoBool::decode(canonical) else {
                    return InstalledOperation::fail(14);
                };
                self.selector = Some(selector.get());
            }
            PortId(1) | PortId(2)
                if value.byte_len == SCALAR_ENCODED_LEN as u32
                    && !self.closed[usize::from(port.0)] =>
            {
                if Scalar::decode(canonical).is_err() {
                    return InstalledOperation::fail(14);
                }
                let index = usize::from(port.0 - 1);
                self.candidates[index] = Some(
                    canonical
                        .try_into()
                        .expect("decoded Scalar has exact canonical length"),
                );
            }
            _ => return InstalledOperation::fail(14),
        }
        self.emit_or_await()
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port } if usize::from(port.0) < self.closed.len() => {
                self.closed[usize::from(port.0)] = true;
                if self.closed.into_iter().all(|closed| closed) {
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => InstalledOperation::fail(14),
        }
    }

    fn emit_or_await(&self) -> OperationAction {
        let Some(selector) = self.selector else {
            return OperationAction::Await;
        };
        let Some(value) = self.candidates[usize::from(selector)] else {
            return OperationAction::Await;
        };
        if self.candidates[usize::from(!selector)].is_none() {
            return OperationAction::Await;
        }
        OperationAction::EmitCanonical {
            port: PortId(0),
            value: conduit_kernel::CanonicalValue::new(&value)
                .expect("Scalar fits derived-value bound"),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.selector = None;
        self.candidates = [None; 2];
    }
}

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
    if placement.kind_id.as_str() != conduit_std_catalog::STATE_SELECT_KIND
        || placement.kind_contract_revision
            != conduit_std_catalog::STATE_SELECT_SCALAR_CONTRACT_REVISION.into()
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
