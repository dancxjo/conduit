use conduit_core::{
    ArtifactId, AuthorityContractId, AuthorityRequirement, CapabilityId, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, resource_requirement,
};

pub const IMPLEMENTATION: &str = "conduitos/kernel-http-client-http1-literal@1";
pub const PROFILE: &str = "conduitos/http1-literal-plain-fixed@1";
pub const ARTIFACT: &str = "conduitos/native-http1-fixed@1";
pub const HOST_OPERATION: &str = "conduit.host/http-client-exchange@1";
pub const RESOURCE_CLASS: &str = "conduit.resource/network/http-client@1";
pub const AUTHORITY: &str = "conduit.authority/http-outbound@1";
pub const NETWORK_BASE: &str = "network/ipv4-tcp";
pub const NETWORK_DRIVER: &str = "conduitos/deterministic-ipv4-tcp@1";
pub const FACILITY: &str = "network/http1-literal-client@1";
pub const PACKET_BUFFERS: u16 = 4;
pub const SOCKET_SLOTS: u16 = 1;
pub const TIMER_SLOTS: u16 = 2;
pub const SIGN_ITEMS: u16 = 32;
pub const REQUEST_BYTES: usize = conduit_std_catalog::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES as usize;
pub const RESPONSE_BYTES: usize = conduit_std_catalog::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES as usize;

pub fn offer() -> CapabilityOffer {
    let contract = conduit_std_catalog::http_client_contract();
    let request_kind = conduit_std_catalog::http_request_type()
        .profile()
        .expect("finite HTTP request profile")
        .value_kind()
        .clone();
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(HOST_OPERATION),
        target_kind: Some(request_kind.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: REQUEST_BYTES as u32,
        maximum_output_bytes: RESPONSE_BYTES as u32,
    };
    CapabilityOffer {
        startup_parameters: alloc::vec::Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-http-client-http1-literal"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_std_catalog::HTTP_CLIENT_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        host_operations: alloc::vec![operation.clone()],
        resource_requirements: alloc::vec![resource_requirement(RESOURCE_CLASS, 1)],
        authority_requirements: alloc::vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(AUTHORITY),
            host_operation_contract_id: operation.contract_id,
            subject_kind: request_kind,
        }],
        limits: contract.limits,
    }
}
