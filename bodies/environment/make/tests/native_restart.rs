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
    journal.save(&binding, &[7; 32], &bytes).unwrap();
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
