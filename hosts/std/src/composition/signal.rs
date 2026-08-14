//! Exact Signal family offers for the std reference composition.

use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ImplementationId,
};
use conduit_signal::{
    pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
    pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_inputs, show_resource_requirements, PULSE_KIND,
    SHOW_KIND,
};

pub(super) fn offers() -> [CapabilityOffer; 2] {
    [
        CapabilityOffer {
            startup_parameters: conduit_signal::pulse_face_startup_parameters(),
            shorthand: None,
            capability_id: CapabilityId::from("pulse-1"),
            kind_id: kind_id(PULSE_KIND),
            kind_contract_revision: pulse_contract_revision(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: pulse_execution_profile(),
                implementation_id: ImplementationId::from("std/pulse-v1"),
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
            },
            inputs: vec![],
            outputs: pulse_outputs(),
            host_operations: pulse_host_operation_requirements(),
            resource_requirements: pulse_resource_requirements(),
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 4,
                max_queue_bytes: 64,
            },
        },
        CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from("stdout-show-1"),
            kind_id: kind_id(SHOW_KIND),
            kind_contract_revision: show_contract_revision(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: show_execution_profile(),
                implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
            },
            inputs: show_inputs(),
            outputs: vec![],
            host_operations: show_host_operation_requirements(),
            resource_requirements: show_resource_requirements(),
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 4,
                max_queue_bytes: 64,
            },
        },
    ]
}
