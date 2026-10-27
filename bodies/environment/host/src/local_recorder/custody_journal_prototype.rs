//! CPU-only affine-media prototype. No target geometry/erase route is enabled.
use conduit_environment_runtime::{
    display_evidence_journal::{self as evidence, Storage},
    observed_record::{self, ObservedRecord},
};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_custody_journal.rs"]
mod custody;
#[allow(dead_code, clippy::duplicate_mod)] // Same actual recovery parser as the host history fixture.
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_recovery.rs"]
mod recovery;
#[allow(dead_code)]
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_state.rs"]
mod state;
#[allow(dead_code, clippy::duplicate_mod)] // Same actual producer wire as the recorder fixture.
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_wire.rs"]
mod wire;
use custody::{Custody, Error, WholeEpoch, ACCEPTANCE, DATA, SECTOR, TOTAL, WORK};
#[derive(Clone, Copy, Debug)]
enum Kind {
    Program,
    MarkerErase,
    DataErase,
}
#[derive(Clone, Copy)]
struct Operation {
    kind: Kind,
    length: usize,
    at: usize,
}
/// Clone is an independent power-cut flash snapshot, not another live handle.
#[derive(Clone)]
struct Media {
    bytes: Vec<u8>,
    identity: [u8; 32],
    closed: bool,
    fault: Option<(usize, usize)>,
    read_fault: Option<(usize, usize)>,
    operations: Vec<Operation>,
}
const GUARD: usize = SECTOR;
impl Media {
    fn new(identity: [u8; 32]) -> Self {
        let mut bytes = vec![255; TOTAL + 2 * GUARD];
        bytes[..GUARD].fill(0x55);
        bytes[GUARD + TOTAL..].fill(0xaa);
        Self {
            bytes,
            identity,
            closed: false,
            fault: None,
            read_fault: None,
            operations: vec![],
        }
    }
    fn write(&mut self, kind: Kind, at: usize, bytes: &[u8]) -> Result<(), Error> {
        assert!(at + bytes.len() <= TOTAL);
        let operation = self.operations.len();
        self.operations.push(Operation {
            kind,
            length: bytes.len(),
            at,
        });
        let cut = self
            .fault
            .filter(|(index, _)| *index == operation)
            .map(|(_, cut)| cut);
        let n = cut.unwrap_or(bytes.len()).min(bytes.len());
        for (output, input) in self.bytes[GUARD + at..GUARD + at + n].iter_mut().zip(bytes) {
            if matches!(kind, Kind::Program) {
                assert_eq!(*output & input, *input);
                *output &= input;
            } else {
                *output = 255;
            }
        }
        if cut.is_some() {
            Err(Error::Media)
        } else {
            Ok(())
        }
    }
    fn guards(&self) {
        assert!(self.bytes[..GUARD].iter().all(|b| *b == 0x55));
        assert!(self.bytes[GUARD + TOTAL..].iter().all(|b| *b == 0xaa));
    }
    fn snapshot(&self) -> Self {
        let mut value = self.clone();
        value.operations.clear();
        value.fault = None;
        value.read_fault = None;
        value
    }
}
impl WholeEpoch for Media {
    fn identity(&self) -> [u8; 32] {
        self.identity
    }
    fn read(&mut self, at: usize, out: &mut [u8]) -> Result<(), Error> {
        if self.read_fault == Some((self.operations.len(), at)) {
            self.read_fault = None;
            return Err(Error::Media);
        }
        out.copy_from_slice(
            self.bytes
                .get(GUARD + at..GUARD + at + out.len())
                .ok_or(Error::Media)?,
        );
        Ok(())
    }
    fn program_marker(&mut self, bank: u8, at: usize, bytes: &[u8]) -> Result<(), Error> {
        assert!(bank < 2 && at + bytes.len() <= SECTOR);
        self.write(Kind::Program, DATA + usize::from(bank) * SECTOR + at, bytes)
    }
    fn erase_marker(&mut self, bank: u8) -> Result<(), Error> {
        assert!(bank < 2);
        self.write(
            Kind::MarkerErase,
            DATA + usize::from(bank) * SECTOR,
            &[255; SECTOR],
        )
    }
    fn erase_data(&mut self, sector: u8) -> Result<(), Error> {
        assert!(usize::from(sector) < DATA / SECTOR);
        self.write(
            Kind::DataErase,
            usize::from(sector) * SECTOR,
            &[255; SECTOR],
        )
    }
}
impl Storage for Media {
    fn capacity(&self) -> u32 {
        DATA as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), evidence::Refusal> {
        if at as usize + out.len() > DATA {
            return Err(evidence::Refusal::Geometry);
        }
        WholeEpoch::read(self, at as usize, out).map_err(|_| evidence::Refusal::Media)
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), evidence::Refusal> {
        if at as usize + bytes.len() > DATA {
            return Err(evidence::Refusal::Geometry);
        }
        self.write(Kind::Program, at as usize, bytes)
            .map_err(|_| evidence::Refusal::Media)
    }
}
struct Borrowed<'a>(&'a mut Media);
impl Storage for Borrowed<'_> {
    fn capacity(&self) -> u32 {
        DATA as u32
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), evidence::Refusal> {
        Storage::read(self.0, at, out)
    }
    fn program(&mut self, _: u32, _: &[u8]) -> Result<(), evidence::Refusal> {
        Err(evidence::Refusal::Media)
    }
}
fn root_message(bytes: &[u8; ACCEPTANCE]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/accepted-epoch-custody@1\0");
    hash.update(&bytes[..352]);
    hash.finalize().into()
}
fn consumer_message(bytes: &[u8; ACCEPTANCE]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/epoch-custody@1\0");
    hash.update(&bytes[..196]);
    hash.update(&bytes[292..352]);
    hash.finalize().into()
}
fn authenticated(bytes: &[u8; ACCEPTANCE], key: &VerifyingKey) -> bool {
    key.verify_strict(
        &root_message(bytes),
        &ed25519_dalek::Signature::from_bytes(bytes[352..].try_into().unwrap()),
    )
    .is_ok()
}
fn closed(
    media: &mut Media,
    bytes: &[u8; ACCEPTANCE],
    basis: &[u8],
    key: &VerifyingKey,
) -> Result<(), Error> {
    if !media.closed {
        return Err(Error::NotClosed);
    }
    let mut work = [0; 536];
    let (mut journal, scope, count) =
        recovery::reopen_profile(Borrowed(media), basis, &mut work, true)
            .map_err(|_| Error::Corrupt)?;
    if count == 0
        || count > 960
        || scope != bytes[32..128]
        || Sha256::digest(basis)[..] != bytes[128..160]
        || count != u64::from(u32::from_le_bytes(bytes[160..164].try_into().unwrap()))
    {
        return Err(Error::NotClosed);
    }
    let mut cursor = journal.replay_cursor();
    let mut record = [0; observed_record::BYTES + 64];
    let mut hash = Sha256::new();
    while journal
        .replay(&mut cursor, &mut record, &mut work)
        .map_err(|_| Error::Corrupt)?
        .is_some()
    {
        key.verify_strict(
            &wire::record_transcript(
                &scope,
                bytes[128..160].try_into().unwrap(),
                &record[..observed_record::BYTES],
            ),
            &ed25519_dalek::Signature::from_bytes(
                record[observed_record::BYTES..].try_into().unwrap(),
            ),
        )
        .map_err(|_| Error::Authentication)?;
        hash.update(&record[..observed_record::BYTES]);
    }
    if hash.finalize()[..] != bytes[164..196] {
        return Err(Error::NotClosed);
    }
    Ok(())
}
struct Fixture {
    media: Media,
    accepted: [u8; ACCEPTANCE],
    key: SigningKey,
    basis: &'static [u8],
    directory: std::path::PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}
