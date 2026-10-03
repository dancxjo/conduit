use super::*;
use conduit_body::{
    PortableSpawnAdmissionRequest, ResidentPlot, SpawnInvitationSecret,
    SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostConfig};
use std::path::PathBuf;

const SOURCE: &str = "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.";
// Ed25519 public key for the deterministic test secret [7; 32].
const BROWSER_KEY: [u8; 32] = [
    0xea, 0x4a, 0x6c, 0x63, 0xe2, 0x9c, 0x52, 0x0a, 0xbe, 0xf5, 0x50, 0x7b, 0x13, 0x2e, 0xc5, 0xf9,
    0x95, 0x47, 0x76, 0xae, 0xbe, 0xbe, 0x7b, 0x92, 0x42, 0x1e, 0xea, 0x69, 0x14, 0x46, 0xd2, 0x2c,
];

fn host(id: &str, boot: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(id),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
    })
}

fn setup() -> (Owner, PathBuf, ResidentPlot) {
    let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
        "owner-browser-window",
        "service-test",
    ));
    std::fs::create_dir_all(&root).unwrap();
    let installation = crate::durable_host::Installation {
        schema: crate::durable_host::INSTALL_SCHEMA.into(),
        host_id: "host/owner-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: crate::durable_host::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
    };
    crate::durable_host::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let plot = crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(
        host("host/owner-test", "boot/owner/first"),
        resident.clone(),
        None,
        "Same Body",
    )
    .unwrap();
    owner.persist(&root).unwrap();
    (owner, root, resident)
}

fn advertise(owner: &mut Owner, window_id: &str) -> conduit_body::AdmissionChallenge {
    let advertisement = host("host/browser-test", "boot/browser/first")
        .advertisement()
        .clone();
    let result = owner
        .browser_begin(
            window_id,
            &LinkBindingId::from("line/test/browser"),
            In::Advertise {
                protocol: PROTOCOL,
                advertisement,
                friendly_label: "Browser test".into(),
                verifying_key: BROWSER_KEY.to_vec(),
                freshness_sequence: 1,
            },
            512,
        )
        .unwrap();
    let Out::Challenge { challenge, .. } = result else {
        panic!("expected ambient challenge")
    };
    challenge
}

#[test]
fn signed_browser_proof_uses_latest_owner_after_interleaved_native_admission() {
    let (mut owner, root, _) = setup();
    let authorized = owner
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let challenge = advertise(&mut owner, &authorized.window_id);
    assert!(
        owner.admissions.is_none(),
        "unfinished browser challenge must stay ephemeral"
    );

    let invitation = owner.issue_invitation(&root, 60, None).unwrap();
    let native = host("host/native-test", "boot/native/first")
        .advertisement()
        .clone();
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: native.clone(),
        nonce: invitation.claim.nonce,
        signature: secret
            .sign(&invitation.claim.signing_transcript(
                &native.host_id,
                &native.boot_id,
                native.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let native_receipt = owner
        .admit_invited(&root, request, "host/native-test")
        .unwrap();
    let browser_secret = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let snapshot = owner
        .browser_complete(
            &root,
            &authorized.window_id,
            In::AmbientProof {
                protocol: PROTOCOL,
                admission_id: challenge.admission_id.clone(),
                body_id: challenge.body_id.clone(),
                host_id: challenge.host_id.clone(),
                boot_id: challenge.boot_id.clone(),
                nonce: challenge.nonce.to_vec(),
                signature: browser_secret
                    .sign(&challenge.signing_transcript())
                    .to_vec(),
            },
        )
        .unwrap();
    assert_eq!(snapshot.biography.membership.parts.len(), 3);
    assert_eq!(
        snapshot.credential.host_id,
        HostId::from("host/browser-test")
    );
    assert!(snapshot
        .biography
        .membership
        .parts
        .iter()
        .any(|part| part.part_id == native_receipt.credential.part_id
            && part
                .current
                .as_ref()
                .is_some_and(|current| current.host_id == native.host_id
                    && current.boot_id == native.boot_id)));
    owner
        .browser_leave(&root, &authorized.window_id, &snapshot.credential)
        .unwrap();
    assert!(owner
        .session
        .evidence()
        .membership
        .parts
        .iter()
        .any(|part| part.part_id == native_receipt.credential.part_id && part.current.is_some()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn aborted_challenge_is_not_retained_across_service_restart() {
    let (mut owner, root, resident) = setup();
    let authorized = owner
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let first = advertise(&mut owner, &authorized.window_id);
    owner.browser_abort(&authorized.window_id).unwrap();
    owner.browser_cancel_window(&authorized.window_id).unwrap();
    let mut resumed = Owner::open(
        host("host/owner-test", "boot/owner/second"),
        resident,
        Some(owner.session.evidence().clone()),
        "ignored",
    )
    .unwrap();
    resumed.restore_execution(&root).unwrap();
    let second = resumed
        .browser_authorize_window("host/browser-test", Some(BROWSER_KEY), 10_000)
        .unwrap();
    let next = advertise(&mut resumed, &second.window_id);
    assert_ne!(first.admission_id, next.admission_id);
    assert!(resumed
        .admissions
        .as_ref()
        .is_none_or(|manager| manager.receipts.is_empty()));
    std::fs::remove_dir_all(root).unwrap();
}
