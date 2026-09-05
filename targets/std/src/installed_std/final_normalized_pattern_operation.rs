//! Installed finite Flow-to-final-Value normalized-pattern selection.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{Failure, FailureCode, OperationAction, OperationInput, PortId, ValueRef};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::FINAL_NORMALIZED_PATTERN_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct FinalNormalizedPatternOperation {
    latest: Option<ValueRef>,
    released: Option<ValueRef>,
    accepted: u64,
    maximum: u64,
    retain_resumed: bool,
    complete_after_emit: bool,
}

impl FinalNormalizedPatternOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.accepted < self.maximum => {
                self.accepted += 1;
                self.retain_resumed = true;
                self.released = self.latest.replace(value);
                OperationAction::Await
            }
            OperationInput::Value {
                port: PortId(0), ..
            } => fail(FailureCode::StorageExhausted, 1),
            OperationInput::Closed { port: PortId(0) } => {
                let Some(value) = self.latest.take() else {
                    return fail(FailureCode::InvalidInput, 2);
                };
                self.complete_after_emit = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 3),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.complete_after_emit {
            self.complete_after_emit = false;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.latest = None;
        self.released = None;
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.take()
    }
}

fn validate(placement: &PlannedGear) -> Result<u64, String> {
    let offer = conduit_std_offers::final_normalized_pattern_std_offer();
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
        || placement.limits != offer.limits
    {
        return Err("planned final pattern differs from installed realization".into());
    }
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-values", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("final pattern value bound is absent")?;
    if !(1..=conduit_semantic_catalog::MAXIMUM_FINAL_PATTERN_VALUES).contains(&maximum) {
        return Err("final pattern value bound is outside reviewed limits".into());
    }
    Ok(maximum)
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let maximum = validate(placement)?;
    Ok(OperationBudget {
        value_items: u16::try_from(maximum + 1).map_err(|_| "final pattern item overflow")?,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32).saturating_mul(maximum as u32 + 1),
        host_requests: 0,
        sign_items: u16::try_from(maximum.saturating_mul(6) + 8)
            .map_err(|_| "final pattern sign overflow")?,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    Ok(InstalledOperation::FinalNormalizedPattern(
        FinalNormalizedPatternOperation {
            latest: None,
            released: None,
            accepted: 0,
            maximum: validate(placement)?,
            retain_resumed: false,
            complete_after_emit: false,
        },
    ))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueStorage;

    fn operation(maximum: u64) -> FinalNormalizedPatternOperation {
        FinalNormalizedPatternOperation {
            latest: None,
            released: None,
            accepted: 0,
            maximum,
            retain_resumed: false,
            complete_after_emit: false,
        }
    }

    #[test]
    fn empty_and_over_bound_flows_fail_distinctly() {
        assert!(matches!(
            operation(1).resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 2
            })
        ));
        let mut operation = operation(0);
        let mut store = conduit_kernel::HostedValueStore::new(1, 1, 1).unwrap();
        let value = store.store(&[0]).unwrap();
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value,
            }),
            OperationAction::Fail(Failure {
                code: FailureCode::StorageExhausted,
                detail: 1
            })
        ));
    }
}
