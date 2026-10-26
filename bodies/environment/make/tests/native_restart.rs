//! CPU restart custody: actual canonical Body lifecycle with explicit stand-in
//! Core Plans. This supplies no native execution, flash or same-Body join proof.
extern crate alloc;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
mod checkpoint;
#[path = "../../../../architecture/core/tests/common/sealed_state.rs"]
mod core_fixture;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_payload.rs"]
mod payload;
use conduit_body::{BodyPlotPlan, SealedBodyLifecycleSession};
use conduit_core as c;
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
struct Media(Rc<RefCell<Vec<u8>>>);
impl checkpoint::Storage for Media {
    fn capacity(&self) -> u32 {
        self.0.borrow().len() as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), checkpoint::Refusal> {
        let media = self.0.borrow();
        let bytes = media
            .get(at as usize..at as usize + out.len())
            .ok_or(checkpoint::Refusal::Storage)?;
        out.copy_from_slice(bytes);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), checkpoint::Refusal> {
        for (old, new) in self.0.borrow_mut()[at as usize..at as usize + bytes.len()]
            .iter_mut()
            .zip(bytes)
        {
            assert_eq!(*old & new, *new);
            *old &= new;
        }
        Ok(())
    }
    fn erase_bank(&mut self, bank: u8) -> Result<(), checkpoint::Refusal> {
        self.0.borrow_mut()[usize::from(bank) * 32768..(usize::from(bank) + 1) * 32768].fill(255);
        Ok(())
    }
}
fn installed() -> (
    SealedBodyLifecycleSession,
    Vec<BodyPlotPlan>,
    c::HostAdvertisement,
) {
    let sign = c::bind_sign(
        &"construction/host".into(),
        &"construction/boot".into(),
        None,
        1,
    );
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let fragment = core_fixture::fragment();
    let host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/root".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let evidence = image.body.evidence().clone();
    let plans = evidence
        .body
        .workset
        .plots()
        .iter()
        .map(|resident| {
            let mut fragment = core_fixture::fragment();
            fragment.source_document_id = resident.source_document_id.clone();
            fragment.checked_plot_id = resident.checked_plot_id.clone();
            fragment.expanded_plot_id =
                format!("fixture/expanded/{}", resident.checked_plot_id.as_str()).into();
            BodyPlotPlan {
                plot: resident.clone(),
                plan: core_fixture::seal(fragment),
            }
        })
        .collect();
    (
        conduit_environment_runtime::body_image::open_installed_image_sealed(evidence, &host)
            .unwrap(),
        plans,
        host,
    )
}
#[test]
fn full_mutable_two_plot_body_and_original_seed_resume_after_interrupted_play() {
    use sha2::{Digest, Sha256};
    let (mut owner, plans, host) = installed();
    let installed = payload::encode(owner.evidence(), &[], &[]).unwrap();
    owner.propose(plans, &host.host_id, &host.boot_id).unwrap();
    let realization = owner.realization().unwrap();
    let play = conduit_body::BodyPlayIdentity::bind(&realization.plan, 1);
    let wake = realization
        .wake
        .body_plan_ready(
            &realization.plan,
            c::bind_sign(&host.host_id, &host.boot_id, Some(&play.active_play_id), 0).sign_id,
        )
        .unwrap()
        .body_play_started(
            &realization.plan,
            &play,
            c::bind_sign(&host.host_id, &host.boot_id, Some(&play.active_play_id), 1).sign_id,
        )
        .unwrap();
    owner
        .started(&host.host_id, &host.boot_id, play, wake)
        .unwrap();
    let binding: [u8; 32] = Sha256::digest(owner.evidence().body_id.as_str().as_bytes()).into();
    let original = owner.evidence().clone();
    let bytes = payload::encode(owner.evidence(), &[], owner.pending_archives()).unwrap();
    println!(
        "restart envelope installed={} playing={}",
        installed.len(),
        bytes.len()
    );
    let media = Media(Rc::new(RefCell::new(vec![255; 65536])));
    let mut journal = checkpoint::Checkpoint::open(media.clone()).unwrap();
    assert_eq!(journal.payload_length(&binding).unwrap(), None);
    let (_, memory) = allocation_probe::observe(|| {
        journal
            .save_streamed(&binding, &[7; 32], |sink| {
                payload::stream(owner.evidence(), &[], owner.pending_archives(), sink).map_err(
                    |error| match error {
                        payload::StreamRefusal::Sink(error) => error,
                        payload::StreamRefusal::Invalid => checkpoint::Refusal::Corrupt,
                    },
                )
            })
            .unwrap();
    });
    println!(
        "playing checkpoint peak={} allocated={}",
        memory.peak_bytes, memory.requested_bytes
    );
    assert!(memory.peak_bytes < 4096);
    drop(journal);
    drop(owner);
    let mut journal = checkpoint::Checkpoint::open(media.clone()).unwrap();
    let mut readback = vec![0; journal.payload_length(&binding).unwrap().unwrap()];
    let mut seed = [0; 32];
    journal.load(&binding, &mut seed, &mut readback).unwrap();
    assert_eq!(seed, [7; 32]);
    assert_eq!(bytes, readback);
    let retained = payload::decode(&readback, &original.body_id).unwrap();
    assert_eq!(retained.evidence, original);
    assert!(SealedBodyLifecycleSession::resume_here(
        retained.evidence.clone(),
        &host.host_id,
        &host.boot_id
    )
    .is_err());
    assert!(SealedBodyLifecycleSession::resume_here(
        retained.evidence.clone(),
        &"foreign/host".into(),
        &"boot/new".into()
    )
    .is_err());
    let resumed = SealedBodyLifecycleSession::resume_here(
        retained.evidence,
        &host.host_id,
        &"boot/new".into(),
    )
    .unwrap();
    assert_eq!(resumed.evidence().body_id, original.body_id);
    assert_eq!(resumed.evidence().body.workset, original.body.workset);
    assert_eq!(resumed.evidence().friendly_name, original.friendly_name);
    assert_eq!(
        resumed.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    assert_eq!(
        resumed.evidence().wakes.last().unwrap().lifecycle,
        conduit_body::WakeLifecycle::Failed
    );
    assert!(resumed.realization().is_none());
    let bytes = payload::encode(
        resumed.evidence(),
        &retained.archives,
        resumed.pending_archives(),
    )
    .unwrap();
    journal.save(&binding, &seed, &bytes).unwrap();
    assert_eq!(journal.generation(), Some(2));
    let mut foreign = binding;
    foreign[0] ^= 1;
    assert!(journal.payload_length(&foreign).is_err());
}
#[test]
fn nonempty_uncommitted_media_never_falls_back_to_a_new_birth_or_key() {
    let media = Media(Rc::new(RefCell::new(vec![255; 65536])));
    media.0.borrow_mut()[32768 + 160] = 0;
    assert!(checkpoint::Checkpoint::open(media.clone()).is_err());
    assert_eq!(media.0.borrow()[32768 + 160], 0);
    let (owner, _, _) = installed();
    let mut bytes = payload::encode(owner.evidence(), &[], &[]).unwrap();
    let body = owner.evidence().body_id.clone();
    let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut changed = document.clone();
    changed["evidence"]["friendly_name"] = "".into();
    assert!(payload::decode(&serde_json::to_vec(&changed).unwrap(), &body).is_err());
    bytes[0] = 0;
    assert!(payload::decode(&bytes, &body).is_err());
    assert!(payload::decode(
        &serde_json::to_vec(&document).unwrap(),
        &conduit_body::Body::born(
            "foreign/source".into(),
            "foreign/checked".into(),
            1,
            "foreign/sign".into()
        )
        .unwrap()
        .body_id
    )
    .is_err());
}

#[test]
fn archival_history_is_in_the_same_complete_transaction_and_cannot_be_dropped() {
    use sha2::{Digest, Sha256};
    let (owner, _, host) = installed();
    let mut evidence = owner.evidence().clone();
    let first = evidence.seal_membership_history().unwrap().unwrap();
    assert!(payload::encode(&evidence, &[], &[]).is_err());
    let bytes = payload::encode(&evidence, &[], core::slice::from_ref(&first)).unwrap();
    let retained = payload::decode(&bytes, &evidence.body_id).unwrap();
    assert_eq!(retained.archives.as_slice(), core::slice::from_ref(&first));
    let resumed = SealedBodyLifecycleSession::resume_here(
        retained.evidence,
        &host.host_id,
        &"boot/new".into(),
    )
    .unwrap();
    let mut evidence = resumed.evidence().clone();
    let second = evidence.seal_membership_history().unwrap().unwrap();
    assert_eq!(second.previous_digest, Some(first.digest));
    assert!(payload::encode(&evidence, &[], core::slice::from_ref(&second)).is_err());
    let bytes = payload::encode(
        &evidence,
        &retained.archives,
        core::slice::from_ref(&second),
    )
    .unwrap();
    let binding: [u8; 32] = Sha256::digest(evidence.body_id.as_str().as_bytes()).into();
    let media = Media(Rc::new(RefCell::new(vec![255; 65536])));
    let mut journal = checkpoint::Checkpoint::open(media.clone()).unwrap();
    journal.save(&binding, &[8; 32], &bytes).unwrap();
    let mut readback = vec![0; bytes.len()];
    let mut seed = [0; 32];
    journal.load(&binding, &mut seed, &mut readback).unwrap();
    let restored = payload::decode(&readback, &evidence.body_id).unwrap();
    assert_eq!(restored.evidence, evidence);
    assert_eq!(restored.archives, [first, second]);
    let mut missing: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    missing["archives"].as_array_mut().unwrap().remove(0);
    assert!(payload::decode(&serde_json::to_vec(&missing).unwrap(), &evidence.body_id).is_err());
    let mut corrupt: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    corrupt["archives"][0]["digest"][0] = 0.into();
    assert!(payload::decode(&serde_json::to_vec(&corrupt).unwrap(), &evidence.body_id).is_err());
}

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use payload::json as streamed_json;
#[test]
fn streamed_serde_json_preserves_all_scalar_variant_escape_and_key_bytes() {
    #[derive(serde::Serialize)]
    enum Variant {
        Unit,
        Newtype(u128),
        Tuple(i64, String),
        Record { text: String, optional: Option<u8> },
    }
    #[derive(serde::Serialize)]
    struct Scalar<'a> {
        signed: (i8, i16, i32, i64, i128),
        unsigned: (u8, u16, u32, u64, u128),
        floats: &'a [f64],
        boolean: bool,
        character: char,
        unit: (),
        text: String,
        variants: Vec<Variant>,
        keys: std::collections::BTreeMap<i128, bool>,
    }
    let floats = [
        0.0,
        -0.0,
        1.0,
        1e-12,
        1e100,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::INFINITY,
        f64::NAN,
    ];
    let value = Scalar {
        signed: (i8::MIN, i16::MIN, i32::MIN, i64::MIN, i128::MIN),
        unsigned: (u8::MAX, u16::MAX, u32::MAX, u64::MAX, u128::MAX),
        floats: &floats,
        boolean: true,
        character: '🦉',
        unit: (),
        text: (0..32)
            .map(char::from)
            .chain(" \"\\\né🦉".chars())
            .collect(),
        variants: vec![
            Variant::Unit,
            Variant::Newtype(u128::MAX),
            Variant::Tuple(-1, "tuple".into()),
            Variant::Record {
                text: "record".into(),
                optional: Some(7),
            },
        ],
        keys: [(i128::MIN, true), (i128::MAX, false)]
            .into_iter()
            .collect(),
    };
    fn check(value: &impl serde::Serialize) {
        let expected = serde_json::to_vec(value).unwrap();
        let mut actual = Vec::new();
        streamed_json::stream(value, &mut |bytes: &[u8]| {
            actual.extend_from_slice(bytes);
            Ok(())
        })
        .unwrap();
        assert_eq!(actual, expected);
        let (_, memory) = allocation_probe::observe(|| {
            streamed_json::stream(value, &mut |_: &[u8]| Ok(())).unwrap()
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    check(&value);
    let (_, _, advertisement) = installed();
    let body = conduit_body::Body::born(
        "manager/source".into(),
        "manager/checked".into(),
        1,
        "manager/born".into(),
    )
    .unwrap();
    let mut manager = conduit_body::AdmissionManager::new(body.body_id.clone()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            conduit_body::SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
            [12; 32],
            10,
            100,
        )
        .unwrap();
    check(&manager);
    let proof = conduit_body::SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        )),
    };
    let mut membership = conduit_body::BodyMembership::new(body.body_id).unwrap();
    manager
        .complete_spawn(
            &mut membership,
            &advertisement,
            &proof,
            11,
            conduit_body::AdmissionSigns {
                part_admitted: "manager/part".into(),
                host_attached: "manager/host".into(),
                candidate_admitted: "manager/candidate".into(),
            },
        )
        .unwrap();
    check(&manager);
    let mut bits = 17u64;
    let floats: Vec<f64> = (0..1024)
        .map(|_| {
            bits = bits.wrapping_mul(6364136223846793005).wrapping_add(1);
            f64::from_bits(bits)
        })
        .collect();
    check(&floats);
    let bad: std::collections::BTreeMap<Vec<u8>, u8> = [(vec![1], 2)].into_iter().collect();
    assert!(serde_json::to_vec(&bad).is_err());
    assert!(streamed_json::stream(&bad, &mut |_: &[u8]| Ok(())).is_err());
    assert!(
        streamed_json::stream(&value, &mut |_: &[u8]| Err(streamed_json::CodecError::Sink))
            .is_err()
    );
}
#[test]
fn streamed_full_envelope_matches_canonical_bytes_with_bounded_memory_and_readback() {
    use sha2::{Digest, Sha256};
    let (owner, _, _) = installed();
    let mut actual = Vec::new();
    payload::stream(owner.evidence(), &[], &[], &mut |bytes: &[u8]| {
        actual.extend_from_slice(bytes);
        Ok::<_, ()>(())
    })
    .unwrap();
    let expected = payload::encode(owner.evidence(), &[], &[]).unwrap();
    assert_eq!(actual, expected);
    let (_, stream_memory) = allocation_probe::observe(|| {
        payload::stream(owner.evidence(), &[], &[], &mut |_: &[u8]| Ok::<_, ()>(())).unwrap()
    });
    let (_, buffer_memory) =
        allocation_probe::observe(|| payload::encode(owner.evidence(), &[], &[]).unwrap());
    println!(
        "restart stream peak={} allocated={} vs buffer peak={}",
        stream_memory.peak_bytes, stream_memory.requested_bytes, buffer_memory.peak_bytes
    );
    assert!(stream_memory.peak_bytes < buffer_memory.peak_bytes);
    let media = Media(Rc::new(RefCell::new(vec![255; 65536])));
    let binding: [u8; 32] = Sha256::digest(owner.evidence().body_id.as_str().as_bytes()).into();
    let mut journal = checkpoint::Checkpoint::open(media).unwrap();
    journal
        .save_streamed(&binding, &[7; 32], |sink| {
            payload::stream(owner.evidence(), &[], &[], sink).map_err(|error| match error {
                payload::StreamRefusal::Sink(error) => error,
                payload::StreamRefusal::Invalid => checkpoint::Refusal::Corrupt,
            })
        })
        .unwrap();
    let mut readback = vec![0; expected.len()];
    let mut seed = [0; 32];
    journal.load(&binding, &mut seed, &mut readback).unwrap();
    assert_eq!(readback, expected);
    assert_eq!(seed, [7; 32]);
    // A changed second pass can leave only an uncommitted bank, preserving the
    // previous complete Body and original signer even if lengths still match.
    let mut pass = 0;
    assert!(journal
        .save_streamed(&binding, &[8; 32], |sink| {
            pass += 1;
            sink(if pass == 1 { b"first" } else { b"other" })
        })
        .is_err());
    journal.load(&binding, &mut seed, &mut readback).unwrap();
    assert_eq!(seed, [7; 32]);
    assert_eq!(readback, expected);
}

