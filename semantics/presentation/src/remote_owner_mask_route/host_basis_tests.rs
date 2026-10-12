//! Local inventory admission and the compact basis sent in a remote seal.
use super::*;
use alloc::{string::ToString, vec};

fn host(profile_bytes: usize) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/remote-mask-owner"),
        boot_id: BootId::from("boot/remote-mask-owner"),
        offer_generation: OfferGeneration(1),
        // Padding isolates the serialized extent from platform catalog changes.
        profile: conduit_core::HostProfileId::from("p".repeat(profile_bytes)),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

#[test]
fn larger_local_inventory_binds_a_compact_exact_basis() {
    let mut offer = host(600 * 1024);
    let bytes = serde_json::to_vec(&offer).unwrap();
    assert!(bytes.len() > 512 * 1024 && bytes.len() <= MAX_HOST_OFFER_BYTES);
    let basis = RemoteMaskHostBasis::bind(&offer).unwrap();
    assert!(basis.valid());
    assert_eq!(basis.offer_digest, hex(&Sha256::digest(&bytes)));
    assert!(serde_json::to_vec(&basis).unwrap().len() < 256);
    offer.profile = conduit_core::HostProfileId::from("q".repeat(600 * 1024));
    assert_ne!(
        RemoteMaskHostBasis::bind(&offer).unwrap().offer_digest,
        basis.offer_digest
    );
    assert_eq!(MAX_REMOTE_ROUTE_SEAL_BYTES, 40 * 1024);
}

#[test]
fn inventory_extent_and_current_host_identity_still_refuse() {
    assert_eq!(
        RemoteMaskHostBasis::bind(&host(MAX_HOST_OFFER_BYTES)),
        Err(RemoteOwnerMaskRouteError::SealCapacityExceeded)
    );
    let mut offer = host(0);
    offer.boot_id = BootId::from("".to_string());
    assert_eq!(
        RemoteMaskHostBasis::bind(&offer),
        Err(RemoteOwnerMaskRouteError::InvalidHost)
    );
}
