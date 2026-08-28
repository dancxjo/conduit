use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal,
};
use conduit_semantic_catalog::{
    CAMERA_ACQUIRE_KIND, CAMERA_FRAME_KIND, CAMERA_FRAME_SINK_KIND, CAMERA_REQUEST_KIND,
    CAMERA_RESOURCE_CLASS, CAMERA_SOURCE_KIND, MAXIMUM_MEDIA_QUEUE_BYTES,
    MAXIMUM_MEDIA_QUEUE_ITEMS, MAXIMUM_MEDIA_REQUEST_BYTES, MAXIMUM_MEDIA_RESULT_BYTES,
    MAXIMUM_MEDIA_VALUE_BYTES, MEDIA_ACQUIRE_OPERATION, MEDIA_ACQUISITION_RESULT_KIND,
    MEDIA_REQUEST_AUTHORITY, MEDIA_USE_AUTHORITY, MEDIA_USE_OPERATION, MICROPHONE_ACQUIRE_KIND,
    MICROPHONE_REQUEST_KIND,
};

pub const BROWSER_MEDIA_PROFILE: &str = "browser/human-media@1";
pub const BROWSER_MEDIA_ARTIFACT: &str = "conduit-browser-runtime/human-media@1";

pub fn browser_media_acquisition_offers() -> Vec<CapabilityOffer> {
    vec![
        acquisition_offer(CAMERA_ACQUIRE_KIND, CAMERA_REQUEST_KIND),
        acquisition_offer(MICROPHONE_ACQUIRE_KIND, MICROPHONE_REQUEST_KIND),
    ]
}

/// Camera source made available only with post-acquisition resource truth.
pub fn acquired_camera_source_offer() -> CapabilityOffer {
    let operation = HostOperationContractId::from(MEDIA_USE_OPERATION);
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("browser/acquired-camera-source@1"),
        kind_id: kind_id(CAMERA_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.std/camera-source@1"),
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("frame"),
            value_kind: kind_id(CAMERA_FRAME_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
        }],
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BROWSER_MEDIA_PROFILE),
            implementation_id: ImplementationId::from("browser/acquired-camera-source@1"),
            artifact_id: ArtifactId::from(BROWSER_MEDIA_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: operation.clone(),
            target_kind: Some(kind_id(CAMERA_FRAME_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: MAXIMUM_MEDIA_VALUE_BYTES,
        }],
        resource_requirements: vec![resource_requirement(CAMERA_RESOURCE_CLASS, 1)],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MEDIA_USE_AUTHORITY),
            host_operation_contract_id: operation,
            subject_kind: kind_id(CAMERA_FRAME_KIND),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_MEDIA_VALUE_BYTES,
        },
    }
}

pub fn browser_camera_frame_sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("browser/camera-frame-sink@1"),
        kind_id: kind_id(CAMERA_FRAME_SINK_KIND),
        kind_contract_revision: KindContractRevision::from("conduit.std/camera-frame-sink@1"),
        inputs: vec![PortDescriptor {
            port_id: port_id("frame"),
            value_kind: kind_id(CAMERA_FRAME_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![],
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                "conduit.std/camera-frame-sink-kernel@1",
            ),
            implementation_id: ImplementationId::from("std/kernel-camera-frame-sink@1"),
            artifact_id: ArtifactId::from(BROWSER_MEDIA_ARTIFACT),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_MEDIA_VALUE_BYTES,
        },
    }
}

fn acquisition_offer(kind: &str, request_kind: &str) -> CapabilityOffer {
    let operation = HostOperationContractId::from(MEDIA_ACQUIRE_OPERATION);
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("browser/{kind}-capability").as_str()),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from("conduit.std/human-media@1"),
        inputs: vec![PortDescriptor {
            port_id: port_id("request"),
            value_kind: kind_id(request_kind),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: kind_id(MEDIA_ACQUISITION_RESULT_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BROWSER_MEDIA_PROFILE),
            implementation_id: ImplementationId::from(format!("browser/{kind}").as_str()),
            artifact_id: ArtifactId::from(BROWSER_MEDIA_ARTIFACT),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: operation.clone(),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_MEDIA_REQUEST_BYTES,
            maximum_output_bytes: MAXIMUM_MEDIA_RESULT_BYTES,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MEDIA_REQUEST_AUTHORITY),
            host_operation_contract_id: operation,
            subject_kind: kind_id(kind),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_MEDIA_QUEUE_ITEMS,
            max_queue_bytes: MAXIMUM_MEDIA_QUEUE_BYTES,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquisition_ports_and_all_limits_are_exact_and_finite() {
        for offer in browser_media_acquisition_offers() {
            assert_eq!(offer.inputs.len(), 1);
            assert_eq!(offer.inputs[0].port_id.as_str(), "request");
            assert_eq!(offer.outputs.len(), 1);
            assert_eq!(offer.outputs[0].port_id.as_str(), "result");
            assert_eq!(offer.authority_requirements.len(), 1);
            assert_eq!(offer.host_operations.len(), 1);
            assert!(offer.limits.max_active_instances > 0);
            assert!(offer.limits.max_queue_items > 0);
            assert!(offer.limits.max_queue_bytes > 0);
            assert!(offer.host_operations[0].maximum_input_bytes > 0);
            assert!(offer.host_operations[0].maximum_output_bytes > 0);
        }
    }
}
