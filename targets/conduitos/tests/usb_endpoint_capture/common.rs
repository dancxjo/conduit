//! Class-neutral advertisement fixture; authority is confined to conformance.
use conduit_core::*;
use conduitos::usb_base::{
    endpoint_read_contract::{ENDPOINT_READ_CALL, EndpointReadContract},
    endpoint_read_factory::{
        ENDPOINT_READ_ATTACHMENT, ENDPOINT_READ_AUTHORITY, ENDPOINT_READ_BASE,
        ENDPOINT_READ_IMPLEMENTATION,
    },
    endpoint_read_proof_plan::EndpointReadProofSubject,
};

pub(super) fn host_and_grants(
    contract: &EndpointReadContract,
    subject: &EndpointReadProofSubject<'_>,
) -> Result<(HostAdvertisement, [AuthorityGrant; 1]), &'static str> {
    let kind = contract.kind();
    let offer = conduitos::usb_base::endpoint_read_offer::offer(
        contract,
        "conduitos/usb-endpoint-read-kernel-proof@1".into(),
    );
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: subject.host_id.into(),
        boot_id: subject.boot_id.into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/usb-endpoint-read-kernel-proof@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut registry = BaseRegistry::new(BaseRegistryLimits {
        maximum_bases: 1,
        maximum_capabilities_per_base: 1,
        maximum_resources_per_base: 1,
        maximum_advertised_capabilities: 1,
        maximum_advertised_resources: 1,
    })
    .map_err(|_| "usb-endpoint-read-proof-planning")?;
    registry
        .register(BaseProviderEntry {
            base_id: subject.controller_base_id.into(),
            provider_instance_id: subject.device_instance_id.into(),
            provider_generation: u64::from(subject.attachment_epoch),
            implementation_id: ENDPOINT_READ_BASE.into(),
            mechanism_family: ENDPOINT_READ_ATTACHMENT.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![resource_offer(
                &format!(
                    "usb-endpoint-read-dma/{}/{}/{}/{}/{}",
                    subject.root_port,
                    subject.slot,
                    subject.attachment_epoch,
                    subject.endpoint_dci,
                    subject.endpoint_epoch
                ),
                ENDPOINT_READ_ATTACHMENT,
                1,
            )],
        })
        .map_err(|_| "usb-endpoint-read-proof-planning")?;
    registry
        .project_ready_into(&mut host)
        .map_err(|_| "usb-endpoint-read-proof-planning")?;
    let grants = [AuthorityGrant {
        grant_id: "conduitos.proof/usb-endpoint-read-explicit-grant@1".into(),
        contract_id: ENDPOINT_READ_AUTHORITY.into(),
        host_call_contract_id: ENDPOINT_READ_CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: ENDPOINT_READ_IMPLEMENTATION.into(),
    }];
    Ok((host, grants))
}
