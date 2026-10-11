//! Explicit deterministic test Root; it initializes or claims no physical USB.
use conduit_core::*;
use conduitos::usb_base::{
    control_contract::*, control_factory::*, endpoint_read_contract::*, endpoint_read_factory::*,
    endpoint_write_contract::*, endpoint_write_factory::*,
};
pub(super) fn host() -> (HostAdvertisement, Vec<AuthorityGrant>) {
    let control = ControlContract::prepare().unwrap();
    let read = EndpointReadContract::prepare().unwrap();
    let write = EndpointWriteContract::prepare().unwrap();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/cdc-host".into(),
        boot_id: "fixture/cdc-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/cdc-kernel-only".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut registry = BaseRegistry::new(BaseRegistryLimits {
        maximum_bases: 3,
        maximum_capabilities_per_base: 1,
        maximum_resources_per_base: 1,
        maximum_advertised_capabilities: 3,
        maximum_advertised_resources: 3,
    })
    .unwrap();
    let offers = [
        (
            conduitos::usb_base::control_offer::offer(&control, "fixture/control-artifact".into()),
            CONTROL_BASE,
            CONTROL_ATTACHMENT,
            CONTROL_AUTHORITY,
            CONTROL_CALL,
        ),
        (
            conduitos::usb_base::endpoint_read_offer::offer(&read, "fixture/read-artifact".into()),
            ENDPOINT_READ_BASE,
            ENDPOINT_READ_ATTACHMENT,
            ENDPOINT_READ_AUTHORITY,
            ENDPOINT_READ_CALL,
        ),
        (
            conduitos::usb_base::endpoint_write_offer::offer(
                &write,
                "fixture/write-artifact".into(),
            ),
            ENDPOINT_WRITE_BASE,
            ENDPOINT_WRITE_ATTACHMENT,
            ENDPOINT_WRITE_AUTHORITY,
            ENDPOINT_WRITE_CALL,
        ),
    ];
    let mut grants = vec![];
    for (index, (offer, implementation, attachment, authority, call)) in
        offers.into_iter().enumerate()
    {
        grants.push(AuthorityGrant {
            grant_id: format!("fixture/grant/{index}").into(),
            contract_id: authority.into(),
            host_call_contract_id: call.into(),
            subject_kind: offer.kind_id.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            capability_id: offer.capability_id.clone(),
        });
        registry
            .register(BaseProviderEntry {
                base_id: format!("fixture/base/{index}").into(),
                provider_instance_id: format!("fixture/provider/{index}").into(),
                provider_generation: 1,
                implementation_id: implementation.into(),
                mechanism_family: attachment.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    &format!("fixture/pool/{index}"),
                    attachment,
                    1,
                )],
            })
            .unwrap();
    }
    registry.project_ready_into(&mut host).unwrap();
    (host, grants)
}
