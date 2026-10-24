use conduit_core::{
    ArtifactId, AuthorityContractId, AuthorityRequirement, Back, BackOfferBuilder, CapabilityId,
    CapabilityOffer, ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId,
    resource_requirement,
};

pub const IMPLEMENTATION: &str = "conduitos/kernel-http-client-http1-literal";
pub const PROFILE: &str = "conduitos/http1-literal-plain-fixed";
pub const ARTIFACT: &str = "conduitos/native-http1-fixed";
pub const HOST_CALL: &str = "conduit.host/http-client-exchange";
pub const RESOURCE_CLASS: &str = "conduit.resource/network/http-client";
pub const AUTHORITY: &str = "conduit.authority/http-outbound";
pub const NETWORK_BASE: &str = "network/ipv4-tcp";
pub const NETWORK_DRIVER: &str = "conduitos/deterministic-ipv4-tcp@1";
pub const FACILITY: &str = "network/http1-literal-client";
pub const PACKET_BUFFERS: u16 = 4;
pub const SOCKET_SLOTS: u16 = 1;
pub const TIMER_SLOTS: u16 = 2;
pub const SIGN_ITEMS: u16 = 32;
pub const REQUEST_BYTES: usize = conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES as usize;
pub const RESPONSE_BYTES: usize = conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES as usize;

pub fn offer() -> CapabilityOffer {
    let contract = conduit_web::http_client_semantics().into_semantic_contract();
    let request_kind = conduit_web::http_request_type()
        .profile()
        .expect("finite HTTP request profile")
        .value_kind()
        .clone();
    let operation = HostCallRequirement {
        contract_id: HostCallContractId::from(HOST_CALL),
        target_kind: Some(request_kind.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: REQUEST_BYTES as u32,
        maximum_output_bytes: RESPONSE_BYTES as u32,
    };
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("conduitos-http-client-http1-literal"),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: alloc::vec![operation.clone()],
            resource_requirements: alloc::vec![resource_requirement(RESOURCE_CLASS, 1)],
            authority_requirements: alloc::vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(AUTHORITY),
                host_call_contract_id: operation.contract_id,
                subject_kind: request_kind,
            }],
        },
    )
    .build()
}
