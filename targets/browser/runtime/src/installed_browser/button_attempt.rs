//! Browser realization of the shared finite timed-attempt operation.

use conduit_core::{
    monotonic_timer_host_operation_requirement, monotonic_timer_resource_requirement, ArtifactId,
    CapabilityId, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    TIMER_RESOURCE_CLASS,
};

pub const TIMED_BUTTON_ATTEMPT_BROWSER_PROFILE: &str =
    "browser/pressed-button-attempt-kernel-hosted@1";
pub const TIMED_BUTTON_ATTEMPT_BROWSER_IMPLEMENTATION: &str =
    "browser/kernel-pressed-button-attempt@1";
pub const TIMED_BUTTON_ATTEMPT_BROWSER_ARTIFACT: &str =
    "conduit-browser-runtime/pressed-button-attempt@1";
pub const TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_OPERATION: &str =
    "conduit.host/observe-pressed-button-instant@1";

pub fn offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::timed_button_attempt_definition();
    let mut deadline = monotonic_timer_host_operation_requirement();
    deadline.target_kind = Some(contract.kind_id.clone());
    CapabilityOffer {
        startup_parameters: vec![
            FaceStartupParameter {
                name: "maximum-transitions".into(),
                value_type: "Count".into(),
                has_default: true,
            },
            FaceStartupParameter {
                name: "maximum-presses".into(),
                value_type: "Count".into(),
                has_default: true,
            },
            FaceStartupParameter {
                name: "timeout-ms".into(),
                value_type: "Duration".into(),
                has_default: true,
            },
        ],
        shorthand: None,
        capability_id: CapabilityId::from("pressed-button-attempt"),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TIMED_BUTTON_ATTEMPT_BROWSER_PROFILE),
            implementation_id: ImplementationId::from(TIMED_BUTTON_ATTEMPT_BROWSER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TIMED_BUTTON_ATTEMPT_BROWSER_ARTIFACT),
        },
        host_operations: vec![
            deadline,
            HostOperationRequirement {
                contract_id: HostOperationContractId::from(
                    TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_OPERATION,
                ),
                target_kind: Some(contract.kind_id),
                maximum_in_flight: 1,
                maximum_input_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            },
        ],
        resource_requirements: vec![
            monotonic_timer_resource_requirement(),
            conduit_core::resource_requirement(TIMER_RESOURCE_CLASS, 1),
        ],
        authority_requirements: Vec::new(),
        limits: conduit_core::CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS as u16,
            max_queue_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32
                * (conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS as u32 + 1),
        },
    }
}

pub(super) fn prepare(
    placement: &conduit_core::PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<super::BrowserOperation, String> {
    use conduit_kernel::ValueStorage;
    let installed = offer();
    super::factory::validate_placement(placement, &installed)?;
    if placement.limits.max_active_instances > installed.limits.max_active_instances
        || placement.limits.max_queue_items > installed.limits.max_queue_items
        || placement.limits.max_queue_bytes > installed.limits.max_queue_bytes
        || !placement.authority.is_empty()
    {
        return Err("timed attempt admission differs from installed limits".into());
    }
    for requirement in &installed.resource_requirements {
        if !placement.resources.iter().any(|resource| {
            resource.class_id == requirement.class_id
                && resource.units == requirement.units
                && resource.protected.is_none()
                && resource.compute.is_none()
        }) {
            return Err("timed attempt lacks its admitted timer resource".into());
        }
    }
    let get = |name: &str| {
        placement
            .configuration
            .iter()
            .find_map(|entry| match &entry.value {
                conduit_core::ConfigurationValue::U64(value) if entry.key == name => Some(*value),
                _ => None,
            })
            .ok_or_else(|| format!("timed attempt lacks {name}"))
    };
    let presses = get("maximum-presses")?;
    let transitions = get("maximum-transitions")?;
    let timeout = get("timeout-ms")?;
    if !(2..=conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS as u64).contains(&presses)
        || !(presses..=conduit_semantic_catalog::MAXIMUM_ATTEMPT_TRANSITIONS).contains(&transitions)
        || !(1..=conduit_semantic_catalog::MAXIMUM_ATTEMPT_TIMEOUT_MS).contains(&timeout)
    {
        return Err("timed attempt configuration exceeds reviewed bounds".into());
    }
    let durations = (0..transitions)
        .map(|_| {
            values
                .store(&conduit_core::encode_monotonic_duration(timeout))
                .map_err(|error| format!("admit timed attempt duration: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(super::BrowserOperation::installed(
        conduit_time::TimedButtonAttemptOperation::from_prepared_durations(
            durations,
            transitions,
            super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        ),
    ))
}

pub(super) static INSTALLATION: super::factory::BrowserInstallation =
    super::factory::BrowserInstallation {
        implementation_id: TIMED_BUTTON_ATTEMPT_BROWSER_IMPLEMENTATION,
        offer,
        prepare,
        perform: None,
    };

#[cfg(test)]
#[path = "button_attempt_tests.rs"]
mod tests;

pub(super) fn admit_clock_resource(resources: &mut Vec<conduit_core::ResourceOffer>) {
    resources.push(conduit_core::resource_offer(
        "browser/monotonic-millisecond-timer",
        conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS,
        1,
    ));
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
}

pub(crate) fn prepare_codec(
    placement: &conduit_core::PlannedGear,
) -> Result<Option<conduit_semantic_catalog::BoundedButtonAttemptCodec>, String> {
    if placement.implementation_id.as_str() != TIMED_BUTTON_ATTEMPT_BROWSER_IMPLEMENTATION {
        return Ok(None);
    }
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match entry.value {
            conduit_core::ConfigurationValue::U64(value) if entry.key == "maximum-presses" => {
                Some(value)
            }
            _ => None,
        })
        .ok_or("timed attempt lacks maximum presses")?;
    if !(2..=conduit_semantic_catalog::MAXIMUM_TIMED_EVENTS as u64).contains(&maximum) {
        return Err("timed attempt press capacity is invalid".into());
    }
    Ok(Some(
        conduit_semantic_catalog::BoundedButtonAttemptCodec::prepare(maximum as usize),
    ))
}
