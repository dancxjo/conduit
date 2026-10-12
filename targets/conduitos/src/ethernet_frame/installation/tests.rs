//! Queue installation admission only; no actual Root/USB/network service proof.
use super::*;
fn identity() -> EthernetFrameNativeIdentity {
    EthernetFrameNativeIdentity {
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        base_id: "base/network/test".into(),
        provider_instance_id: "provider/network/test".into(),
        provider_generation: 7,
        resource_pool_id: "pool/network/test".into(),
        resource_generation_id: ResourceGenerationId("resource/network/7".into()),
        envelope_id: "envelope/network/test".into(),
        artifact_id: "artifact/network/test".into(),
    }
}
fn approval(host: &str, boot: &str) -> EthernetNetworkApproval {
    // SAFETY: explicit test fixture Root approval, no native authority evidence.
    unsafe {
        EthernetNetworkApproval::admit(
            host.into(),
            boot.into(),
            EthernetNetworkScope {
                mac: [2, 0, 0, 0, 0, 1],
                endpoint: crate::virtio_tcp::VirtioTcpEndpoint {
                    guest_address: [10, 0, 2, 15],
                    prefix_length: 24,
                    gateway: [10, 0, 2, 2],
                    remote_address: [10, 0, 2, 2],
                    remote_port: 8080,
                    local_port: 40000,
                },
                maximum_polls: 64,
            },
        )
    }
    .unwrap()
}
#[test]
fn wrong_generation_and_boot_cannot_bind_root_owned_queue() {
    let queues = EthernetFrameQueues::prepare(7, [2, 0, 0, 0, 0, 1]).unwrap();
    let mut stale = identity();
    stale.provider_generation = 8;
    assert!(matches!(
        unsafe { ReadyEthernetFrameBase::new(stale, &queues, approval("host/test", "boot/test")) },
        Err(EthernetFrameNativeRefusal::Identity)
    ));
    assert!(matches!(
        unsafe {
            ReadyEthernetFrameBase::new(identity(), &queues, approval("host/test", "boot/stale"))
        },
        Err(EthernetFrameNativeRefusal::Identity)
    ));
}
#[test]
fn initialized_offer_retains_independent_base_and_resource_without_issuing_possession() {
    let queues = EthernetFrameQueues::prepare(7, [2, 0, 0, 0, 0, 1]).unwrap();
    let ready = unsafe {
        ReadyEthernetFrameBase::new(identity(), &queues, approval("host/test", "boot/test"))
    }
    .unwrap();
    let mut advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/native-queues".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let contract = EthernetFrameContract::prepare().unwrap();
    ready
        .append_to_advertisement(&mut advertisement, &contract)
        .unwrap();
    assert_eq!(advertisement.bases.len(), 1);
    assert_eq!(advertisement.resources.len(), 1);
    assert_eq!(advertisement.capabilities.len(), 1);
    assert_eq!(
        advertisement.capabilities[0].capability_id.as_str(),
        FRAME_IMPLEMENTATION
    );
    assert_eq!(
        advertisement.capabilities[0].authority_requirements[0]
            .contract_id
            .as_str(),
        FRAME_AUTHORITY
    );
    assert_eq!(
        advertisement.resources[0].class_id.as_str(),
        FRAME_ATTACHMENT
    );
    assert_eq!(
        ready.resource_bytes(),
        core::mem::size_of::<EthernetFrameQueues>() as u64
    );
    assert!(matches!(
        ready.append_to_advertisement(&mut advertisement, &contract),
        Err(EthernetFrameNativeRefusal::Advertisement)
    ));
    assert_eq!(advertisement.bases.len(), 1);
    assert_eq!(advertisement.resources.len(), 1);
    assert_eq!(advertisement.capabilities.len(), 1);
}
