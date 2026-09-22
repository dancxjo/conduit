use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId,
};

pub const BROWSER_TEXT_UPPER_PROFILE: &str = "browser/text-upper-kernel@1";
pub const BROWSER_TEXT_UPPER_ARTIFACT: &str = "conduit-browser-runtime/text-upper@1";
pub const BROWSER_TEXT_UPPER_IMPLEMENTATION: &str = "browser/text-upper@1";
pub const BROWSER_TEXT_UPPER_CAPABILITY: &str = "browser-text-upper-v1";
pub const BROWSER_TEXT_UPPER_HOST_CALL: &str = "conduit.host/text-upper@1";
pub const BROWSER_TEXT_UPPER_TARGET: &str = "text/uppercase-utf8";

pub fn browser_text_upper_offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_text::text_upper_semantics().into_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(BROWSER_TEXT_UPPER_CAPABILITY),
            execution_profile_id: ExecutionProfileId::from(BROWSER_TEXT_UPPER_PROFILE),
            implementation_id: ImplementationId::from(BROWSER_TEXT_UPPER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(BROWSER_TEXT_UPPER_ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(BROWSER_TEXT_UPPER_HOST_CALL),
                target_kind: Some(kind_id(BROWSER_TEXT_UPPER_TARGET)),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
                maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
