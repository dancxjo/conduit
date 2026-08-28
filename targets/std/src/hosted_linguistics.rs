//! Finite std-host offers for portable linguistic contracts.

use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const LINGUISTICS_PROFILE: &str = "std/linguistics-kernel-hosted@1";
pub const LINGUISTICS_ARTIFACT: &str = "conduit-std-host/linguistics@1";
pub const LINGUISTICS_HOST_OPERATION: &str = "conduit.host/linguistics@1";

pub fn linguistics_std_offers() -> Vec<CapabilityOffer> {
    vec![
        offer(
            conduit_language::tokenize_four_definition(),
            vec![FaceStartupParameter {
                name: "text".into(),
                value_type: "Text".into(),
                has_default: false,
            }],
        ),
        offer(conduit_language::annotate_four_definition(), vec![]),
    ]
}

fn offer(
    definition: conduit_form::KindDefinition,
    startup_parameters: Vec<FaceStartupParameter>,
) -> CapabilityOffer {
    let kind = definition.kind_id.as_str();
    CapabilityOffer {
        startup_parameters,
        shorthand: None,
        capability_id: CapabilityId::from(format!("std/{kind}@1")),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(LINGUISTICS_PROFILE),
            implementation_id: ImplementationId::from(format!("std/{kind}@1")),
            artifact_id: ArtifactId::from(LINGUISTICS_ARTIFACT),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(LINGUISTICS_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 4,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}
