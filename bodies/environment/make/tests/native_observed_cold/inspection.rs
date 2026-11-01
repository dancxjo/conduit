//! Real journal/signature inspection with only physical storage substituted.
use super::*;
use conduit_environment_runtime::{
    display_evidence_journal::{self as evidence, Storage},
    observed_record,
};
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_custody_acceptance.rs"]
mod acceptance;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_custody_journal.rs"]
mod custody_journal;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_local_history.rs"]
mod history;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_state.rs"]
mod state;
struct Media {
    bytes: Vec<u8>,
    identity: u8,
    reads: usize,
}
struct Region(Arc<StdMutex<Media>>);
impl Storage for Region {
    fn capacity(&self) -> u32 {
        custody_journal::DATA as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), evidence::Refusal> {
        let mut m = self.0.lock().unwrap();
        m.reads += 1;
        out.copy_from_slice(
            m.bytes
                .get(at as usize..at as usize + out.len())
                .ok_or(evidence::Refusal::Media)?,
        );
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), evidence::Refusal> {
        let mut m = self.0.lock().unwrap();
        let to = m
            .bytes
            .get_mut(at as usize..at as usize + bytes.len())
            .ok_or(evidence::Refusal::Media)?;
        for (a, b) in to.iter_mut().zip(bytes) {
            assert_eq!(*a & b, *b);
            *a &= b;
        }
        Ok(())
    }
}
impl custody_journal::WholeEpoch for Region {
    fn identity(&self) -> [u8; 32] {
        [self.0.lock().unwrap().identity; 32]
    }
    fn read(&mut self, at: usize, out: &mut [u8]) -> Result<(), custody_journal::Error> {
        Storage::read(self, at as u32, out).map_err(|_| custody_journal::Error::Media)
    }
    fn program_marker(&mut self, _: u8, _: usize, _: &[u8]) -> Result<(), custody_journal::Error> {
        Err(custody_journal::Error::Phase)
    }
    fn erase_marker(&mut self, _: u8) -> Result<(), custody_journal::Error> {
        Err(custody_journal::Error::Phase)
    }
    fn erase_data(&mut self, _: u8) -> Result<(), custody_journal::Error> {
        Err(custody_journal::Error::Phase)
    }
}
fn fixture(
    count: u32,
) -> (
    state::Active<Region>,
    Arc<StdMutex<Media>>,
    conduit_body::BodyId,
    ed25519_dalek::SigningKey,
) {
    let sign = conduit_core::bind_sign(
        &"inspection/host".into(),
        &"inspection/boot".into(),
        None,
        0,
    );
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let body = image.body.evidence().body_id.clone();
    let key = ed25519_dalek::SigningKey::from_bytes(&[41; 32]);
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let basis: &'static [u8] = Box::leak(Box::new(observed_record::descriptor(&contract).unwrap()));
    let forms = Sha256::digest(basis).into();
    let mut scope = [0; 96];
    scope[..32].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
    scope[32..64].fill(3);
    scope[64..].fill(4);
    let media = Arc::new(StdMutex::new(Media {
        bytes: vec![255; custody_journal::DATA],
        identity: 7,
        reads: 0,
    }));
    let erased = evidence::Erased::prepare_retained(Region(media.clone()))
        .ok()
        .unwrap();
    let mut work = [0; 536];
    let mut journal =
        evidence::Journal::create_prepared(erased, &scope, basis, 960, &mut work).unwrap();
    for sequence in 1..=u64::from(count) {
        let record = observed_record::ObservedRecord {
            sequence,
            observed_at_boot_ms: sequence * 100,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap();
        let mut signed = [0; observed_record::BYTES + 64];
        signed[..observed_record::BYTES].copy_from_slice(&record);
        signed[observed_record::BYTES..]
            .copy_from_slice(&history::sign(&key, &scope, &forms, &record).unwrap());
        journal.append(&signed, &mut work).unwrap();
    }
    (
        state::Active {
            journal,
            scope,
            forms,
            last_sequence: u64::from(count),
            failed: false,
            recovered: true,
            signed: true,
            maximum_records: 960,
        },
        media,
        body,
        key,
    )
}
#[test]
fn bounded_inspector_matches_whole_offer_at_full_960_record_scope() {
    let (mut active, media, body, key) = fixture(960);
    let mut work = [0; 536];
    let expected = acceptance::inspect(&mut active, &body, &key.verifying_key(), &mut work)
        .unwrap()
        .transcript();
    let mut inspector =
        acceptance::Inspector::start(&mut active, &body, &key.verifying_key()).unwrap();
    for _ in 0..960 {
        let reads = media.lock().unwrap().reads;
        assert!(!inspector.step(&mut work).unwrap());
        assert_eq!(
            media.lock().unwrap().reads - reads,
            2,
            "one physical journal record per turn"
        );
    }
    assert!(inspector.step(&mut work).unwrap());
    assert!(inspector.step(&mut work).unwrap());
    assert_eq!(inspector.finish().unwrap().transcript(), expected);
}
#[test]
fn bounded_inspector_refuses_incomplete_poisoned_and_changed_media() {
    let (mut active, media, body, key) = fixture(3);
    let mut work = [0; 536];
    let key = key.verifying_key();
    assert!(acceptance::Inspector::start(&mut active, &body, &key)
        .unwrap()
        .finish()
        .is_err());
    let mut inspector = acceptance::Inspector::start(&mut active, &body, &key).unwrap();
    assert!(inspector.step(&mut []).is_err());
    assert_eq!(inspector.step(&mut work), Err("custody inspection failed"));
    assert!(inspector.finish().is_err());
    let mut inspector = acceptance::Inspector::start(&mut active, &body, &key).unwrap();
    assert!(!inspector.step(&mut work).unwrap());
    media.lock().unwrap().identity = 8;
    assert_eq!(inspector.step(&mut work), Err("custody inspection changed"));
    assert!(inspector.finish().is_err());
    media.lock().unwrap().identity = 7;
    let mut inspector = acceptance::Inspector::start(&mut active, &body, &key).unwrap();
    assert!(!inspector.step(&mut work).unwrap());
    media.lock().unwrap().bytes.fill(0);
    assert_eq!(inspector.step(&mut work), Err("custody journal replay"));
    assert_eq!(inspector.step(&mut work), Err("custody inspection failed"));
    assert!(inspector.finish().is_err());
    active.failed = true;
    assert!(acceptance::Inspector::start(&mut active, &body, &key).is_err());
}

#[test]
fn owned_inspection_can_move_between_turns_and_always_returns_the_same_media() {
    let (mut active, media, body, key) = fixture(3);
    let mut work = [0; 536];
    let expected = acceptance::inspect(&mut active, &body, &key.verifying_key(), &mut work)
        .unwrap()
        .transcript();
    let mut inspection = acceptance::OwnedInspector::start(active, &body, &key.verifying_key())
        .ok()
        .unwrap();
    for _ in 0..3 {
        assert!(!inspection.step(&mut work).unwrap());
        // Model handing the actual operation to the next Root service turn.
        inspection = *Box::new(inspection);
    }
    assert!(inspection.step(&mut work).unwrap());
    let (active, draft) = inspection.finish();
    assert_eq!(draft.unwrap().transcript(), expected);
    let owned = active.journal.into_storage();
    assert!(Arc::ptr_eq(&owned.0, &media));

    let (mut active, media, body, key) = fixture(2);
    active.recovered = false;
    let (mut active, _) = acceptance::OwnedInspector::start(active, &body, &key.verifying_key())
        .err()
        .unwrap();
    assert!(Arc::ptr_eq(
        &active.journal.inspect_storage(|s| s.0.clone()),
        &media
    ));
    active.recovered = true;
    let mut inspection = acceptance::OwnedInspector::start(active, &body, &key.verifying_key())
        .ok()
        .unwrap();
    assert!(!inspection.step(&mut work).unwrap());
    media.lock().unwrap().bytes.fill(0);
    assert!(inspection.step(&mut work).is_err());
    assert_eq!(inspection.step(&mut work), Err("custody inspection failed"));
    let (active, result) = inspection.finish();
    assert!(result.is_err());
    assert!(Arc::ptr_eq(
        &active.journal.inspect_storage(|s| s.0.clone()),
        &media
    ));
}

#[test]
fn accepted_endorsement_must_match_the_actual_inspected_closed_owner() {
    let (mut active, _, body, key) = fixture(2);
    let draft =
        acceptance::inspect(&mut active, &body, &key.verifying_key(), &mut [0; 536]).unwrap();
    let signature = key.sign(&draft.transcript()).to_bytes();
    let offer = draft.signed(signature);
    // Reinspect the same actual journal rather than constructing a draft from
    // the offered fields or the received endorsement.
    let draft =
        acceptance::inspect(&mut active, &body, &key.verifying_key(), &mut [0; 536]).unwrap();
    let mut accepted = [0; 416];
    accepted[..32].copy_from_slice(&offer.region.0);
    accepted[32..128].copy_from_slice(&offer.scope.0);
    accepted[128..160].copy_from_slice(&offer.forms.0);
    accepted[160..164].copy_from_slice(&offer.committed.to_le_bytes());
    accepted[164..196].copy_from_slice(&offer.digest.0);
    let endorse = |bytes: &mut [u8; 416]| {
        let signature = key.sign(&acceptance::wire::accepted_transcript(bytes));
        bytes[352..].copy_from_slice(&signature.to_bytes());
    };
    endorse(&mut accepted);
    assert!(draft.matches_accepted(&accepted, &body, &key.verifying_key()));
    // Every substitution is authentically signed by the pinned Root. A valid
    // endorsement for another epoch must still fail this owner's identity.
    for offset in [0, 32, 64, 96, 128, 160, 164] {
        let mut changed = accepted;
        changed[offset] ^= 1;
        endorse(&mut changed);
        assert!(
            !draft.matches_accepted(&changed, &body, &key.verifying_key()),
            "offset {offset}"
        );
    }
    let mut reserved = accepted;
    reserved[324] = 1;
    endorse(&mut reserved);
    assert!(!draft.matches_accepted(&reserved, &body, &key.verifying_key()));
    let mut tampered = accepted;
    tampered[228] ^= 1;
    assert!(!draft.matches_accepted(&tampered, &body, &key.verifying_key()));
    let other = ed25519_dalek::SigningKey::from_bytes(&[42; 32]);
    let mut foreign = accepted;
    foreign[352..].copy_from_slice(
        &other
            .sign(&acceptance::wire::accepted_transcript(&accepted))
            .to_bytes(),
    );
    assert!(!draft.matches_accepted(&foreign, &body, &other.verifying_key()));
    assert!(!draft.matches_accepted(&foreign, &body, &key.verifying_key()));
    assert!(!draft.matches_accepted(
        &accepted,
        &serde_json::from_value(serde_json::json!("03".repeat(32))).unwrap(),
        &key.verifying_key()
    ));
}
