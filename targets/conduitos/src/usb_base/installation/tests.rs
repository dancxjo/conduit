use super::*;

fn identity() -> UsbNativeIdentity {
    UsbNativeIdentity {
        host_id: "host/product".into(),
        boot_id: "boot/current".into(),
        base_id: "base/controller".into(),
        provider_instance_id: "provider/device".into(),
        provider_generation: 3,
        resource_pool_id: "resource/dma".into(),
        resource_generation_id: ResourceGenerationId("resource-generation/current".into()),
        envelope_id: "envelope/current".into(),
        artifact_id: "artifact/product".into(),
    }
}
fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/product".into(),
        boot_id: "boot/current".into(),
        offer_generation: OfferGeneration(1),
        profile: "product".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

#[test]
fn projection_preserves_exact_provider_and_artifact_without_authority() {
    let identity = identity();
    let mut host = advertisement();
    identity
        .append_to_advertisement(&mut host, &ControlContract::prepare().unwrap())
        .unwrap();
    assert_eq!(host.bases.len(), 1);
    assert_eq!(host.bases[0].base_id, identity.base_id);
    assert_eq!(
        host.bases[0].provider_instance_id,
        identity.provider_instance_id
    );
    assert_eq!(host.bases[0].provider_generation, 3);
    assert_eq!(host.bases[0].lifecycle, BaseLifecycle::Ready);
    assert_eq!(
        host.bases[0].enforcement_class,
        BaseEnforcementClass::Cooperative
    );
    assert_eq!(host.resources[0].pool_id, identity.resource_pool_id);
    assert_eq!(
        host.capabilities[0].implementation.artifact_id,
        identity.artifact_id
    );
}

#[test]
fn wrong_boot_and_duplicate_refuse_without_mutation() {
    let identity = identity();
    let contract = ControlContract::prepare().unwrap();
    let mut host = advertisement();
    host.boot_id = "boot/other".into();
    let before = host.clone();
    assert_eq!(
        identity.append_to_advertisement(&mut host, &contract),
        Err(UsbInstallationRefusal::WrongBoot)
    );
    assert_eq!(host, before);
    host.boot_id = identity.boot_id.clone();
    identity
        .append_to_advertisement(&mut host, &contract)
        .unwrap();
    let before = host.clone();
    assert_eq!(
        identity.append_to_advertisement(&mut host, &contract),
        Err(UsbInstallationRefusal::Advertisement)
    );
    assert_eq!(host, before);
}

#[test]
fn every_native_identity_field_is_required() {
    let good = identity();
    let variants = [
        UsbNativeIdentity {
            host_id: "".into(),
            ..good.clone()
        },
        UsbNativeIdentity {
            boot_id: "".into(),
            ..good.clone()
        },
        UsbNativeIdentity {
            base_id: "".into(),
            ..good.clone()
        },
        UsbNativeIdentity {
            provider_instance_id: "".into(),
            ..good.clone()
        },
        UsbNativeIdentity {
            provider_generation: 0,
            ..good.clone()
        },
        UsbNativeIdentity {
            resource_pool_id: "".into(),
            ..good.clone()
        },
        UsbNativeIdentity {
            resource_generation_id: ResourceGenerationId("".into()),
            ..good.clone()
        },
        UsbNativeIdentity {
            envelope_id: "".into(),
            ..good.clone()
        },
        UsbNativeIdentity {
            artifact_id: "".into(),
            ..good
        },
    ];
    for invalid in variants {
        assert_eq!(
            invalid.validate(),
            Err(UsbInstallationRefusal::InvalidIdentity)
        );
    }
}
