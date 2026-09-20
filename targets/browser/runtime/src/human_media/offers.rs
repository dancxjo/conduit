use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, Kind,
};
use conduit_semantic_catalog::{
    CAMERA_ACQUIRE_KIND, CAMERA_FRAME_KIND, CAMERA_RESOURCE_CLASS, MAXIMUM_MEDIA_REQUEST_BYTES,
    MAXIMUM_MEDIA_RESULT_BYTES, MAXIMUM_MEDIA_VALUE_BYTES, MEDIA_ACQUIRE_OPERATION,
    MEDIA_REQUEST_AUTHORITY, MEDIA_USE_AUTHORITY, MEDIA_USE_OPERATION, MICROPHONE_ACQUIRE_KIND,
};

pub const BROWSER_MEDIA_PROFILE: &str = "browser/human-media@1";
pub const BROWSER_MEDIA_ARTIFACT: &str = "conduit-browser-runtime/human-media@1";

pub fn browser_media_acquisition_offers() -> Vec<CapabilityOffer> {
    vec![
        acquisition_offer(CAMERA_ACQUIRE_KIND),
        acquisition_offer(MICROPHONE_ACQUIRE_KIND),
    ]
}

/// Camera source made available only with post-acquisition resource truth.
pub fn acquired_camera_source_offer() -> CapabilityOffer {
    let operation = HostOperationContractId::from(MEDIA_USE_OPERATION);
    realization_offer(
        conduit_semantic_catalog::camera_source_semantic_contract(),
        "browser/acquired-camera-source@1",
        "browser/acquired-camera-source@1",
        vec![HostOperationRequirement {
            contract_id: operation.clone(),
            target_kind: Some(kind_id(CAMERA_FRAME_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: MAXIMUM_MEDIA_VALUE_BYTES,
        }],
        vec![resource_requirement(CAMERA_RESOURCE_CLASS, 1)],
        vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MEDIA_USE_AUTHORITY),
            host_operation_contract_id: operation,
            subject_kind: kind_id(CAMERA_FRAME_KIND),
        }],
    )
}

pub fn browser_camera_frame_sink_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::camera_frame_sink_semantic_contract(),
        Back {
            capability_id: CapabilityId::from("browser/camera-frame-sink@1"),
            execution_profile_id: ExecutionProfileId::from(
                "conduit.std/camera-frame-sink-kernel@1",
            ),
            implementation_id: ImplementationId::from("std/kernel-camera-frame-sink@1"),
            artifact_id: ArtifactId::from(BROWSER_MEDIA_ARTIFACT),
            host_operations: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn acquisition_offer(kind: &str) -> CapabilityOffer {
    let operation = HostOperationContractId::from(MEDIA_ACQUIRE_OPERATION);
    realization_offer(
        conduit_semantic_catalog::media_acquisition_semantic_contract(kind)
            .expect("browser offers only registered human-media acquisition Kinds"),
        &format!("browser/{kind}-capability"),
        &format!("browser/{kind}"),
        vec![HostOperationRequirement {
            contract_id: operation.clone(),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_MEDIA_REQUEST_BYTES,
            maximum_output_bytes: MAXIMUM_MEDIA_RESULT_BYTES,
        }],
        vec![],
        vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(MEDIA_REQUEST_AUTHORITY),
            host_operation_contract_id: operation,
            subject_kind: kind_id(kind),
        }],
    )
}

fn realization_offer(
    contract: Kind,
    capability: &str,
    implementation: &str,
    host_operations: Vec<HostOperationRequirement>,
    resource_requirements: Vec<conduit_core::ResourceRequirement>,
    authority_requirements: Vec<AuthorityRequirement>,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(BROWSER_MEDIA_PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(BROWSER_MEDIA_ARTIFACT),
            host_operations,
            resource_requirements,
            authority_requirements,
        },
    )
    .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_exact_semantics(offer: &CapabilityOffer, contract: &Kind) {
        assert_eq!(offer.startup_parameters, contract.startup_parameters);
        assert_eq!(offer.shorthand, contract.shorthand);
        assert_eq!(offer.kind_id, contract.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            contract.kind_contract_revision
        );
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert_eq!(offer.limits, contract.limits);
    }

    #[test]
    fn acquisition_ports_and_all_limits_are_exact_and_finite() {
        for offer in browser_media_acquisition_offers() {
            let contract = conduit_semantic_catalog::media_acquisition_semantic_contract(
                offer.kind_id.as_str(),
            )
            .unwrap();
            assert_exact_semantics(&offer, &contract);
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
        assert_exact_semantics(
            &acquired_camera_source_offer(),
            &conduit_semantic_catalog::camera_source_semantic_contract(),
        );
        assert_exact_semantics(
            &browser_camera_frame_sink_offer(),
            &conduit_semantic_catalog::camera_frame_sink_semantic_contract(),
        );
    }
}
