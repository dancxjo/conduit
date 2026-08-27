use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, PortDescriptor, PortDirection, SCALAR_ENCODED_LEN};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef};

pub(super) static STATE_LATEST_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::STATE_LATEST_SCALAR_IMPLEMENTATION,
    budget: state_latest_budget,
    prepare: prepare_state_latest,
};

pub(super) static FLOW_TEE_SCALAR_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::FLOW_TEE_SCALAR_IMPLEMENTATION,
    budget: flow_tee_budget,
    prepare: prepare_flow_tee,
};

pub(super) struct StateLatestScalarOperation {
    held: Option<ValueRef>,
    released: Option<ValueRef>,
    retain_resumed: bool,
}

pub(super) struct FlowTeeScalarOperation {
    pending: Option<ValueRef>,
    phase: u8,
}

impl StateLatestScalarOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if value.byte_len == SCALAR_ENCODED_LEN as u32 => {
                self.released = self.held.replace(value);
                self.retain_resumed = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            OperationInput::Closed { port: PortId(0) } => {
                self.retain_resumed = false;
                self.released = self.held.take();
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(12),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.take()
    }

    pub(super) fn cancel(&mut self) {
        self.held = None;
        self.released = None;
        self.retain_resumed = false;
    }
}

impl FlowTeeScalarOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if value.byte_len == SCALAR_ENCODED_LEN as u32 && self.pending.is_none() => {
                self.pending = Some(value);
                self.phase = 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(13),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        match (self.pending, self.phase) {
            (Some(value), 1) => {
                self.phase = 2;
                OperationAction::Emit {
                    port: PortId(1),
                    value,
                }
            }
            (Some(_), 2) => {
                self.pending = None;
                self.phase = 0;
                OperationAction::Await
            }
            _ => InstalledOperation::fail(13),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.phase = 0;
    }
}

fn state_latest_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_state_latest(placement)?;
    Ok(budget())
}

fn flow_tee_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_flow_tee(placement)?;
    Ok(budget())
}

fn budget() -> OperationBudget {
    OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    }
}

fn prepare_state_latest(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_state_latest(placement)?;
    Ok(InstalledOperation::StateLatestScalar(
        StateLatestScalarOperation {
            held: None,
            released: None,
            retain_resumed: false,
        },
    ))
}

fn prepare_flow_tee(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_flow_tee(placement)?;
    Ok(InstalledOperation::FlowTeeScalar(FlowTeeScalarOperation {
        pending: None,
        phase: 0,
    }))
}

fn validate_state_latest(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        InstalledIdentity {
            kind: conduit_std_catalog::LATEST_KIND,
            revision: conduit_std_catalog::STATE_LATEST_SCALAR_CONTRACT_REVISION,
            profile: conduit_std_offers::STATE_LATEST_SCALAR_EXECUTION_PROFILE,
            implementation: conduit_std_offers::STATE_LATEST_SCALAR_IMPLEMENTATION,
            artifact: conduit_std_offers::STATE_LATEST_SCALAR_ARTIFACT,
        },
        &conduit_std_catalog::state_latest_scalar_contract().inputs,
        &conduit_std_catalog::state_latest_scalar_contract().outputs,
    )
}

fn validate_flow_tee(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        InstalledIdentity {
            kind: conduit_std_catalog::TEE_KIND,
            revision: conduit_std_catalog::FLOW_TEE_SCALAR_CONTRACT_REVISION,
            profile: conduit_std_offers::FLOW_TEE_SCALAR_EXECUTION_PROFILE,
            implementation: conduit_std_offers::FLOW_TEE_SCALAR_IMPLEMENTATION,
            artifact: conduit_std_offers::FLOW_TEE_SCALAR_ARTIFACT,
        },
        &conduit_std_catalog::flow_tee_scalar_contract().inputs,
        &conduit_std_catalog::flow_tee_scalar_contract().outputs,
    )
}

struct InstalledIdentity {
    kind: &'static str,
    revision: &'static str,
    profile: &'static str,
    implementation: &'static str,
    artifact: &'static str,
}

fn validate_identity(
    placement: &PlannedGear,
    identity: InstalledIdentity,
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
) -> Result<(), String> {
    if placement.kind_id.as_str() != identity.kind
        || placement.kind_contract_revision.as_str() != identity.revision
        || placement.execution_profile_id.as_str() != identity.profile
        || placement.implementation_id.as_str() != identity.implementation
        || placement.artifact_id.as_str() != identity.artifact
        || placement.inputs != inputs
        || placement.outputs != outputs
        || !placement.configuration.is_empty()
        || inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
        || outputs
            .iter()
            .any(|port| port.direction != PortDirection::Output)
    {
        return Err("planned flow/state scalar identity does not match its installation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: SCALAR_ENCODED_LEN as u32,
        }
    }

    #[test]
    fn latest_replaces_one_retained_value_and_releases_on_close() {
        let mut operation = StateLatestScalarOperation {
            held: None,
            released: None,
            retain_resumed: false,
        };
        assert_eq!(operation.start(), OperationAction::Await);
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: value(1),
            }),
            OperationAction::Emit { value: found, .. } if found == value(1)
        ));
        assert!(operation.retains_resumed_value());
        assert_eq!(operation.take_released_value(), None);
        operation.resume(OperationInput::Value {
            port: PortId(0),
            value: value(2),
        });
        assert_eq!(operation.take_released_value(), Some(value(1)));
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );
        assert_eq!(operation.take_released_value(), Some(value(2)));
    }

    #[test]
    fn tee_exposes_both_outputs_in_one_operation_transaction() {
        let mut operation = FlowTeeScalarOperation {
            pending: None,
            phase: 0,
        };
        assert_eq!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value: value(3),
            }),
            OperationAction::Emit {
                port: PortId(0),
                value: value(3),
            }
        );
        assert_eq!(
            operation.advance(),
            OperationAction::Emit {
                port: PortId(1),
                value: value(3),
            }
        );
        assert_eq!(operation.advance(), OperationAction::Await);
    }
}
