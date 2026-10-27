use super::*;
use crate::spi_base::{SpiDisposition, SpiTransaction};
struct Provider;
impl SpiProvider for Provider {
    fn exchange(&mut self, _: &SpiTransaction<'_>, _: &mut [u8]) -> Result<(), SpiDisposition> {
        panic!("advertisement must not issue bus traffic")
    }
    fn revoke(&mut self) {}
}
fn identity() -> SpiNativeIdentity {
    SpiNativeIdentity {
        host_id: HostId::from("host/native"),
        boot_id: BootId::from("boot/native"),
        base_id: HostBaseId::from("base/native-spi"),
        provider_instance_id: BaseInstanceId::from("provider/native-spi"),
        provider_generation: 2,
        resource_pool_id: ResourcePoolId::from("resource/native-spi"),
        resource_generation_id: ResourceGenerationId("attachment-generation/3".into()),
        envelope_id: CapabilityEnvelopeId::from("envelope/native-spi"),
        artifact_id: ArtifactId::from("conduitos-build/reviewed"),
    }
}
fn attachment() -> SpiAttachment {
    SpiAttachment {
        generation: 3,
        resource_bytes: 512,
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
    let ready = unsafe { ReadySpiBase::new(identity(), attachment(), Provider) }.unwrap();
    let contract = SpiContract::prepare().unwrap();
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
    assert_eq!(offer.host_calls[0].contract_id.as_str(), SPI_CALL);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        SPI_AUTHORITY
    );
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        SPI_IMPLEMENTATION
    );
    let before = advertised.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut advertised, &contract),
        Err(SpiInstallationRefusal::Advertisement)
    );
    assert_eq!(advertised, before);
}
#[test]
fn stale_boot_and_invalid_native_generations_are_not_advertised() {
    let mut invalid = identity();
    invalid.provider_generation = 0;
    // SAFETY: this inert test provider has no physical resources or effects.
    assert!(matches!(
        unsafe { ReadySpiBase::new(invalid, attachment(), Provider) },
        Err(SpiInstallationRefusal::InvalidIdentity)
    ));
    // SAFETY: this inert test provider has no physical resources or effects.
    let ready = unsafe { ReadySpiBase::new(identity(), attachment(), Provider) }.unwrap();
    let mut advertised = advertisement();
    advertised.boot_id = BootId::from("boot/stale");
    let before = advertised.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut advertised, &SpiContract::prepare().unwrap()),
        Err(SpiInstallationRefusal::WrongBoot)
    );
    assert_eq!(advertised, before);
}
