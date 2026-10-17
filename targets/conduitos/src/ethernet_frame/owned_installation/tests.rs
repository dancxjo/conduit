//! Move/drop safety of private preparation storage; no USB physical proof.
use super::*;
fn identity() -> EthernetFrameNativeIdentity {
    EthernetFrameNativeIdentity {
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        base_id: "base/network/test".into(),
        provider_instance_id: "provider/network/test".into(),
        provider_generation: 7,
        resource_pool_id: "pool/network/test".into(),
        resource_generation_id: conduit_core::ResourceGenerationId("resource/network/7".into()),
        envelope_id: "envelope/network/test".into(),
        artifact_id: "artifact/network/test".into(),
    }
}
#[test]
fn retained_geometry_charges_hidden_identity_capacity_and_full_socket_storage() {
    let mut identity = identity();
    identity
        .base_id
        .0
        .reserve(super::super::retention::OWNED_RETENTION_BYTES);
    // SAFETY: this fixture owns the complete stable storage and inert approval.
    let ready = unsafe {
        ReadyOwnedEthernetFrameBase::prepare(
            identity,
            approval(),
            OwnedEthernetNetworkInput {
                request: b"request",
                maximum_response_bytes: 64,
                expected_response_bytes: 16,
            },
        )
    }
    .unwrap();
    assert!(
        ready.retained_preparation_bytes().unwrap()
            > super::super::retention::OWNED_RETENTION_BYTES
    );
    assert!(ready.retained_preparation_bytes().unwrap() > ready.resource_bytes() as usize);
}
fn approval() -> EthernetNetworkApproval {
    // SAFETY: fixture-only Root permission, below native administrator proof.
    unsafe {
        EthernetNetworkApproval::admit(
            "host/test".into(),
            "boot/test".into(),
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
fn moving_ready_container_retains_exact_address_and_copied_request_without_leak() {
    let mut request = *b"owned-request";
    let ready = unsafe {
        ReadyOwnedEthernetFrameBase::prepare(
            identity(),
            approval(),
            OwnedEthernetNetworkInput {
                request: &request,
                maximum_response_bytes: 64,
                expected_response_bytes: 16,
            },
        )
    }
    .unwrap();
    let pointer = ready.storage.pointer();
    request.fill(0);
    let mut moved = alloc::vec![ready];
    for _ in 0..8 {
        moved.reserve(8);
    }
    let ready = moved.pop().unwrap();
    assert_eq!(ready.storage.pointer(), pointer);
    // SAFETY: only Ready's shared queue reference exists; request is a disjoint
    // admitted immutable field. No references leave the owning container.
    unsafe {
        assert_eq!(
            core::slice::from_raw_parts(core::ptr::addr_of!((*pointer).request).cast::<u8>(), 13),
            b"owned-request"
        );
    }
    assert!(ready.resource_bytes() > core::mem::size_of::<EthernetFrameQueues>() as u64);
    drop(ready); // OwnedStorage recovers and frees the exact allocation.
}
#[test]
fn session_is_destroyed_before_socket_slots_and_backing_storage_are_freed() {
    let storage = Storage::prepare(
        &identity(),
        &approval(),
        OwnedEthernetNetworkInput {
            request: b"request",
            maximum_response_bytes: 64,
            expected_response_bytes: 16,
        },
    )
    .unwrap();
    let pointer = storage.pointer();
    // SAFETY: this test retains the same private owner and exclusive disjoint
    // session projections; session is cancelled and dropped before storage.
    let mut network = unsafe {
        PreparedEthernetNetwork::prepare(
            &(*pointer).queues,
            approval(),
            Storage::network(pointer),
            7,
            Instant::ZERO,
        )
    }
    .unwrap();
    network.cancel();
    drop(network);
    drop(storage); // Clears borrowed socket slots before Box::from_raw.
}
#[test]
fn oversized_request_and_response_are_refused_before_any_session_exists() {
    for input in [
        OwnedEthernetNetworkInput {
            request: &[0; 2049],
            maximum_response_bytes: 64,
            expected_response_bytes: 16,
        },
        OwnedEthernetNetworkInput {
            request: b"r",
            maximum_response_bytes: 2049,
            expected_response_bytes: 16,
        },
        OwnedEthernetNetworkInput {
            request: b"r",
            maximum_response_bytes: 16,
            expected_response_bytes: 17,
        },
    ] {
        assert!(Storage::prepare(&identity(), &approval(), input).is_err());
    }
}
