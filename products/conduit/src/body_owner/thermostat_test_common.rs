//! Shared signed peer and directional Line fixtures; no live TLS transport proof.
use super::*;
use conduit_body::{
    PortableAdmissionReceipt, PortableSpawnAdmissionRequest, SpawnInvitationSecret,
    SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::*;
use conduit_face::OwnerFaceSnapshotRequest;
use std::{os::unix::net::UnixStream, path::Path};

pub(super) fn admitted_peer(
    owner: &mut Owner,
    root: &Path,
) -> (
    StdHost,
    UnixStream,
    PortableAdmissionReceipt,
    OwnerFaceSnapshotRequest,
) {
    let mut peer = StdHost::new_with_config(StdHostConfig {
        host_id: "host/thermostat-terminal".into(),
        boot_id: "boot/thermostat-terminal".into(),
        offer_generation: OfferGeneration(1),
    });
    let (provider, foreground) = UnixStream::pair().unwrap();
    peer.attach_terminal_mask(provider).unwrap();
    let peer_offer = peer.advertisement().clone();
    let invitation = owner.issue_invitation(root, 60, None).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let receipt = owner
        .admit_invited(
            root,
            PortableSpawnAdmissionRequest {
                schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
                invitation_id: invitation.claim.invitation_id.clone(),
                body_id: invitation.claim.body_id.clone(),
                host_advertisement: peer_offer.clone(),
                nonce: invitation.claim.nonce,
                signature: secret
                    .sign(&invitation.claim.signing_transcript(
                        &peer_offer.host_id,
                        &peer_offer.boot_id,
                        peer_offer.offer_generation,
                    ))
                    .to_vec(),
                membership_admitted: false,
                plan_created: false,
                play_created: false,
            },
            "host/thermostat-terminal",
        )
        .unwrap();
    let credential = &receipt.credential;
    let request = OwnerFaceSnapshotRequest {
        schema: conduit_face::OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: credential.credential_id.as_str().into(),
        body_id: credential.body_id.clone(),
        part_id: credential.part_id.clone(),
        host_id: credential.host_id.clone(),
        boot_id: credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    };
    (peer, foreground, receipt, request)
}

pub(super) fn terminal_lines(
    owner_offer: &HostAdvertisement,
    peer_offer: &HostAdvertisement,
    receipt: &PortableAdmissionReceipt,
) -> [LineOffer; 3] {
    let credential = &receipt.credential;
    let line = |name: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            &format!("line/thermostat-terminal-test/{name}"),
            &format!("binding/thermostat-terminal-test/{name}"),
            "conduit.base/authenticated-tls-websocket@1".into(),
            "base-instance/thermostat-terminal-test",
            source,
            sink,
            LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64 * 1024,
                maximum_buffered_bytes: 128 * 1024,
                maximum_frame_bytes: 64 * 1024,
            },
        );
        line.binding.credential =
            LinkCredentialReference::Opaque(credential.credential_id.as_str().into());
        line.binding.authority =
            LinkAuthorityReference::Grant("grant/thermostat-terminal-test".into());
        line.contract.scope = LineScope::RoutedNetwork;
        line.contract.security = LineSecurity::AuthenticatedEncrypted;
        line
    };
    let face_line = line("face", owner_offer, peer_offer);
    let show_line = line("show", peer_offer, owner_offer);
    let interaction_line = line("interaction", peer_offer, owner_offer);
    [face_line, show_line, interaction_line]
}
