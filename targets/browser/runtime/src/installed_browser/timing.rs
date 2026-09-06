//! Browser timing transforms using the shared preallocated semantic codecs.
use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{CapabilityOffer, PlannedGear};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};
use conduit_semantic_catalog::{BoundedIntervalCodec, BoundedNormalizationCodec};

const MAXIMUM: u32 = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
const IMPLEMENTATIONS: [&str; 2] = [
    "browser/kernel-ordered-event-intervals@1",
    "browser/kernel-normalize-relative-duration@1",
];
pub(crate) const OPERATIONS: [&str; 2] = [
    "conduit.host/ordered-event-intervals@1",
    "conduit.host/normalize-relative-duration@1",
];
pub(super) static INTERVALS: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATIONS[0],
    offer: interval_offer,
    prepare,
    perform: None,
};
pub(super) static NORMALIZE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATIONS[1],
    offer: normalization_offer,
    prepare,
    perform: None,
};
fn interval_offer() -> CapabilityOffer {
    offer(0)
}
fn normalization_offer() -> CapabilityOffer {
    offer(1)
}
fn offer(index: usize) -> CapabilityOffer {
    let contract = if index == 0 {
        conduit_semantic_catalog::ordered_event_intervals_definition()
    } else {
        conduit_semantic_catalog::normalize_relative_duration_definition()
    };
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: IMPLEMENTATIONS[index].into(),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: "browser/bounded-timing@1".into(),
            implementation_id: IMPLEMENTATIONS[index].into(),
            artifact_id: "conduit-browser-runtime/bounded-timing@1".into(),
        },
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: OPERATIONS[index].into(),
            target_kind: Some(contract.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM,
            maximum_output_bytes: MAXIMUM,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM * 2,
        },
    }
}
fn index(placement: &PlannedGear) -> Option<usize> {
    IMPLEMENTATIONS
        .iter()
        .position(|id| *id == placement.implementation_id.as_str())
}
fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    let index = index(placement).ok_or("unknown timing implementation")?;
    validate_placement(placement, &offer(index))?;
    if !placement.configuration.is_empty() {
        return Err("unexpected timing configuration".into());
    }
    Ok(BrowserOperation::unary(MAXIMUM, 1))
}

pub(crate) enum PreparedTiming {
    Intervals(BoundedIntervalCodec),
    Normalize(BoundedNormalizationCodec),
}
impl PreparedTiming {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        let Some(index) = index(placement) else {
            return Ok(None);
        };
        validate_placement(placement, &offer(index))?;
        if !placement.configuration.is_empty() {
            return Err("unexpected timing configuration".into());
        }
        Ok(Some(if index == 0 {
            Self::Intervals(BoundedIntervalCodec::prepare())
        } else {
            Self::Normalize(BoundedNormalizationCodec::prepare())
        }))
    }
    pub(crate) fn execute(&mut self, operation: &str, input: &[u8]) -> Result<&[u8], Failure> {
        if input.len() > MAXIMUM as usize {
            return Err(failure(1));
        }
        match self {
            Self::Intervals(codec) if operation == OPERATIONS[0] => {
                codec.execute(input).map_err(|error| {
                    use conduit_semantic_catalog::TimedPatternRefusal::*;
                    failure(match error {
                        Malformed => 1,
                        TooFewEvents => 2,
                        TooManyEvents => 3,
                        ReorderedOrDuplicateEvent => 4,
                        IntervalOverflow => 5,
                    })
                })
            }
            Self::Normalize(codec) if operation == OPERATIONS[1] => {
                codec.execute(input).map_err(|error| {
                    use conduit_semantic_catalog::SequenceNormalizationRefusal::*;
                    failure(match error {
                        Malformed => 1,
                        Empty => 2,
                        TooManyValues => 3,
                        ZeroDuration => 4,
                    })
                })
            }
            _ => Err(failure(1)),
        }
    }
}
fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

#[cfg(test)]
#[path = "timing_tests.rs"]
mod tests;
