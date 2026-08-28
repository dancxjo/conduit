//! Hosted std realizations of portable timing and render-cadence contracts.

use conduit_core::{
    monotonic_timer_host_operation_requirement, monotonic_timer_resource_requirement,
    resource_requirement, wait_host_operation_requirement, CapabilityOffer,
    HostOperationRequirement, ResourceRequirement, TIMER_RESOURCE_CLASS,
};
use conduit_semantic_catalog::{realization_offer, RealizationOfferIdentity, StandardKindContract};

pub const TICK_EXECUTION_PROFILE: &str = "conduit.std/time-tick-kernel-hosted@2";
pub const TICK_IMPLEMENTATION: &str = "std/kernel-time-tick@2";
pub const TICK_ARTIFACT: &str = "conduit-std-host/time-tick@2";
pub const TIME_EVERY_EXECUTION_PROFILE: &str = "conduit.std/time-every-kernel-hosted@1";
pub const TIME_EVERY_IMPLEMENTATION: &str = "std/kernel-time-every@1";
pub const TIME_EVERY_ARTIFACT: &str = "conduit-std-host/time-every@1";
pub const AUDIO_RENDER_DEMAND_PROFILE: &str = "std/monotonic-audio-render-p240-c256@1";
pub const AUDIO_RENDER_DEMAND_IMPLEMENTATION: &str = "std/kernel-audio-render-demand@1";
pub const AUDIO_RENDER_DEMAND_ARTIFACT: &str = "conduit-std-host/audio-render-demand@1";
pub const TIME_DEBOUNCE_EXECUTION_PROFILE: &str = "conduit.std/time-debounce-bool-kernel-hosted@1";
pub const TIME_DEBOUNCE_IMPLEMENTATION: &str = "std/kernel-time-debounce-bool@1";
pub const TIME_DEBOUNCE_ARTIFACT: &str = "conduit-std-host/time-debounce-bool@1";
pub const TIME_TIMEOUT_EXECUTION_PROFILE: &str =
    "conduit.std/time-timeout-tick-bool-kernel-hosted@1";
pub const TIME_TIMEOUT_IMPLEMENTATION: &str = "std/kernel-time-timeout-tick-bool@1";
pub const TIME_TIMEOUT_ARTIFACT: &str = "conduit-std-host/time-timeout-tick-bool@1";
pub const TIME_DELAY_EXECUTION_PROFILE: &str = "conduit.std/time-delay-bool-kernel-hosted@1";
pub const TIME_DELAY_IMPLEMENTATION: &str = "std/kernel-time-delay-bool@1";
pub const TIME_DELAY_ARTIFACT: &str = "conduit-std-host/time-delay-bool@1";
pub const TIME_THROTTLE_EXECUTION_PROFILE: &str =
    "conduit.std/time-throttle-bool-leading-kernel-hosted@1";
pub const TIME_THROTTLE_IMPLEMENTATION: &str = "std/kernel-time-throttle-bool-leading@1";
pub const TIME_THROTTLE_ARTIFACT: &str = "conduit-std-host/time-throttle-bool-leading@1";

