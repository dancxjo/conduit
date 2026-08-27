//! Finite software-cadenced demand for exact PCM render intervals.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_audio::AUDIO_RENDER_DEMAND_INFO_ID;
use conduit_core::{
    kind_id, monotonic_timer_host_operation_requirement, monotonic_timer_resource_requirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    ImplementationId, ImplementationOffer, KindContractRevision, PortDirection,
};

pub const AUDIO_RENDER_DEMAND_KIND: &str = "audio/render-demand";
pub const AUDIO_RENDER_DEMAND_REVISION: &str = "conduit.std/audio-render-demand@1";
pub const AUDIO_RENDER_DEMAND_PROFILE: &str = "std/monotonic-audio-render-p240-c256@1";
pub const AUDIO_RENDER_DEMAND_IMPLEMENTATION: &str = "std/kernel-audio-render-demand@1";
pub const AUDIO_RENDER_DEMAND_ARTIFACT: &str = "conduit-std-host/audio-render-demand@1";
pub const AUDIO_RENDER_DEMAND_CAPABILITY: &str = "audio-render-demand-v1";
pub const AUDIO_RENDER_BLOCK_FRAMES_KEY: &str = "block-frames";
pub const AUDIO_RENDER_MAXIMUM_BLOCKS_KEY: &str = "maximum-blocks";
pub const AUDIO_RENDER_BLOCK_FRAMES: u16 = 240;
pub const AUDIO_RENDER_MAXIMUM_BLOCKS: u16 = 256;
pub const AUDIO_RENDER_PERIOD_MILLIS: u64 = 5;
pub const AUDIO_RENDER_CLOCK_ID: u64 = 1;

/// A finite software-cadenced render profile. It establishes an explicit
/// ordinary timing seam for the reference path without claiming that the
/// monotonic Host clock is the physical device clock.
pub fn audio_render_demand_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(AUDIO_RENDER_DEMAND_KIND),
        plain_name: "Request bounded audio intervals".to_string(),
        summary: "Emit exact finite PCM frame intervals on one admitted clock.".to_string(),
        inputs: Vec::new(),
        outputs: vec![super::sound::port(
            "demand",
            AUDIO_RENDER_DEMAND_INFO_ID,
            PortDirection::Output,
        )],
        configuration: audio_render_demand_configuration(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: AUDIO_RENDER_MAXIMUM_BLOCKS,
            max_queue_bytes: u32::from(AUDIO_RENDER_MAXIMUM_BLOCKS)
                * conduit_audio::AUDIO_RENDER_DEMAND_ENCODED_LEN as u32,
        },
        terminal_behavior: TerminalBehavior::CompletesAfterFixedCount {
            count: u64::from(AUDIO_RENDER_MAXIMUM_BLOCKS),
        },
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "render: audio/render-demand".to_string(),
    }
}

pub fn audio_render_demand_offer() -> CapabilityOffer {
    let contract = audio_render_demand_contract();
    CapabilityOffer {
        startup_parameters: crate::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(AUDIO_RENDER_DEMAND_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(AUDIO_RENDER_DEMAND_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(AUDIO_RENDER_DEMAND_PROFILE),
            implementation_id: ImplementationId::from(AUDIO_RENDER_DEMAND_IMPLEMENTATION),
            artifact_id: conduit_core::ArtifactId::from(AUDIO_RENDER_DEMAND_ARTIFACT),
        },
        host_operations: vec![monotonic_timer_host_operation_requirement()],
        resource_requirements: vec![monotonic_timer_resource_requirement()],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

pub fn audio_render_demand_configuration() -> Vec<StandardConfigurationField> {
    vec![
        exact_u64(
            AUDIO_RENDER_BLOCK_FRAMES_KEY,
            u64::from(AUDIO_RENDER_BLOCK_FRAMES),
        ),
        exact_u64(
            AUDIO_RENDER_MAXIMUM_BLOCKS_KEY,
            u64::from(AUDIO_RENDER_MAXIMUM_BLOCKS),
        ),
    ]
}

fn exact_u64(key: &str, value: u64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::U64(value),
        rule: StandardConfigurationRule::U64Range {
            minimum: value,
            maximum: value,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_profile_is_exact_and_uses_the_monotonic_deadline_base() {
        let contract = audio_render_demand_contract();
        let offer = audio_render_demand_offer();
        assert!(contract.inputs.is_empty());
        assert_eq!(contract.outputs.len(), 1);
        assert_eq!(
            contract.outputs[0].value_kind.as_str(),
            AUDIO_RENDER_DEMAND_INFO_ID
        );
        assert_eq!(contract.configuration, audio_render_demand_configuration());
        assert_eq!(offer.startup_parameters.len(), 2);
        assert_eq!(
            offer.host_operations,
            vec![monotonic_timer_host_operation_requirement()]
        );
        assert_eq!(
            offer.resource_requirements,
            vec![monotonic_timer_resource_requirement()]
        );
        assert_eq!(
            contract.terminal_behavior,
            TerminalBehavior::CompletesAfterFixedCount {
                count: u64::from(AUDIO_RENDER_MAXIMUM_BLOCKS)
            }
        );
    }
}
