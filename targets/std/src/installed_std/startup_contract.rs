//! Borrowed exact installation checks for the existing canonical startup offers.
use conduit_core::{
    KindSemanticLaw, KindTerminalBehavior, PlannedGear, PortDirection, PortTemporal,
};
pub(super) fn exact(gear: &PlannedGear, source: bool) -> bool {
    let (kind, revision, capability, profile, implementation, artifact, terminal, direction) =
        if source {
            (
                conduit_semantic_catalog::BODY_WAKE_KIND,
                conduit_semantic_catalog::BODY_WAKE_REVISION,
                conduit_std_offers::BODY_WAKE_CAPABILITY,
                conduit_std_offers::BODY_WAKE_PROFILE,
                conduit_std_offers::BODY_WAKE_IMPLEMENTATION,
                conduit_std_offers::BODY_WAKE_ARTIFACT,
                KindTerminalBehavior::EmitsOnceWhenScopeIsEligible,
                PortDirection::Output,
            )
        } else {
            (
                conduit_semantic_catalog::STARTUP_CHIME_KIND,
                conduit_semantic_catalog::STARTUP_CHIME_REVISION,
                "startup-chime",
                conduit_std_offers::STARTUP_CHIME_PROFILE,
                conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION,
                conduit_std_offers::STARTUP_CHIME_ARTIFACT,
                KindTerminalBehavior::CompletesWhenInputsClose,
                PortDirection::Input,
            )
        };
    let ports = if source { &gear.outputs } else { &gear.inputs };
    let host_calls = if source {
        gear.host_calls.is_empty()
    } else {
        gear.host_calls.len() == 1
            && gear.host_calls[0].contract_id.as_str()
                == conduit_std_offers::STARTUP_CHIME_OPERATION
            && gear.host_calls[0]
                .target_kind
                .as_ref()
                .is_some_and(|kind| kind.as_str() == conduit_core::BOOL_INFO_ID)
            && gear.host_calls[0].maximum_in_flight == 1
            && gear.host_calls[0].maximum_input_bytes == conduit_core::BOOL_ENCODED_LEN as u32
            && gear.host_calls[0].maximum_output_bytes == 0
    };
    host_calls
        && gear.kind_id.as_str() == kind
        && gear.kind_contract_revision.as_str() == revision
        && gear.capability_id.as_str() == capability
        && gear.execution_profile_id.as_str() == profile
        && gear.implementation_id.as_str() == implementation
        && gear.artifact_id.as_str() == artifact
        && gear.semantic_contract.configuration.is_empty()
        && gear.semantic_contract.laws.as_slice() == [KindSemanticLaw::Terminal(terminal)]
        && gear.realization_properties.is_empty()
        && gear.pool_references.is_empty()
        && gear.configuration.is_empty()
        && gear.terminal_transductions.is_empty()
        && (if source {
            gear.inputs.is_empty()
        } else {
            gear.outputs.is_empty()
        })
        && ports.len() == 1
        && ports[0].port_id.as_str() == "pulse"
        && ports[0].value_kind.as_str() == conduit_core::BOOL_INFO_ID
        && ports[0].direction == direction
        && ports[0].temporal == (PortTemporal::Flow { closes: true })
        && ports[0].abnormal_kind.is_none()
        && gear.limits.max_active_instances == 16
        && gear.limits.max_queue_items == 1
        && gear.limits.max_queue_bytes == conduit_core::BOOL_ENCODED_LEN as u32
}