#[test]
fn complete_admission_custody_is_in_the_same_streamed_checkpoint() {
    use conduit_body::{AdmissionManager, SpawnInvitationSecret};
    let (owner, _, _) = installed();
    let mut manager = AdmissionManager::new(owner.evidence().body_id.clone()).unwrap();
    manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([41; 32]).unwrap(),
            [42; 32],
            10,
            100,
        )
        .unwrap();
    let mut bytes = Vec::new();
    payload::stream_with_admissions(
        owner.evidence(),
        &[],
        &[],
        Some(&manager),
        &mut |part: &[u8]| {
            bytes.extend_from_slice(part);
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    let retained = payload::decode(&bytes, &owner.evidence().body_id).unwrap();
    assert_eq!(retained.admissions, Some(manager.clone()));
    let expected = serde_json::json!({"schema":payload::SCHEMA,"evidence":owner.evidence(),"archives":[],"admissions":manager});
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        expected
    );
    let mut wrong = manager;
    wrong.body_id = conduit_body::Body::born(
        "source/foreign".into(),
        "checked/foreign".into(),
        1,
        "sign/foreign".into(),
    )
    .unwrap()
    .body_id;
    assert!(payload::stream_with_admissions(
        owner.evidence(),
        &[],
        &[],
        Some(&wrong),
        &mut |_: &[u8]| Ok::<_, ()>(())
    )
    .is_err());
}

