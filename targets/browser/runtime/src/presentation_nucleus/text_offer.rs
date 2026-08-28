use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId,
};

pub const BROWSER_TEXT_UPPER_PROFILE: &str = "browser/text-upper-kernel@1";
pub const BROWSER_TEXT_UPPER_ARTIFACT: &str = "conduit-browser-runtime/text-upper@1";
pub const BROWSER_TEXT_UPPER_IMPLEMENTATION: &str = "browser/text-upper@1";
pub const BROWSER_TEXT_UPPER_CAPABILITY: &str = "browser-text-upper-v1";
pub const BROWSER_TEXT_UPPER_HOST_OPERATION: &str = "conduit.host/text-upper@1";
pub const BROWSER_TEXT_UPPER_TARGET: &str = "text/uppercase-utf8";

pub fn browser_text_upper_offer() -> CapabilityOffer {
    let contract = conduit_text::text_upper_semantics();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: Some((port_id("text"), port_id("text"))),
        capability_id: CapabilityId::from(BROWSER_TEXT_UPPER_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(BROWSER_TEXT_UPPER_PROFILE),
            implementation_id: ImplementationId::from(BROWSER_TEXT_UPPER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(BROWSER_TEXT_UPPER_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(BROWSER_TEXT_UPPER_HOST_OPERATION),
            target_kind: Some(kind_id(BROWSER_TEXT_UPPER_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
            maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}
