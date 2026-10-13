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
        host_id: "fixture/bot-storage-host".into(),
        boot_id: "fixture/bot-storage-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/bot-storage-kernel-only".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut registry = BaseRegistry::new(BaseRegistryLimits {
        maximum_bases: 5,
        maximum_capabilities_per_base: 2,
        maximum_resources_per_base: 1,
        maximum_advertised_capabilities: 7,
        maximum_advertised_resources: 5,
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
    use conduitos::storage_base::{
        contract::BLOCK_READ_AUTHORITY, transaction_contract::*, transaction_factory::*,
    };
    let contract = BlockReadTransactionContract::prepare().unwrap();
    for pair in 0..2 {
        let mut capabilities = vec![];
        for (role, call) in [
            (StorageTransactionRole::Authorize, AUTHORIZE_CALL),
            (StorageTransactionRole::Settle, SETTLE_CALL),
        ] {
            let mut capability = offer(
                role,
                &contract,
                format!("fixture/storage-artifact/{pair}").into(),
            );
            capability.capability_id =
                format!("fixture/storage/{pair}/{}", role.implementation()).into();
            grants.push(AuthorityGrant {
                grant_id: format!("fixture/storage-grant/{pair}/{call}").into(),
                contract_id: BLOCK_READ_AUTHORITY.into(),
                host_call_contract_id: call.into(),
                subject_kind: capability.kind_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                capability_id: capability.capability_id.clone(),
            });
            capabilities.push(capability);
        }
        registry
            .register(BaseProviderEntry {
                base_id: format!("fixture/storage-base/{pair}").into(),
                provider_instance_id: format!("fixture/storage-provider/{pair}").into(),
                provider_generation: 1,
                implementation_id: TRANSACTION_BASE.into(),
                mechanism_family: TRANSACTION_RESOURCE.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities,
                resources: vec![resource_offer(
                    &format!("fixture/storage-pool/{pair}"),
                    TRANSACTION_RESOURCE,
                    2,
                )],
            })
            .unwrap();
    }
    registry.project_ready_into(&mut host).unwrap();
    (host, grants)
}
