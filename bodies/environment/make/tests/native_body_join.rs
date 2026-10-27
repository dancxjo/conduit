//! Actual Root custody/join producer on simulated flash; no hardware or public
//! authenticated Line is established by this CPU proof.
// Embedded profile flags do not exist in this CPU fixture crate.
#![allow(unexpected_cfgs)]
extern crate alloc;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
#[allow(dead_code)] // Compatibility byte-slice API is covered by native_restart tests.
mod native_body_checkpoint;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_join.rs"]
mod native_body_join;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_joined_host_wire.rs"]
mod native_joined_host_wire;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart.rs"]
#[allow(dead_code)] // Legacy envelope helpers are covered by native_restart tests.
mod native_restart;
use native_body_join::recovery;
#[path = "native_body_join/returning.rs"]
mod returning;
mod native_evidence_region {
    use crate::native_body_checkpoint::{Refusal, Storage};
    use std::sync::{Arc, Mutex};
    #[derive(Clone)]
    pub struct BodyRegion(pub Arc<Mutex<(Vec<u8>, bool)>>);
    pub fn test_empty() -> BodyRegion {
        BodyRegion(Arc::new(Mutex::new((vec![255; 65536], true))))
    }
    impl Storage for BodyRegion {
        fn capacity(&self) -> u32 {
            65536
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
            out.copy_from_slice(&self.0.lock().unwrap().0[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
            let mut state = self.0.lock().unwrap();
            if !state.1 {
                return Err(Refusal::Storage);
            }
            for (old, new) in state.0[at as usize..at as usize + bytes.len()]
                .iter_mut()
                .zip(bytes)
            {
                assert_eq!(*old & new, *new);
                *old &= new;
            }
            Ok(())
        }
        fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
            let mut state = self.0.lock().unwrap();
            if !state.1 {
                return Err(Refusal::Storage);
            }
            state.0[usize::from(bank) * 32768..(usize::from(bank) + 1) * 32768].fill(255);
            Ok(())
        }
    }
}
use conduit_core as c;
use ed25519_dalek::{Signer, SigningKey};
#[test]
fn real_store_producer_admits_only_after_durable_custody_and_restores_short_lock() {
    let host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/root".into(),
        boot_id: "boot/root".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "profile/root".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let sign = c::bind_sign(&host.host_id, &host.boot_id, None, 1);
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let owner = conduit_environment_runtime::body_image::open_installed_image_sealed(
        image.body.evidence().clone(),
        &host,
    )
    .unwrap();
    let media = native_evidence_region::BodyRegion(std::sync::Arc::new(std::sync::Mutex::new((
        vec![255; 65536],
        true,
    ))));
    native_restart::initialize(media.clone(), owner.evidence().clone()).unwrap();
    let seed = native_restart::select_signer_seed([7; 32]).unwrap();
    let signer = SigningKey::from_bytes(&seed);
    let root_key = signer.verifying_key().to_bytes();
    let retained = native_restart::take_body().unwrap();
    assert!(retained.pristine);
    let mut owner = conduit_body::SealedBodyLifecycleSession::open(retained.evidence).unwrap();
    native_restart::checkpoint(&mut owner, &root_key).unwrap();
    assert!(native_restart::publication_ready());
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap();
    let bootstrap = native_body_join::issue(
        &mut owner,
        &root_key,
        &host.host_id,
        &host.boot_id,
        secret.clone(),
        [12; 32],
        10,
        100,
    )
    .unwrap();
    bootstrap.invitation.validate(11).unwrap();
    assert_eq!(bootstrap.root_key, root_key);
    let observer = c::HostAdvertisement {
        host_id: "host/recorder".into(),
        boot_id: "boot/recorder".into(),
        ..host.clone()
    };
    let claim = bootstrap.invitation.claim;
    let request = conduit_body::PortableSpawnAdmissionRequest {
        schema: conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_advertisement: observer.clone(),
        nonce: claim.nonce,
        signature: secret
            .sign(&claim.signing_transcript(
                &observer.host_id,
                &observer.boot_id,
                observer.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let encoded = serde_json::to_vec(&request).unwrap();
    match native_body_join::decode_command(&encoded).unwrap() {
        native_body_join::Command::Admission(decoded) => {
            assert_eq!(decoded.body_id, request.body_id)
        }
        _ => panic!("admission became recovery"),
    }
    let request = native_body_join::wire::decode(&encoded).unwrap();
    for field in ["bases", "resources", "capabilities", "planner_capabilities"] {
        let mut malformed: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        malformed["host_advertisement"][field] =
            serde_json::json!([{"unknown":"must refuse before rich parsing"}]);
        assert!(native_body_join::wire::decode(&serde_json::to_vec(&malformed).unwrap()).is_err());
    }
    let mut malformed: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    malformed["host_advertisement"]["unknown"] = serde_json::Value::Bool(true);
    assert!(native_body_join::wire::decode(&serde_json::to_vec(&malformed).unwrap()).is_err());
    assert!(native_body_join::wire::decode(&vec![b' '; 1025]).is_err());
    let durable = native_body_join::admit(
        &mut owner,
        &root_key,
        &host.host_id,
        &host.boot_id,
        &request,
        11,
    )
    .unwrap();
    let signature = signer.sign(&durable.transcript()).to_bytes();
    let receipt = durable.signed(signature);
    receipt.verify(&root_key).unwrap();
    assert_eq!(receipt.credential.host_id, observer.host_id);
    let transcript = [31; 32];
    native_restart::with_admissions(|manager| {
        assert!(!native_restart::publication_ready());
        assert!(native_restart::with_admissions(|_| Ok(())).is_err());
        // Another thread can enter the platform critical section while the
        // custody callback executes; the old long outer lock prevented this.
        let (send, receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || critical_section::with(|_| send.send(()).unwrap()));
        receive
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        worker.join().unwrap();
        manager
            .verify_admitted_message(
                &owner.evidence().membership,
                &receipt.credential.part_id,
                &observer.host_id,
                &observer.boot_id,
                &transcript,
                &secret.sign(&transcript),
            )
            .map_err(|_| "member-message")?;
        Ok(())
    })
    .unwrap();
    assert!(native_restart::publication_ready());
    assert!(native_restart::generation().unwrap() >= 3);
    let retained = owner.evidence().clone();
    assert!(native_body_join::admit(
        &mut owner,
        &root_key,
        &host.host_id,
        &host.boot_id,
        &request,
        12
    )
    .is_err());
    assert_eq!(owner.evidence(), &retained);
    // Restore through the actual streamed journal/typed DTO, preserving the
    // admitted Part's private continuity verifier and completed receipt exactly.
    let before_manager =
        native_restart::with_admissions(|manager| Ok(serde_json::to_vec(manager).unwrap()))
            .unwrap();
    let mut boxed = Some(Box::new(owner));
    let parked =
        native_restart::park_box(&mut boxed, &root_key, &host.host_id, &host.boot_id).unwrap();
    assert!(boxed.is_none());
    owner =
        *native_restart::restore_parked(parked, &host.host_id, &host.boot_id, &root_key).unwrap();
    assert_eq!(owner.evidence(), &retained);
    native_restart::with_admissions(|manager| {
        assert_eq!(serde_json::to_vec(manager).unwrap(), before_manager);
        manager
            .verify_admitted_message(
                &owner.evidence().membership,
                &receipt.credential.part_id,
                &observer.host_id,
                &observer.boot_id,
                &transcript,
                &secret.sign(&transcript),
            )
            .map_err(|_| "member-message")?;
        Ok(())
    })
    .unwrap();
    // A durable admitted Part can recover a current-membership receipt after
    // unfinished invitation state and volatile HTTP results have disappeared.
    native_restart::with_admissions(|manager| {
        let mut restored: conduit_body::AdmissionManager =
            serde_json::from_slice(&serde_json::to_vec(manager).unwrap()).unwrap();
        restored.retire_unfinished_challenges();
        let before = serde_json::to_vec(&restored).unwrap();
        let mut query = recovery::Request {
            schema: recovery::REQUEST.into(),
            body_id: owner.evidence().body_id.clone(),
            host_id: observer.host_id.clone(),
            boot_id: observer.boot_id.clone(),
            nonce: [91; 32],
            signature: vec![],
        };
        query.signature = secret.sign(&query.transcript()).to_vec();
        match native_body_join::decode_command(&serde_json::to_vec(&query).unwrap()).unwrap() {
            native_body_join::Command::Recovery(decoded) => {
                assert_eq!(decoded.transcript(), query.transcript())
            }
            _ => panic!("recovery became admission"),
        }
        let root_boot: c::BootId = "boot/root-after-power-cycle".into();
        let verified = recovery::verify_current(
            &restored,
            &owner.evidence().membership,
            &query,
            &host.host_id,
            &root_boot,
            &root_key,
        )
        .unwrap();
        let signature = signer.sign(&verified.transcript()).to_bytes();
        let current = verified.signed(signature);
        current.verify(&root_key, &query).unwrap();
        assert_eq!(current.credential, receipt.credential);
        assert_eq!(current.producer_boot, root_boot);
        assert_eq!(current.credential.boot_id, observer.boot_id);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), before);
        assert_eq!(owner.evidence(), &retained);
        let encoded = serde_json::to_value(&current).unwrap();
        for field in [
            "schema",
            "membership_revision",
            "producer_host",
            "producer_boot",
            "producer_key",
            "request_nonce",
            "signature",
            "credential",
        ] {
            let mut altered = encoded.clone();
            altered[field] = match field {
                "membership_revision" => serde_json::json!(999),
                "producer_key" | "request_nonce" => serde_json::json!(vec![0; 32]),
                "signature" => serde_json::json!(vec![0; 64]),
                "credential" => {
                    let mut c = encoded[field].clone();
                    c["boot_id"] = "boot/foreign".into();
                    c
                }
                _ => serde_json::json!("foreign"),
            };
            let altered: recovery::Receipt = serde_json::from_value(altered).unwrap();
            assert!(altered.verify(&root_key, &query).is_err(), "{field}");
        }
        query.signature[0] ^= 1;
        assert!(recovery::verify_current(
            &restored,
            &owner.evidence().membership,
            &query,
            &host.host_id,
            &root_boot,
            &root_key
        )
        .is_err());
        query.signature[0] ^= 1;
        query.nonce = [92; 32];
        assert!(current.verify(&root_key, &query).is_err());
        assert!(recovery::verify_current(
            &restored,
            &owner.evidence().membership,
            &query,
            &host.host_id,
            &root_boot,
            &root_key
        )
        .is_err());
        query.signature = secret.sign(&query.transcript()).to_vec();
        let mut duplicate = restored.clone();
        duplicate.receipts.push(duplicate.receipts[0].clone());
        assert!(recovery::verify_current(
            &duplicate,
            &owner.evidence().membership,
            &query,
            &host.host_id,
            &root_boot,
            &root_key
        )
        .is_err());
        let mut lost = owner.evidence().membership.clone();
        lost.parts
            .iter_mut()
            .find(|p| p.part_id == receipt.credential.part_id)
            .unwrap()
            .current = None;
        assert!(recovery::verify_current(
            &restored,
            &lost,
            &query,
            &host.host_id,
            &root_boot,
            &root_key
        )
        .is_err());
        query.nonce = [0; 32];
        query.signature = secret.sign(&query.transcript()).to_vec();
        assert!(recovery::verify_current(
            &restored,
            &owner.evidence().membership,
            &query,
            &host.host_id,
            &root_boot,
            &root_key
        )
        .is_err());
        query.nonce = [92; 32];
        query.boot_id = "boot/observer-new".into();
        query.signature = secret.sign(&query.transcript()).to_vec();
        assert!(recovery::verify_current(
            &restored,
            &owner.evidence().membership,
            &query,
            &host.host_id,
            &root_boot,
            &root_key
        )
        .is_err());
        assert_eq!(serde_json::to_vec(&restored).unwrap(), before);
        Ok(())
    })
    .unwrap();
    owner = returning::prove(
        owner, &root_key, &host, &observer, &receipt, &secret, &signer, &media,
    );
    // No private bootstrap can leave the issuer after failed persistence.
    media.0.lock().unwrap().1 = false;
    assert!(native_body_join::issue(
        &mut owner,
        &root_key,
        &host.host_id,
        &host.boot_id,
        secret,
        [21; 32],
        20,
        100
    )
    .is_err());
    assert!(!native_restart::publication_ready());
    assert!(native_restart::with_admissions(|_| Ok(())).is_err());
}

#[test]
fn usb_bootstrap_requires_pinned_owner_fresh_root_scope_and_consumable_nonce() {
    use native_body_join::owner::{Gate, Request, PROFILE};
    let owner = SigningKey::from_bytes(&[71; 32]);
    let wrong = SigningKey::from_bytes(&[72; 32]);
    let mut gate = Gate::new(Some(owner.verifying_key().to_bytes())).unwrap();
    assert!(Gate::new(None).is_err());
    assert!(gate.challenge([0; 32], 100).is_err());
    gate.challenge([73; 32], 100).unwrap();
    assert_eq!(gate.next_sequence(), Some(1));
    let mut request = Request {
        schema: PROFILE.into(),
        host: "host/root".into(),
        boot: "boot/root".into(),
        body: "body/exact".into(),
        root_key: [74; 32],
        nonce: [73; 32],
        sequence: 1,
        ttl_ms: 60_000,
        signature: vec![],
    };
    request.signature = wrong.sign(&request.transcript()).to_bytes().to_vec();
    assert!(gate
        .authorize(
            &request,
            "host/root",
            "boot/root",
            "body/exact",
            &[74; 32],
            101
        )
        .is_err());
    request.signature = owner.sign(&request.transcript()).to_bytes().to_vec();
    assert!(serde_json::to_vec(&request).unwrap().len() <= 1024);
    for (host, boot, body, key, now) in [
        ("host/foreign", "boot/root", "body/exact", [74; 32], 101),
        ("host/root", "boot/stale", "body/exact", [74; 32], 101),
        ("host/root", "boot/root", "body/foreign", [74; 32], 101),
        ("host/root", "boot/root", "body/exact", [75; 32], 101),
        ("host/root", "boot/root", "body/exact", [74; 32], 30_101),
    ] {
        assert!(gate
            .authorize(&request, host, boot, body, &key, now)
            .is_err());
    }
    assert_eq!(
        gate.authorize(
            &request,
            "host/root",
            "boot/root",
            "body/exact",
            &[74; 32],
            101
        )
        .unwrap()
        .ttl_ms(),
        60_000
    );
    assert!(gate
        .authorize(
            &request,
            "host/root",
            "boot/root",
            "body/exact",
            &[74; 32],
            102
        )
        .is_err());
    gate.challenge([76; 32], 103).unwrap();
    assert!(gate
        .authorize(
            &request,
            "host/root",
            "boot/root",
            "body/exact",
            &[74; 32],
            104
        )
        .is_err());
    request.nonce = [76; 32];
    request.signature = owner.sign(&request.transcript()).to_bytes().to_vec();
    assert!(gate
        .authorize(
            &request,
            "host/root",
            "boot/root",
            "body/exact",
            &[74; 32],
            104
        )
        .is_err());
    request.sequence = 2;
    request.ttl_ms = 600_001;
    request.signature = owner.sign(&request.transcript()).to_bytes().to_vec();
    assert!(gate
        .authorize(
            &request,
            "host/root",
            "boot/root",
            "body/exact",
            &[74; 32],
            104
        )
        .is_err());
}

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
