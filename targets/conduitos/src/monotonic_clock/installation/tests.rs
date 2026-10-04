use super::*;
use crate::monotonic_clock::codec::ClockDisposition;
struct Provider;
impl MonotonicDeadlineProvider for Provider {
    fn poll_until(&mut self, _: u64) -> Result<Option<u64>, ClockDisposition> {
        panic!("advertisement must not observe time or arm a timer")
    }
    fn revoke(&mut self) {}
}
fn identity() -> ClockNativeIdentity {
    ClockNativeIdentity {
        host_id: HostId::from("host/native"),
        boot_id: BootId::from("boot/native"),
        base_id: HostBaseId::from("base/native-clock"),
        provider_instance_id: BaseInstanceId::from("provider/native-clock"),
        provider_generation: 2,
        resource_pool_id: ResourcePoolId::from("resource/native-clock"),
        resource_generation_id: ResourceGenerationId("attachment-generation/3".into()),
        envelope_id: CapabilityEnvelopeId::from("envelope/native-clock"),
        artifact_id: ArtifactId::from("conduitos-build/reviewed"),
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
    let ready = unsafe { ReadyClockBase::new(identity(), Provider) }.unwrap();
    let contract = MonotonicClockContract::prepare().unwrap();
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
    assert_eq!(offer.host_calls[0].contract_id.as_str(), CLOCK_CALL);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        CLOCK_AUTHORITY
    );
    assert_eq!(
        offer.implementation.implementation_id.as_str(),
        CLOCK_IMPLEMENTATION
    );
    let before = advertised.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut advertised, &contract),
        Err(ClockInstallationRefusal::Advertisement)
    );
    assert_eq!(advertised, before);
}
#[test]
fn stale_boot_and_invalid_native_generations_are_not_advertised() {
    let mut invalid = identity();
    invalid.provider_generation = 0;
    // SAFETY: this inert test provider has no physical resources or effects.
    assert!(matches!(
        unsafe { ReadyClockBase::new(invalid, Provider) },
        Err(ClockInstallationRefusal::InvalidIdentity)
    ));
    // SAFETY: this inert test provider has no physical resources or effects.
    let ready = unsafe { ReadyClockBase::new(identity(), Provider) }.unwrap();
    let mut advertised = advertisement();
    advertised.boot_id = BootId::from("boot/stale");
    let before = advertised.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut advertised, &MonotonicClockContract::prepare().unwrap()),
        Err(ClockInstallationRefusal::WrongBoot)
    );
    assert_eq!(advertised, before);
}
