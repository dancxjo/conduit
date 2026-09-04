//! Exact Signal implementation offers owned by the hosted std Host.

use conduit_core::{ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ImplementationId};

pub const SIGNAL_PULSE_STD_IMPLEMENTATION: &str = "std/pulse-v1";
pub const SIGNAL_SHOW_STD_IMPLEMENTATION: &str = "std/stdout-show-signal-v1";

pub fn signal_pulse_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: conduit_signal::pulse_face_startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from("pulse-1"),
        kind_id: conduit_signal::pulse_kind(),
        kind_contract_revision: conduit_signal::pulse_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: conduit_signal::pulse_execution_profile(),
            implementation_id: ImplementationId::from(SIGNAL_PULSE_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
        },
        inputs: vec![],
        outputs: conduit_signal::pulse_outputs(),
        host_operations: conduit_signal::pulse_host_operation_requirements(),
        resource_requirements: conduit_signal::pulse_resource_requirements(),
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

pub fn signal_show_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("stdout-show-1"),
        kind_id: conduit_signal::show_kind(),
        kind_contract_revision: conduit_signal::show_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: conduit_signal::show_execution_profile(),
            implementation_id: ImplementationId::from(SIGNAL_SHOW_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
        },
        inputs: conduit_signal::show_inputs(),
        outputs: vec![],
        host_operations: conduit_signal::show_host_operation_requirements(),
        resource_requirements: conduit_signal::show_resource_requirements(),
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_signal_offers_are_exact_target_truth_without_scenario_identity() {
        let pulse = signal_pulse_offer();
        let show = signal_show_offer();

        assert_eq!(
            pulse.implementation.implementation_id.as_str(),
            SIGNAL_PULSE_STD_IMPLEMENTATION
        );
        assert_eq!(
            show.implementation.implementation_id.as_str(),
            SIGNAL_SHOW_STD_IMPLEMENTATION
        );
        assert_eq!(pulse.kind_id, conduit_signal::pulse_kind());
        assert_eq!(show.kind_id, conduit_signal::show_kind());
        assert!(pulse.inputs.is_empty());
        assert!(show.outputs.is_empty());
        for offer in [pulse, show] {
            let identities = [
                offer.capability_id.as_str(),
                offer.implementation.execution_profile_id.as_str(),
                offer.implementation.implementation_id.as_str(),
                offer.implementation.artifact_id.as_str(),
            ];
            for scenario_identity in ["s4/", "pico", "browser", "websocket", "boot"] {
                assert!(identities
                    .iter()
                    .all(|identity| !identity.contains(scenario_identity)));
            }
        }
    }
}