fn retain_actual_batch(
    directory: &std::path::Path,
    scope: [u8; 96],
    forms: [u8; 32],
    records: Vec<u8>,
    key: &SigningKey,
) -> [u8; 32] {
    use std::io::{Read, Write};
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let batch = |after, bytes: &[u8]| {
        serde_json::to_vec(&serde_json::json!({
            "profile":"observed-history@1","scope":hex(&scope),"forms":hex(&forms),
            "after":after,"committed":3,"records":hex(bytes),
            "signature":hex(&key.sign(&wire::transcript(&scope,&forms,after,3,bytes)).to_bytes()),
        }))
        .unwrap()
    };
    let first = batch(0, &records);
    let empty = batch(3, &[]);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let expected = first.clone();
    let worker = std::thread::spawn(move || {
        for (after, body) in [(0, first), (3, empty)] {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            assert!(String::from_utf8(request)
                .unwrap()
                .starts_with(&format!("GET /history?after={after} ")));
            socket
                .write_all(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n")
                .unwrap();
            socket.write_all(&body).unwrap();
        }
    });
    let custody = directory.join(format!("epoch-{}", hex(&scope)));
    let mut collector = crate::observed_history::Collector::from_authenticated_scope(
        scope,
        &key.verifying_key().to_bytes(),
    )
    .unwrap();
    collector.collect(address, &custody).unwrap();
    worker.join().unwrap();
    let mut restarted = crate::observed_history::Collector::from_authenticated_scope(
        scope,
        &key.verifying_key().to_bytes(),
    )
    .unwrap();
    restarted.restore(&custody).unwrap();
    assert_eq!(restarted.after, 3);
    let saved = std::fs::read(custody.join("observed-history-0000-0003.json")).unwrap();
    assert_eq!(saved, expected);
    let document: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    let text = document["records"].as_str().unwrap();
    let durable: Vec<u8> = (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect();
    Sha256::digest(durable).into()
}
fn fixture(label: &str) -> Fixture {
    use std::os::unix::fs::DirBuilderExt;
    let directory =
        std::env::temp_dir().join(format!("conduit-custody-{label}-{}", std::process::id()));
    if directory.exists() {
        std::fs::remove_dir_all(&directory).unwrap();
    }
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let (key, receipt, _) = super::tests::canonical_admission(&directory);
    let (evidence, manager): (
        conduit_body::BodyBiographyEvidence,
        conduit_body::AdmissionManager,
    ) = serde_json::from_slice(
        &std::fs::read(directory.join("canonical-body-checkpoint.json")).unwrap(),
    )
    .unwrap();
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let basis: &'static [u8] = Box::leak(Box::new(observed_record::descriptor(&contract).unwrap()));
    let forms: [u8; 32] = Sha256::digest(basis).into();
    let mut scope = [0; 96];
    scope[..32].copy_from_slice(&Sha256::digest(evidence.body_id.as_str().as_bytes()));
    scope[32..64].copy_from_slice(&Sha256::digest(receipt.producer_boot.as_str().as_bytes()));
    scope[64..].fill(4);
    let mut work = [0; 536];
    let erased = evidence::Erased::prepare_retained(Media::new([71; 32]))
        .ok()
        .unwrap();
    let mut journal =
        evidence::Journal::create_prepared(erased, &scope, basis, 960, &mut work).unwrap();
    let mut content = Sha256::new();
    let mut records = Vec::new();
    for sequence in 1..=3 {
        let record = ObservedRecord {
            sequence,
            observed_at_boot_ms: sequence * 100,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap();
        let mut signed = [0; observed_record::BYTES + 64];
        signed[..record.len()].copy_from_slice(&record);
        signed[record.len()..].copy_from_slice(
            &key.sign(&wire::record_transcript(&scope, &forms, &record))
                .to_bytes(),
        );
        journal.append(&signed, &mut work).unwrap();
        content.update(record);
        records.extend_from_slice(&record);
    }
    // Exercise actual generic journal rotation: old writer is frozen before its
    // affine storage is returned. This is not native sampler completion proof.
    let mut primary = state::State::Active(state::Active {
        journal,
        scope,
        forms,
        last_sequence: 3,
        failed: false,
        recovered: false,
        signed: true,
        maximum_records: 960,
    });
    let mut epochs = core::array::from_fn(|_| state::State::Failed);
    epochs[0] = state::State::Cold(state::Cold {
        region: evidence::Erased::prepare_retained(Media::new([72; 32]))
            .ok()
            .unwrap(),
        signed: true,
        basis,
        scope: None,
        capacity: DATA as u32,
        maximum_requested: 0,
    });
    let mut fresh = scope;
    fresh[64..].fill(5);
    state::rotate(&mut primary, &mut epochs, &scope, fresh, 960).unwrap();
    assert!(state::check_writer(&epochs[0], &[4; 32]).is_err());
    let state::State::Active(active) = core::mem::replace(&mut epochs[0], state::State::Failed)
    else {
        panic!("old journal")
    };
    assert!(active.recovered);
    let mut media = active.journal.into_storage();
    media.closed = true;
    media.operations.clear();
    // Actual canonical host collector fsyncs signed history before the consumer
    // signs any custody statement. Keep that directory until the test finishes.
    let durable_digest = retain_actual_batch(&directory, scope, forms, records, &key);
    assert_eq!(&durable_digest[..], &content.finalize()[..]);
    let mut accepted = [0; ACCEPTANCE];
    accepted[..32].copy_from_slice(&media.identity);
    accepted[32..128].copy_from_slice(&scope);
    accepted[128..160].copy_from_slice(&forms);
    accepted[160..164].copy_from_slice(&3u32.to_le_bytes());
    accepted[164..196].copy_from_slice(&durable_digest);
    let mut member = Sha256::new();
    for id in [
        receipt.credential.part_id.as_str(),
        receipt.credential.host_id.as_str(),
        receipt.credential.boot_id.as_str(),
    ] {
        member.update((id.len() as u64).to_le_bytes());
        member.update(id.as_bytes());
    }
    member.update(receipt.membership_revision.to_le_bytes());
    accepted[292..324].copy_from_slice(&member.finalize());
    let message = consumer_message(&accepted);
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([22; 32]).unwrap();
    let signature = secret.sign(&message);
    let verified = manager
        .verify_admitted_message(
            &evidence.membership,
            &receipt.credential.part_id,
            &receipt.credential.host_id,
            &receipt.credential.boot_id,
            &message,
            &signature,
        )
        .unwrap();
    assert_eq!(verified.transcript(), &message);
    assert_eq!(
        &accepted[32..64],
        &Sha256::digest(verified.body_id().as_str().as_bytes())[..]
    );
    accepted[196..228].copy_from_slice(verified.transcript());
    accepted[228..292].copy_from_slice(&signature);
    closed(&mut media, &accepted, basis, &key.verifying_key()).unwrap();
    let endorsed = key.sign(&root_message(&accepted)).to_bytes();
    accepted[352..].copy_from_slice(&endorsed);
    Fixture {
        media,
        accepted,
        key,
        basis,
        directory,
    }
}
#[allow(clippy::result_large_err)] // Return the same affine model owner without a new allocation.
fn run(media: Media, f: &Fixture) -> Result<custody::ErasedData<Media>, (Custody<Media>, Error)> {
    let mut work = [0; WORK];
    let key = f.key.verifying_key();
    let controller = Custody::open(media, &mut work, |a| authenticated(a, &key))
        .ok()
        .unwrap();
    controller.begin(
        &f.accepted,
        &mut work,
        |a| authenticated(a, &key),
        |m, a| closed(m, a, f.basis, &key),
    )
}
fn recover(mut captured: Media, f: &Fixture) -> Media {
    captured.guards();
    captured.fault = None;
    captured.read_fault = None;
    captured.operations.clear();
    let mut work = [0; WORK];
    let key = f.key.verifying_key();
    match Custody::open(captured, &mut work, |a| authenticated(a, &key)) {
        Err((preserved, Error::UnknownMetadata)) => {
            assert_eq!(
                &preserved.bytes[GUARD..GUARD + DATA],
                &f.media.bytes[GUARD..GUARD + DATA]
            );
            preserved
        }
        Ok(controller) => match controller.resume(&mut work) {
            Ok(erased) => erased.into_storage(),
            Err((controller, Error::Phase)) => match controller.release() {
                Ok(erased) => erased.into_storage(),
                Err((controller, Error::Phase)) => {
                    let unchanged = controller.into_storage();
                    assert_eq!(
                        &unchanged.bytes[GUARD..GUARD + DATA],
                        &f.media.bytes[GUARD..GUARD + DATA]
                    );
                    unchanged
                }
                Err((_, error)) => panic!("release {error:?}"),
            },
            Err((_, error)) => panic!("resume {error:?}"),
        },
        Err((_, error)) => panic!("recovery {error:?}"),
    }
}
#[test]
fn admitted_closed_signed_journal_reclaim_and_every_marker_program_byte_cut_preserve_custody() {
    let f = fixture("cuts");
    let finished = run(f.media.snapshot(), &f).ok().unwrap().into_storage();
    let operations = finished.operations.clone();
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(op.kind, Kind::DataErase))
            .count(),
        DATA / SECTOR
    );
    let mut tested = 0;
    for (index, operation) in operations.iter().enumerate() {
        let cuts: Vec<usize> = if matches!(operation.kind, Kind::Program) {
            (0..=operation.length).collect()
        } else {
            vec![0, 1, 3, 4, 511, 512, operation.length - 1, operation.length]
        };
        for cut in cuts {
            let mut media = f.media.snapshot();
            media.fault = Some((index, cut));
            let (failed, _) = run(media, &f).err().expect("injected uncertain outcome");
            recover(failed.into_storage(), &f).guards();
            tested += 1;
        }
    }
    assert!(tested > 1000);
    println!(
        "closed custody cuts verified={tested}; data sectors={}",
        DATA / SECTOR
    );
}
#[test]
fn foreign_marker_and_active_writer_refuse_and_old_reclaimed_marker_never_erases_later_data() {
    let f = fixture("fences");
    let mut active = f.media.snapshot();
    active.closed = false;
    let (refused, error) = run(active, &f).err().unwrap();
    assert_eq!(error, Error::NotClosed);
    let unchanged = refused.into_storage();
    assert!(unchanged.operations.is_empty());
    let mut finished = run(f.media.snapshot(), &f).ok().unwrap().into_storage();
    finished.guards();
    // Identical Root-signed marker copied to another physical epoch refuses.
    let mut foreign = finished.snapshot();
    foreign.identity = [99; 32];
    let mut work = [0; WORK];
    let key = f.key.verifying_key();
    let (_, error) = Custody::open(foreign, &mut work, |a| authenticated(a, &key))
        .err()
        .unwrap();
    assert_eq!(error, Error::Region);
    // Later torn/new-scope bytes cannot be erased from an old Reclaimed ACK.
    finished.bytes[GUARD + 64] = 0;
    finished.operations.clear();
    let controller = Custody::open(finished, &mut work, |a| authenticated(a, &key))
        .ok()
        .unwrap();
    let (controller, error) = controller.resume(&mut work).err().unwrap();
    assert_eq!(error, Error::Phase);
    let (controller, error) = controller.release().err().unwrap();
    assert_eq!(error, Error::Occupied);
    assert!(controller.into_storage().operations.is_empty());
}

#[test]
fn applied_commit_and_each_phase_readback_refusal_recover_without_premature_release() {
    let f = fixture("readbacks");
    let finished = run(f.media.snapshot(), &f).ok().unwrap().into_storage();
    let mut tested = 0;
    for (index, operation) in finished.operations.iter().enumerate() {
        let offset = if matches!(operation.kind, Kind::Program | Kind::MarkerErase) {
            Some(operation.at)
        } else if matches!(operation.kind, Kind::DataErase) && operation.at + SECTOR == DATA {
            Some(0)
        } else {
            None
        };
        if let Some(offset) = offset {
            let mut media = f.media.snapshot();
            media.read_fault = Some((index + 1, offset));
            let (failed, error) = run(media, &f)
                .err()
                .expect("applied mutation, uncertain readback");
            assert_eq!(error, Error::Media);
            recover(failed.into_storage(), &f).guards();
            tested += 1;
        }
    }
    assert_eq!(tested, 7);
}
#[test]
fn retained_root_endorsement_does_not_replace_exact_closed_scope_count_and_digest_checks() {
    let f = fixture("extent-fences");
    let key = f.key.verifying_key();
    for at in [32, 64, 96, 128, 160, 164] {
        let mut wrong = f.accepted;
        wrong[at] ^= 1;
        let signature = f.key.sign(&root_message(&wrong)).to_bytes();
        wrong[352..].copy_from_slice(&signature);
        let mut work = [0; WORK];
        let controller = Custody::open(f.media.snapshot(), &mut work, |a| authenticated(a, &key))
            .ok()
            .unwrap();
        let (refused, error) = controller
            .begin(
                &wrong,
                &mut work,
                |a| authenticated(a, &key),
                |m, a| closed(m, a, f.basis, &key),
            )
            .err()
            .unwrap();
        assert_eq!(error, Error::NotClosed);
        let media = refused.into_storage();
        assert!(media.operations.is_empty());
        assert_eq!(media.bytes, f.media.bytes);
    }
    let mut forged = f.accepted;
    forged[228] ^= 1;
    let mut work = [0; WORK];
    let controller = Custody::open(f.media.snapshot(), &mut work, |a| authenticated(a, &key))
        .ok()
        .unwrap();
    let (refused, error) = controller
        .begin(
            &forged,
            &mut work,
            |a| authenticated(a, &key),
            |m, a| closed(m, a, f.basis, &key),
        )
        .err()
        .unwrap();
    assert_eq!(error, Error::Authentication);
    assert!(refused.into_storage().operations.is_empty());
}
fn successor(mut media: Media, f: &Fixture, number: u8) -> (Media, [u8; ACCEPTANCE]) {
    // Model another actual signed journal on returned erased data. This is not
    // a native Root lifecycle/Boot proof; the real generic writer-freeze path
    // still must transfer the journal before accepting its custody statement.
    media.closed = false;
    media.operations.clear();
    let mut scope: [u8; 96] = f.accepted[32..128].try_into().unwrap();
    if number >= 16 {
        scope[32..64].copy_from_slice(&Sha256::digest(
            format!("producer/boot-{number}").as_bytes(),
        ));
    }
    scope[64..].copy_from_slice(&Sha256::digest(
        format!("producer/play-{number}").as_bytes(),
    ));
    let forms: [u8; 32] = f.accepted[128..160].try_into().unwrap();
    let mut work = [0; 536];
    let erased = evidence::Erased::prepare_retained(media).ok().unwrap();
    let mut journal =
        evidence::Journal::create_prepared(erased, &scope, f.basis, 960, &mut work).unwrap();
    let mut records = Vec::new();
    for sequence in 1..=3 {
        let record = ObservedRecord {
            sequence,
            observed_at_boot_ms: sequence * 100,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap();
        let mut signed = [0; observed_record::BYTES + 64];
        signed[..record.len()].copy_from_slice(&record);
        signed[record.len()..].copy_from_slice(
            &f.key
                .sign(&wire::record_transcript(&scope, &forms, &record))
                .to_bytes(),
        );
        journal.append(&signed, &mut work).unwrap();
        records.extend_from_slice(&record);
    }
    let mut primary = state::State::Active(state::Active {
        journal,
        scope,
        forms,
        last_sequence: 3,
        failed: false,
        recovered: false,
        signed: true,
        maximum_records: 960,
    });
    let mut epochs = core::array::from_fn(|_| state::State::Failed);
    epochs[0] = state::State::Cold(state::Cold {
        region: evidence::Erased::prepare_retained(Media::new([72; 32]))
            .ok()
            .unwrap(),
        signed: true,
        basis: f.basis,
        scope: None,
        capacity: DATA as u32,
        maximum_requested: 0,
    });
    let mut fresh = scope;
    fresh[64] ^= 1;
    state::rotate(&mut primary, &mut epochs, &scope, fresh, 960).unwrap();
    assert!(state::check_writer(&epochs[0], scope[64..].try_into().unwrap()).is_err());
    let state::State::Active(active) = core::mem::replace(&mut epochs[0], state::State::Failed)
    else {
        panic!("closed journal")
    };
    let mut media = active.journal.into_storage();
    media.closed = active.recovered;
    media.operations.clear();
    let digest = retain_actual_batch(&f.directory, scope, forms, records, &f.key);
    let mut accepted = f.accepted;
    accepted[32..128].copy_from_slice(&scope);
    accepted[164..196].copy_from_slice(&digest);
    let (evidence, manager): (
        conduit_body::BodyBiographyEvidence,
        conduit_body::AdmissionManager,
    ) = serde_json::from_slice(
        &std::fs::read(f.directory.join("canonical-body-checkpoint.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manager.receipts.len(), 1);
    let credential = &manager.receipts[0].credential;
    let message = consumer_message(&accepted);
    let secret = conduit_body::SpawnInvitationSecret::from_csprng_bytes([22; 32]).unwrap();
    let signature = secret.sign(&message);
    let verified = manager
        .verify_admitted_message(
            &evidence.membership,
            &credential.part_id,
            &credential.host_id,
            &credential.boot_id,
            &message,
            &signature,
        )
        .unwrap();
    assert_eq!(
        &accepted[32..64],
        &Sha256::digest(verified.body_id().as_str().as_bytes())[..]
    );
    accepted[196..228].copy_from_slice(verified.transcript());
    accepted[228..292].copy_from_slice(&signature);
    closed(&mut media, &accepted, f.basis, &f.key.verifying_key()).unwrap();
    let signature = f.key.sign(&root_message(&accepted)).to_bytes();
    accepted[352..].copy_from_slice(&signature);
    (media, accepted)
}
#[test]
fn marker_banks_recycle_across_many_signed_scopes_and_malformed_latest_never_falls_back() {
    let f = fixture("recycle");
    let key = f.key.verifying_key();
    let mut work = [0; WORK];
    let mut media = run(f.media.snapshot(), &f).ok().unwrap().into_storage();
    for number in 0..32 {
        let (next, accepted) = successor(media, &f, number);
        let original = next.bytes.clone();
        let controller = Custody::open(next, &mut work, |a| authenticated(a, &key))
            .ok()
            .unwrap();
        // Original ACK cannot relabel even a same-Boot/new-Play journal.
        let (controller, error) = controller
            .begin(
                &f.accepted,
                &mut work,
                |a| authenticated(a, &key),
                |m, a| closed(m, a, f.basis, &key),
            )
            .err()
            .unwrap();
        assert_eq!(error, Error::NotClosed);
        media = controller
            .begin(
                &accepted,
                &mut work,
                |a| authenticated(a, &key),
                |m, a| closed(m, a, f.basis, &key),
            )
            .ok()
            .unwrap()
            .into_storage();
        media.guards();
        assert!(media.bytes[GUARD..GUARD + DATA].iter().all(|b| *b == 255));
        assert_ne!(media.bytes, original);
        let generation = u64::from_le_bytes(
            media.bytes[GUARD + DATA + SECTOR + 8..GUARD + DATA + SECTOR + 16]
                .try_into()
                .unwrap(),
        );
        assert_eq!(generation, 2 * (u64::from(number) + 2));
    }
    // Corrupt the authoritative Reclaimed marker while the older Reclaiming
    // remains valid. Never fall back and erase later/new-scope dirty data.
    media.bytes[GUARD + 64] = 0;
    media.bytes[GUARD + DATA + SECTOR + 40] ^= 1;
    media.operations.clear();
    let before = media.bytes.clone();
    let (preserved, error) = Custody::open(media, &mut work, |a| authenticated(a, &key))
        .err()
        .unwrap();
    assert_eq!(error, Error::Corrupt);
    assert!(preserved.operations.is_empty());
    assert_eq!(preserved.bytes, before);
    preserved.guards();
}
#[test]
fn applied_commit_error_can_retry_with_the_same_affine_controller() {
    let f = fixture("same-owner");
    let baseline = run(f.media.snapshot(), &f).ok().unwrap().into_storage();
    let key = f.key.verifying_key();
    let commits: Vec<_> = baseline
        .operations
        .iter()
        .enumerate()
        .filter(|(_, op)| matches!(op.kind, Kind::Program) && op.length == 4)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(commits.len(), 2);
    for (phase, index) in commits.into_iter().enumerate() {
        let mut media = f.media.snapshot();
        media.fault = Some((index, 4));
        let (controller, error) = run(media, &f).err().unwrap();
        assert_eq!(error, Error::Media);
        // The fault targets that exact operation once. Retry moves the SAME
        // controller forward, with no raw-media or new ownership extraction.
        let mut work = [0; WORK];
        let erased = if phase == 0 {
            controller
                .begin(
                    &f.accepted,
                    &mut work,
                    |a| authenticated(a, &key),
                    |m, a| closed(m, a, f.basis, &key),
                )
                .ok()
                .unwrap()
        } else {
            controller.resume(&mut work).ok().unwrap()
        };
        let media = erased.into_storage();
        media.guards();
        assert!(media.bytes[GUARD..GUARD + DATA].iter().all(|b| *b == 255));
    }
}
fn reframe_test_marker(media: &mut Media, bank: usize, generation: u64, phase: u32) {
    // Fault-injection only: retain the actual Root-endorsed acceptance while
    // placing the media counter at its finite boundary, without minting keys.
    let bytes = &mut media.bytes[GUARD + DATA + bank * SECTOR..GUARD + DATA + bank * SECTOR + 448];
    bytes[8..16].copy_from_slice(&generation.to_le_bytes());
    bytes[16..20].copy_from_slice(&phase.to_le_bytes());
    let mut crc = !0u32;
    for byte in &bytes[..440] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    bytes[440..444].copy_from_slice(&(!crc).to_le_bytes());
}
#[test]
fn generation_boundary_reserves_both_phases_and_refuses_without_reset_or_erase() {
    let f = fixture("generation");
    let key = f.key.verifying_key();
    let mut work = [0; WORK];
    let finished = run(f.media.snapshot(), &f).ok().unwrap().into_storage();
    let (mut next, accepted) = successor(finished, &f, 2);
    next.bytes[GUARD + DATA..GUARD + DATA + SECTOR].fill(255);
    reframe_test_marker(&mut next, 1, u64::MAX - 1, 2);
    let original = next.bytes.clone();
    let controller = Custody::open(next, &mut work, |a| authenticated(a, &key))
        .ok()
        .unwrap();
    let (refused, error) = controller
        .begin(
            &accepted,
            &mut work,
            |a| authenticated(a, &key),
            |m, a| closed(m, a, f.basis, &key),
        )
        .err()
        .unwrap();
    assert_eq!(error, Error::Exhausted);
    let media = refused.into_storage();
    assert!(media.operations.is_empty());
    assert_eq!(media.bytes, original);
    let mut reclaiming = run(f.media.snapshot(), &f).ok().unwrap().into_storage();
    reclaiming.bytes[GUARD..GUARD + DATA].copy_from_slice(&f.media.bytes[GUARD..GUARD + DATA]);
    reclaiming.bytes[GUARD + DATA..GUARD + DATA + SECTOR].fill(255);
    reframe_test_marker(&mut reclaiming, 1, u64::MAX, 1);
    reclaiming.operations.clear();
    let original = reclaiming.bytes.clone();
    let controller = Custody::open(reclaiming, &mut work, |a| authenticated(a, &key))
        .ok()
        .unwrap();
    let (refused, error) = controller.resume(&mut work).err().unwrap();
    assert_eq!(error, Error::Exhausted);
    let media = refused.into_storage();
    assert!(media.operations.is_empty());
    assert_eq!(media.bytes, original);
    media.guards();
}
