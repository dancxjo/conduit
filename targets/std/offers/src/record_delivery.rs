//! Std Host realization of reusable correlated delivery-status projection.

use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision,
};

pub const RECORD_DELIVERY_STATUS_STD_IMPLEMENTATION: &str = "std/record-delivery-status@1";
pub const RECORD_DELIVERY_STATUS_HOST_OPERATION: &str = "conduit.host/record-delivery-status@1";

pub fn record_delivery_status_std_offer() -> CapabilityOffer {
    let definition = conduit_net::record_delivery_status_kind_definition();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(RECORD_DELIVERY_STATUS_STD_IMPLEMENTATION),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_net::RECORD_DELIVERY_STATUS_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("std/record-delivery-status@1"),
            implementation_id: ImplementationId::from(RECORD_DELIVERY_STATUS_STD_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-net/record-delivery-status@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(RECORD_DELIVERY_STATUS_HOST_OPERATION),
            target_kind: Some(definition.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32,
            maximum_output_bytes: conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: (conduit_net::MAXIMUM_RECORD_DELIVERY_CANONICAL_BYTES * 2) as u32,
        },
    }
}
