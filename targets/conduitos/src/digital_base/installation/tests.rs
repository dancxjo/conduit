use super::*;
use crate::digital_base::{DigitalDisposition, DigitalRequest};
struct Provider;
impl DigitalProvider for Provider {
    fn access(&mut self, _: DigitalRequest) -> Result<bool, DigitalDisposition> {
        panic!("advertisement cannot access a contact")
    }
    fn revoke(&mut self) {}
}
fn identity() -> DigitalNativeIdentity {
    DigitalNativeIdentity {
        capability_id: CapabilityId::from("capability/contact-one"),
        host_id: HostId::from("host/native"),
        boot_id: BootId::from("boot/native"),
        base_id: HostBaseId::from("base/native-digital"),
        provider_instance_id: BaseInstanceId::from("provider/native-digital"),
        provider_generation: 2,
        resource_pool_id: ResourcePoolId::from("resource/native-digital"),
        resource_generation_id: ResourceGenerationId("attachment-generation/3".into()),
        envelope_id: CapabilityEnvelopeId::from("envelope/native-digital"),
        artifact_id: ArtifactId::from("conduitos-build/reviewed"),
    }
}
fn attachment() -> DigitalAttachment {
    DigitalAttachment {
        generation: 3,
        resource_bytes: 1,
    }
}
fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity().host_id,
        boot_id: identity().boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/native@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[test]
fn native_ready_provider_offers_exact_source_contract_and_requires_authority() {
    // SAFETY: this inert test provider has no physical resources or effects.
    let ready = unsafe { ReadyDigitalBase::new(identity(), attachment(), Provider) }.unwrap();
    let contract = DigitalContract::prepare().unwrap();
    let mut advertised = advertisement();
    ready
        .append_to_advertisement(&mut advertised, &contract)
        .unwrap();
    assert_eq!(advertised.bases.len(), 1);
    assert_eq!(
        advertised.bases[0].provider_instance_id,
        identity().provider_instance_id
    );
    assert_eq!(advertised.bases[0].provider_generation, 2);
    assert_eq!(advertised.resources.len(), 1);
    assert_eq!(advertised.resources[0].pool_id, identity().resource_pool_id);
    let [offer] = advertised.capabilities.as_slice() else {
        panic!("one ready provider")
    };
    assert_eq!(offer.semantic_contract, contract.kind().semantic_contract());
    assert_eq!(offer.inputs, contract.kind().inputs);
    assert_eq!(offer.outputs, contract.kind().outputs);
    assert_eq!(offer.host_calls[0].contract_id.as_str(), DIGITAL_CALL);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        DIGITAL_AUTHORITY
    );
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        DIGITAL_IMPLEMENTATION
    );
    let before = advertised.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut advertised, &contract),
        Err(DigitalInstallationRefusal::Advertisement)
    );
    assert_eq!(advertised, before);
}
#[test]
fn stale_boot_and_invalid_native_generations_are_not_advertised() {
    let mut invalid = identity();
    invalid.provider_generation = 0;
    // SAFETY: this inert test provider has no physical resources or effects.
    assert!(matches!(
        unsafe { ReadyDigitalBase::new(invalid, attachment(), Provider) },
        Err(DigitalInstallationRefusal::InvalidIdentity)
    ));
    // SAFETY: this inert test provider has no physical resources or effects.
    let ready = unsafe { ReadyDigitalBase::new(identity(), attachment(), Provider) }.unwrap();
    let mut advertised = advertisement();
    advertised.boot_id = BootId::from("boot/stale");
    let before = advertised.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut advertised, &DigitalContract::prepare().unwrap()),
        Err(DigitalInstallationRefusal::WrongBoot)
    );
    assert_eq!(advertised, before);
}

#[test]
fn independent_contacts_retain_distinct_capabilities_and_resources() {
    let contract = DigitalContract::prepare().unwrap();
    // SAFETY: both providers are inert and panic on any contact access.
    let first = unsafe { ReadyDigitalBase::new(identity(), attachment(), Provider) }.unwrap();
    let mut second_identity = identity();
    second_identity.capability_id = CapabilityId::from("capability/contact-two");
    second_identity.base_id = HostBaseId::from("base/contact-two");
    second_identity.provider_instance_id = BaseInstanceId::from("provider/contact-two");
    second_identity.resource_pool_id = ResourcePoolId::from("resource/contact-two");
    second_identity.envelope_id = CapabilityEnvelopeId::from("envelope/contact-two");
    // SAFETY: independently identified inert fixture; no hardware exists.
    let second = unsafe { ReadyDigitalBase::new(second_identity, attachment(), Provider) }.unwrap();
    let mut host = advertisement();
    first.append_to_advertisement(&mut host, &contract).unwrap();
    second
        .append_to_advertisement(&mut host, &contract)
        .unwrap();
    assert_eq!(host.bases.len(), 2);
    assert_eq!(host.capabilities.len(), 2);
    assert_eq!(host.resources.len(), 2);
    assert_ne!(
        host.capabilities[0].capability_id,
        host.capabilities[1].capability_id
    );
    assert_ne!(host.resources[0].pool_id, host.resources[1].pool_id);
}

#[test]
fn duplicate_custom_capability_id_refuses_without_advertisement_mutation() {
    let contract = DigitalContract::prepare().unwrap();
    // SAFETY: all providers in this test are inert.
    let first = unsafe { ReadyDigitalBase::new(identity(), attachment(), Provider) }.unwrap();
    let mut other = identity();
    other.base_id = HostBaseId::from("base/other");
    other.provider_instance_id = BaseInstanceId::from("provider/other");
    other.resource_pool_id = ResourcePoolId::from("resource/other");
    other.envelope_id = CapabilityEnvelopeId::from("envelope/other");
    // SAFETY: distinct inert provider with a deliberately conflicting capability.
    let second = unsafe { ReadyDigitalBase::new(other, attachment(), Provider) }.unwrap();
    let mut host = advertisement();
    first.append_to_advertisement(&mut host, &contract).unwrap();
    let before = host.clone();
    assert_eq!(
        second.append_to_advertisement(&mut host, &contract),
        Err(DigitalInstallationRefusal::Advertisement)
    );
    assert_eq!(host, before);
}

#[test]
fn invalid_ready_identity_revokes_the_supplied_physical_provider() {
    use alloc::rc::Rc;
    use core::cell::Cell;
    struct Tracked(Rc<Cell<bool>>);
    impl DigitalProvider for Tracked {
        fn access(&mut self, _: DigitalRequest) -> Result<bool, DigitalDisposition> {
            panic!("invalid preparation cannot drive");
        }
        fn revoke(&mut self) {
            self.0.set(true);
        }
    }
    let revoked = Rc::new(Cell::new(false));
    let mut invalid = identity();
    invalid.provider_generation = 0;
    // SAFETY: inert tracked provider has no physical resources; identity is deliberately invalid.
    assert!(
        unsafe { ReadyDigitalBase::new(invalid, attachment(), Tracked(revoked.clone())) }.is_err()
    );
    assert!(revoked.get());
}
