//! Typed bounded observation evidence sink; not scan, attachment or renewal authority.
use crate::MAXIMUM_SCAN_OUTPUT_BYTES;
use conduit_core::*;

pub const NETWORK_SCAN_SIGN_OPERATION: &str = "network/scan-sign";
pub const NETWORK_SCAN_SIGN_HOST_CALL: &str = "conduit.host/network-scan-sign@1";
pub const NETWORK_SCAN_SIGN_EXECUTION_PROFILE: &str = "conduit.network/scan-sign@1";

pub fn network_scan_sign_contract_ref() -> KindDefinitionRef<'static> {
    const INPUT: &[PortDescriptorRef<'static>] = &[PortDescriptorRef {
        port_id: "observations",
        value_kind: "network/scan-observations",
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }];
    const LIMITS: CapabilityLimits = CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: MAXIMUM_SCAN_OUTPUT_BYTES,
    };
    KindDefinitionRef {
        startup_parameters: &[],
        shorthand: None,
        kind_id: NETWORK_SCAN_SIGN_OPERATION,
        kind_contract_revision: "conduit.network/scan-sign@1",
        inputs: PortDescriptorRefs::Borrowed(INPUT),
        outputs: PortDescriptorRefs::Borrowed(&[]),
        configuration: &[],
        semantic_laws: &[],
        limits: &LIMITS,
    }
}

pub fn network_scan_sign_contract() -> Kind {
    network_scan_sign_contract_ref().materialize()
}

/// Pure evidence delivery only. Root separately retains/commits mandatory
/// execution evidence; a successful callback grants neither another scan nor
/// profile renewal, radio possession, Body membership or recorder custody.
pub fn network_scan_sign_offer(
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        network_scan_sign_contract(),
        Back {
            capability_id,
            implementation_id,
            artifact_id,
            execution_profile_id: NETWORK_SCAN_SIGN_EXECUTION_PROFILE.into(),
            host_calls: alloc::vec![HostCallRequirement {
                contract_id: NETWORK_SCAN_SIGN_HOST_CALL.into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_SCAN_OUTPUT_BYTES,
                maximum_output_bytes: 0,
            }],
            resource_requirements: alloc::vec![],
            authority_requirements: alloc::vec![],
        },
    )
    .build()
}