#[test]
fn invitation_observer_admission_preserves_existing_local_wake_plan_and_play() {
    use conduit_body::{AdmissionManager, SpawnAdmissionProof, SpawnInvitationSecret};
    let (mut owner, plans, host) = installed();
    owner.propose(plans, &host.host_id, &host.boot_id).unwrap();
    let realization = owner.realization().unwrap();
    assert!(realization.plan.body_time_requirement().is_none());
    let play = conduit_body::BodyPlayIdentity::bind(&realization.plan, 1);
    let wake = realization
        .wake
        .body_plan_ready(
            &realization.plan,
            c::bind_sign(&host.host_id, &host.boot_id, Some(&play.active_play_id), 0).sign_id,
        )
        .unwrap()
        .body_play_started(
            &realization.plan,
            &play,
            c::bind_sign(&host.host_id, &host.boot_id, Some(&play.active_play_id), 1).sign_id,
        )
        .unwrap();
    owner
        .started(&host.host_id, &host.boot_id, play, wake)
        .unwrap();
    let original = owner.realization().unwrap().clone();
    let root_presence = owner.evidence().membership.parts.clone();
    let mut manager = AdmissionManager::new(owner.evidence().body_id.clone()).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes([51; 32]).unwrap();
    let claim = owner
        .issue_invitation(
            &mut manager,
            secret.clone(),
            [52; 32],
            10,
            100,
            &host.host_id,
            &host.boot_id,
        )
        .unwrap();
    let mut observer = host.clone();
    observer.host_id = "host/recorder".into();
    observer.boot_id = "boot/recorder".into();
    let proof = SpawnAdmissionProof {
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_id: observer.host_id.clone(),
        boot_id: observer.boot_id.clone(),
        nonce: claim.nonce,
        signature: secret.sign(&claim.signing_transcript(
            &observer.host_id,
            &observer.boot_id,
            observer.offer_generation,
        )),
    };
    owner
        .admit_invited_host(
            &mut manager,
            &observer,
            &proof,
            11,
            &host.host_id,
            &host.boot_id,
        )
        .unwrap();
    assert_eq!(owner.realization(), Some(&original));
    assert_eq!(
        &owner.evidence().membership.parts[..root_presence.len()],
        root_presence
    );
    assert_eq!(
        owner.evidence().membership.parts.len(),
        root_presence.len() + 1
    );
    owner.evidence().validate().unwrap();
    let retained = owner.evidence().clone();
    assert!(owner
        .admit_invited_host(
            &mut manager,
            &observer,
            &proof,
            12,
            &host.host_id,
            &host.boot_id
        )
        .is_err());
    assert_eq!(owner.evidence(), &retained);
    assert_eq!(owner.realization(), Some(&original));
}
