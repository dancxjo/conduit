//! CPU ownership fixtures, never evidence of a real initialized radio.
use conduit_core::*;
use conduit_net::*;
use conduit_network_join_call::{installation::*, *};
struct Provider;
impl NetworkJoinProvider for Provider {
    async fn join(
        &mut self,
        _: NetworkJoinRequest<'_>,
    ) -> Result<AttachmentRecord, StationAttemptFailure> {
        panic!("projection must not perform network effects")
    }
    async fn cleanup(&mut self) -> Result<(), StationFailure> {
        panic!("projection must not perform cleanup effects")
    }
}
fn identity() -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: "fixture/host".into(),
        boot: "fixture/boot".into(),
        base: "fixture/base".into(),
        instance: "fixture/instance".into(),
        provider_generation: 1,
        resource: "fixture/resource".into(),
        resource_generation: ResourceGenerationId("fixture/generation".into()),
        envelope: "fixture/envelope".into(),
        artifact: "fixture/artifact".into(),
        implementation: "fixture/network".into(),
        base_implementation: "fixture/base-implementation".into(),
    }
}
fn ready() -> ReadyNetworkJoinBase<Provider> {
    // SAFETY: inert CPU fixture owns no hardware and cannot perform effects.
    unsafe { ReadyNetworkJoinBase::new(identity(), 9, Provider) }.unwrap()
}
fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity().host,
        boot_id: identity().boot,
        offer_generation: OfferGeneration(1),
        profile: "fixture/profile".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[test]
fn retained_provider_projects_exact_semantics_native_identity_and_authority_subject() {
    let ready = ready();
    let mut ad = advertisement();
    ready.append_to_advertisement(&mut ad).unwrap();
    assert_eq!(ad.bases.len(), 1);
    assert_eq!(ad.bases[0].provider_instance_id, identity().instance);
    assert_eq!(ad.resources.len(), 1);
    assert_eq!(ad.resources[0].pool_id, identity().resource);
    assert_eq!(
        ad.resources[0].class_id.as_str(),
        WIFI_STATION_RESOURCE_CLASS
    );
    let [offer] = ad.capabilities.as_slice() else {
        panic!("one retained implementation");
    };
    assert_eq!(offer.capability_id.as_str(), JOIN_CAPABILITY);
    assert_eq!(
        offer.implementation.execution_profile_id.as_str(),
        NETWORK_JOIN_EXECUTION_PROFILE
    );
    assert_eq!(
        offer.semantic_contract,
        PreparedJoinContract::prepare().kind().semantic_contract()
    );
    assert_eq!(
        offer.implementation.implementation_id,
        identity().implementation
    );
    assert_eq!(
        offer.host_calls[0].target_kind.as_ref().unwrap().as_str(),
        NETWORK_CONFIG_SUBJECT
    );
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        NETWORK_CONFIG_AUTHORITY
    );
    let before = ad.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut ad),
        Err(NetworkInstallationRefusal::Advertisement)
    );
    assert_eq!(ad, before);
}
#[test]
fn stale_boot_and_invalid_native_generations_never_project_availability() {
    let ready = ready();
    let mut ad = advertisement();
    ad.boot_id = "boot/old".into();
    let before = ad.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut ad),
        Err(NetworkInstallationRefusal::WrongBoot)
    );
    assert_eq!(ad, before);
    for attachment in [0, 9] {
        let mut i = identity();
        if attachment == 9 {
            i.provider_generation = 0;
        }
        // SAFETY: inert CPU fixture.
        assert!(matches!(
            unsafe { ReadyNetworkJoinBase::new(i, attachment, Provider) },
            Err(NetworkInstallationRefusal::InvalidIdentity)
        ));
    }
}
