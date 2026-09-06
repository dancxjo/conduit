//! Finite, effect-free realization of ordered nominal pulse observations.
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, ImplementationId, ImplementationOffer,
};
pub const PULSE_OBSERVE_PROFILE: &str = "std/pulse-observe-ordered-64@1";
pub const PULSE_OBSERVE_IMPLEMENTATION: &str = "std/kernel-pulse-observe@1";
pub const PULSE_OBSERVE_ARTIFACT: &str = "conduit-std-host/pulse-observe@1";

pub fn pulse_observe_offer() -> CapabilityOffer {
    let contract = conduit_time::pulse_observe_kind_definition();
    CapabilityOffer {
        capability_id: CapabilityId::from("pulse-observe"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        startup_parameters: ["period-ms", "maximum-pulses"]
            .into_iter()
            .map(|name| FaceStartupParameter {
                name: name.into(),
                value_type: "Count".into(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PULSE_OBSERVE_PROFILE),
            implementation_id: ImplementationId::from(PULSE_OBSERVE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(PULSE_OBSERVE_ARTIFACT),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: conduit_time::TICK_ENCODED_LEN,
        },
    }
}
