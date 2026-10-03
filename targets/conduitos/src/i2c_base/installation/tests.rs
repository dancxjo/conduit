use super::*;
use crate::i2c_base::{I2cDisposition, I2cTransaction};
struct Provider;
impl I2cProvider for Provider {
    fn transact(&mut self, _: &I2cTransaction<'_>, _: &mut [u8]) -> Result<usize, I2cDisposition> {
        panic!("advertisement must not issue bus traffic")
    }
    fn revoke(&mut self) {}
}
fn identity() -> I2cNativeIdentity {
    I2cNativeIdentity {
        host_id: HostId::from("host/native"),
        boot_id: BootId::from("boot/native"),
        base_id: HostBaseId::from("base/native-i2c"),
        provider_instance_id: BaseInstanceId::from("provider/native-i2c"),
        provider_generation: 2,
        resource_pool_id: ResourcePoolId::from("resource/native-i2c"),
        resource_generation_id: ResourceGenerationId("attachment-generation/3".into()),
        envelope_id: CapabilityEnvelopeId::from("envelope/native-i2c"),
        artifact_id: ArtifactId::from("conduitos-build/reviewed"),
    }
}
fn attachment() -> I2cAttachment {
    I2cAttachment {
        generation: 3,
        minimum_address: 8,
        maximum_address: 119,
        resource_bytes: 32,
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
    let ready = unsafe { ReadyI2cBase::new(identity(), attachment(), Provider) }.unwrap();
    let contract = I2cContract::prepare().unwrap();
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
    assert_eq!(offer.host_calls[0].contract_id.as_str(), I2C_CALL);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        I2C_AUTHORITY
    );
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        I2C_IMPLEMENTATION
    );
    let before = advertised.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut advertised, &contract),
        Err(I2cInstallationRefusal::Advertisement)
    );
    assert_eq!(advertised, before);
}
#[test]
fn stale_boot_and_invalid_native_generations_are_not_advertised() {
    let mut invalid = identity();
    invalid.provider_generation = 0;
    // SAFETY: this inert test provider has no physical resources or effects.
    assert!(matches!(
        unsafe { ReadyI2cBase::new(invalid, attachment(), Provider) },
        Err(I2cInstallationRefusal::InvalidIdentity)
    ));
    // SAFETY: this inert test provider has no physical resources or effects.
    let ready = unsafe { ReadyI2cBase::new(identity(), attachment(), Provider) }.unwrap();
    let mut advertised = advertisement();
    advertised.boot_id = BootId::from("boot/stale");
    let before = advertised.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut advertised, &I2cContract::prepare().unwrap()),
        Err(I2cInstallationRefusal::WrongBoot)
    );
    assert_eq!(advertised, before);
}
