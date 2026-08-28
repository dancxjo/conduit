use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, ImplementationId, ImplementationOffer,
    KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};

pub fn recurrence_proof_offer() -> CapabilityOffer {
    let result = conduit_std_catalog::recurrence_result_type();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "request".into(),
            value_type: conduit_std_catalog::RECURRENCE_REQUEST_TYPE.into(),
            has_default: false,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("proof/time-expand-recurrence"),
        kind_id: kind_id(conduit_std_catalog::RECURRENCE_KIND),
        kind_contract_revision: KindContractRevision::from(
            conduit_std_catalog::RECURRENCE_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("proof/recurrence-kernel@1"),
            implementation_id: ImplementationId::from("proof/kernel-expand-recurrence@1"),
            artifact_id: ArtifactId::from("proof/expand-recurrence@1"),
        },
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("occurrences"),
            value_kind: result.profile().unwrap().value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
                * usize::from(conduit_std_catalog::RECURRENCE_MAXIMUM_RESULTS))
                as u32,
        },
    }
}
