#![cfg(feature = "host-call")]
use conduit_core::*;
use conduit_pulse_capture::{installation::*, owner::*, *};
struct Provider;
impl PulseReadProvider for Provider {
    async fn read(&mut self, _: CheckedPulseRead) -> Result<PulseReadCapture, PulseReadRefusal> {
        panic!("advertisement must not perform electrical work")
    }
}
fn identity() -> PulseNativeIdentity {
    PulseNativeIdentity {
        host_id: "fixture/native".into(),
        boot_id: "fixture/boot".into(),
        base_id: "fixture/base".into(),
        provider_instance_id: "fixture/provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/resource".into(),
        resource_generation_id: ResourceGenerationId("fixture/attachment/1".into()),
        envelope_id: "fixture/envelope".into(),
        artifact_id: "fixture/artifact".into(),
        implementation_id: "fixture/pulse@1".into(),
        base_implementation_id: "fixture/base@1".into(),
    }
}
fn attachment() -> PulseAttachment {
    PulseAttachment {
        generation: 1,
        resource_bytes: 64,
        limits: PulseReadLimits {
            maximum_hold_low_us: 50_000,
            maximum_skipped_pulses: 4,
            minimum_sample_offset_us: 10,
            maximum_sample_offset_us: 200,
            maximum_octets: 8,
            maximum_capture_timeout_us: 50_000,
        },
    }
}
fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity().host_id,
        boot_id: identity().boot_id,
        offer_generation: OfferGeneration(1),
        profile: "fixture/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn ready() -> ReadyPulseBase<Provider> {
    // SAFETY: inert fixture owns no hardware and cannot perform electrical effects.
    unsafe { ReadyPulseBase::new(identity(), attachment(), Provider) }.unwrap()
}
#[test]
fn ready_provider_projects_exact_contract_resource_and_authority_without_effects() {
    let ready = ready();
    let contract = PulseReadContract::prepare().unwrap();
    let mut ad = advertisement();
    ready.append_to_advertisement(&mut ad, &contract).unwrap();
    assert_eq!(ad.bases.len(), 1);
    assert_eq!(
        ad.bases[0].provider_instance_id,
        identity().provider_instance_id
    );
    assert_eq!(ad.resources.len(), 1);
    assert_eq!(ad.resources[0].pool_id, identity().resource_pool_id);
    let [offer] = ad.capabilities.as_slice() else {
        panic!("one provider")
    };
    assert_eq!(offer.semantic_contract, contract.kind().semantic_contract());
    assert_eq!(offer.inputs, contract.kind().inputs);
    assert_eq!(offer.outputs, contract.kind().outputs);
    assert_eq!(offer.host_calls[0].contract_id.as_str(), PULSE_READ_CALL);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        PULSE_AUTHORITY
    );
    assert_eq!(
        offer.implementation.implementation_id,
        identity().implementation_id
    );
    let before = ad.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut ad, &contract),
        Err(PulseInstallationRefusal::Advertisement)
    );
    assert_eq!(ad, before);
}
#[test]
fn wrong_boot_and_invalid_native_generations_do_not_publish_availability() {
    let ready = ready();
    let mut ad = advertisement();
    ad.boot_id = "fixture/stale".into();
    let before = ad.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut ad, &PulseReadContract::prepare().unwrap()),
        Err(PulseInstallationRefusal::WrongBoot)
    );
    assert_eq!(ad, before);
    let mut bad = identity();
    bad.provider_generation = 0;
    // SAFETY: inert fixture owns no hardware.
    assert!(matches!(
        unsafe { ReadyPulseBase::new(bad, attachment(), Provider) },
        Err(PulseInstallationRefusal::InvalidIdentity)
    ));
    // SAFETY: inert fixture owns no hardware.
    assert!(matches!(
        unsafe {
            ReadyPulseBase::new(
                identity(),
                PulseAttachment {
                    generation: 0,
                    ..attachment()
                },
                Provider,
            )
        },
        Err(PulseInstallationRefusal::InvalidIdentity)
    ));
}