pub fn tick_capability_offer() -> CapabilityOffer {
    offer(
        conduit_semantic_catalog::tick_contract(),
        conduit_time::TICK_CONTRACT_REVISION,
        Identity {
            capability: "time-tick-v2",
            profile: TICK_EXECUTION_PROFILE,
            implementation: TICK_IMPLEMENTATION,
            artifact: TICK_ARTIFACT,
        },
        vec![wait_host_operation_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
    )
}

pub fn time_every_offer() -> CapabilityOffer {
    let mut offer = offer(
        conduit_semantic_catalog::time_every_contract(),
        conduit_time::TIME_EVERY_CONTRACT_REVISION,
        Identity {
            capability: "time-every-v1",
            profile: TIME_EVERY_EXECUTION_PROFILE,
            implementation: TIME_EVERY_IMPLEMENTATION,
            artifact: TIME_EVERY_ARTIFACT,
        },
        vec![wait_host_operation_requirement()],
        vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
    );
    offer.startup_parameters[0].value_type = "Duration".into();
    offer.startup_parameters[0].has_default = false;
    offer
}

pub fn audio_render_demand_offer() -> CapabilityOffer {
    monotonic_offer(
        conduit_semantic_catalog::audio_render_demand_contract(),
        conduit_semantic_catalog::AUDIO_RENDER_DEMAND_REVISION,
        Identity {
            capability: "audio-render-demand-v1",
            profile: AUDIO_RENDER_DEMAND_PROFILE,
            implementation: AUDIO_RENDER_DEMAND_IMPLEMENTATION,
            artifact: AUDIO_RENDER_DEMAND_ARTIFACT,
        },
        false,
    )
}

pub fn time_debounce_offer() -> CapabilityOffer {
    timing_offer(
        conduit_semantic_catalog::time_debounce_contract(),
        conduit_semantic_catalog::TIME_DEBOUNCE_CONTRACT_REVISION,
        "time-debounce-bool-v1",
        TIME_DEBOUNCE_EXECUTION_PROFILE,
        TIME_DEBOUNCE_IMPLEMENTATION,
        TIME_DEBOUNCE_ARTIFACT,
    )
}

pub fn time_timeout_offer() -> CapabilityOffer {
    timing_offer(
        conduit_semantic_catalog::time_timeout_contract(),
        conduit_semantic_catalog::TIME_TIMEOUT_CONTRACT_REVISION,
        "time-timeout-tick-bool-v1",
        TIME_TIMEOUT_EXECUTION_PROFILE,
        TIME_TIMEOUT_IMPLEMENTATION,
        TIME_TIMEOUT_ARTIFACT,
    )
}

pub fn time_delay_offer() -> CapabilityOffer {
    timing_offer(
        conduit_semantic_catalog::time_delay_contract(),
        conduit_semantic_catalog::TIME_DELAY_CONTRACT_REVISION,
        "time-delay-bool-v1",
        TIME_DELAY_EXECUTION_PROFILE,
        TIME_DELAY_IMPLEMENTATION,
        TIME_DELAY_ARTIFACT,
    )
}

pub fn time_throttle_offer() -> CapabilityOffer {
    timing_offer(
        conduit_semantic_catalog::time_throttle_contract(),
        conduit_semantic_catalog::TIME_THROTTLE_CONTRACT_REVISION,
        "time-throttle-bool-leading-v1",
        TIME_THROTTLE_EXECUTION_PROFILE,
        TIME_THROTTLE_IMPLEMENTATION,
        TIME_THROTTLE_ARTIFACT,
    )
}

fn timing_offer(
    contract: StandardKindContract,
    revision: &str,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    monotonic_offer(
        contract,
        revision,
        Identity {
            capability,
            profile,
            implementation,
            artifact,
        },
        true,
    )
}

fn monotonic_offer(
    contract: StandardKindContract,
    revision: &str,
    identity: Identity<'_>,
    duration_startup: bool,
) -> CapabilityOffer {
    let mut offer = offer(
        contract,
        revision,
        identity,
        vec![monotonic_timer_host_operation_requirement()],
        vec![monotonic_timer_resource_requirement()],
    );
    if duration_startup {
        offer.startup_parameters[0].value_type = "Duration".into();
    }
    offer
}

#[derive(Clone, Copy)]
struct Identity<'a> {
    capability: &'a str,
    profile: &'a str,
    implementation: &'a str,
    artifact: &'a str,
}

fn offer(
    contract: StandardKindContract,
    revision: &str,
    identity: Identity<'_>,
    host_operations: Vec<HostOperationRequirement>,
    resources: Vec<ResourceRequirement>,
) -> CapabilityOffer {
    realization_offer(
        contract,
        revision,
        RealizationOfferIdentity {
            capability: identity.capability,
            execution_profile: identity.profile,
            implementation: identity.implementation,
            artifact: identity.artifact,
        },
        host_operations,
        resources,
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_offers_preserve_exact_contracts_and_effect_requirements() {
        for offer in [
            time_debounce_offer(),
            time_timeout_offer(),
            time_delay_offer(),
            time_throttle_offer(),
        ] {
            assert_eq!(offer.host_operations.len(), 1);
            assert_eq!(offer.resource_requirements.len(), 1);
            assert_eq!(offer.startup_parameters[0].value_type, "Duration");
        }
        assert_eq!(
            time_every_offer().startup_parameters[0].value_type,
            "Duration"
        );
        assert!(!time_every_offer().startup_parameters[0].has_default);
        assert_eq!(tick_capability_offer().host_operations.len(), 1);
        assert_eq!(audio_render_demand_offer().host_operations.len(), 1);
    }
}
