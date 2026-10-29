use super::*;
use ed25519_dalek::{Signer, SigningKey};
fn signed(
    key: &SigningKey,
    scope: [u8; 96],
    forms: [u8; 32],
    after: u32,
    committed: u32,
    records: &[u8],
) -> Vec<u8> {
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    serde_json::to_vec(&serde_json::json!({"profile":"observed-history@1","scope":hex(&scope),"forms":hex(&forms),"after":after,"committed":committed,"records":hex(records),"signature":hex(&key.sign(&wire::transcript(&scope,&forms,after,committed,records)).to_bytes())})).unwrap()
}
#[test]
fn replay_requires_exact_scope_key_cursor_forms_and_continuous_records() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let collector = Collector {
        scope: [3; 96],
        forms: [4; 32],
        key: key.verifying_key(),
        after: 0,
        observed_ms: 0,
    };
    let record = ObservedRecord {
        sequence: 1,
        observed_at_boot_ms: 123,
        disposition: conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
        quantities: None,
    }
    .encode()
    .unwrap();
    let good = signed(&key, collector.scope, collector.forms, 0, 1, &record);
    assert_eq!(collector.validate(&good).unwrap(), (1, 123));
    assert!(collector
        .validate(&signed(&key, [8; 96], collector.forms, 0, 1, &record))
        .is_err());
    assert!(collector
        .validate(&signed(&key, collector.scope, [8; 32], 0, 1, &record))
        .is_err());
    assert!(collector
        .validate(&signed(
            &SigningKey::from_bytes(&[8; 32]),
            collector.scope,
            collector.forms,
            0,
            1,
            &record
        ))
        .is_err());
    assert!(collector
        .validate(&signed(&key, collector.scope, collector.forms, 1, 1, &[]))
        .is_err());
    assert!(collector
        .validate(&signed(&key, collector.scope, collector.forms, 0, 1, &[]))
        .is_err());
    let mut skipped = record;
    skipped[..8].copy_from_slice(&2u64.to_le_bytes());
    assert!(collector
        .validate(&signed(
            &key,
            collector.scope,
            collector.forms,
            0,
            1,
            &skipped
        ))
        .is_err());
    let mut altered: serde_json::Value = serde_json::from_slice(&good).unwrap();
    altered["committed"] = 2.into();
    assert!(collector
        .validate(&serde_json::to_vec(&altered).unwrap())
        .is_err());
    let later = Collector {
        after: 1,
        observed_ms: 200,
        ..collector
    };
    assert!(later
        .validate(&signed(&key, later.scope, later.forms, 1, 2, &skipped))
        .is_err());
}

#[test]
fn verified_history_fills_face_gap_without_changing_observation_clock() {
    use conduit_environment_runtime::{
        observation::{ObservationContract, PreparedSourceFrameDecoder},
        publication::{PublicationChannel, PublicationSnapshot},
        recording::RecordingDisposition,
        retained_publication_face::RetainedPublicationFace,
    };
    let key = SigningKey::from_bytes(&[19; 32]);
    let collector = Collector {
        scope: [3; 96],
        forms: [4; 32],
        key: key.verifying_key(),
        after: 3,
        observed_ms: 250,
    };
    let record = |sequence, observed_at_boot_ms| ObservedRecord {
        sequence,
        observed_at_boot_ms,
        disposition: RecordingDisposition::Available,
        quantities: Some((
            conduit_core::Quantity::new(20, conduit_core::Unit::Celsius),
            conduit_core::Quantity::new(59, conduit_core::Unit::Percent),
        )),
    };
    let mut face = RetainedPublicationFace::prepare(
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("fixture/source".into()),
            checked_plot_id: Some("fixture/checked".into()),
            expanded_plot_id: Some("fixture/expanded".into()),
            plan_id: Some("fixture/plan".into()),
            active_play_id: Some("fixture/play".into()),
            sign_ids: vec![],
        },
        960,
        120000,
    )
    .unwrap();
    face.publish_projected(&record(1, 100), 100).unwrap();
    face.at(170).unwrap();
    let directory =
        std::env::temp_dir().join(format!("conduit-history-gap-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let records = [
        record(1, 100).encode().unwrap(),
        record(2, 160).encode().unwrap(),
        record(3, 250).encode().unwrap(),
    ]
    .concat();
    std::fs::write(
        directory.join("observed-history-0000-0003.json"),
        signed(&key, collector.scope, collector.forms, 0, 3, &records),
    )
    .unwrap();
    let interrupted = directory.join("observed-history-0000-0001.pending-old");
    std::fs::write(&interrupted, b"torn").unwrap();
    collector.replay_gap(&directory, &mut face, 3, 300).unwrap();
    std::fs::remove_file(interrupted).unwrap();
    assert_eq!(face.last_source_sequence(), 2);
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    let canonical = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
    face.publish_observed(
        PublicationSnapshot {
            sequence: 3,
            channel: PublicationChannel::Reading,
            canonical,
        },
        250,
        300,
    )
    .unwrap();
    assert_eq!(face.last_source_sequence(), 3);
    let observed = face
        .current()
        .properties
        .iter()
        .find(|p| p.name == "observed-at-boot-ms")
        .unwrap();
    assert_eq!(observed.value, conduit_face::FacePropertyValue::Count(250));
    assert!(face.publish_projected(&record(4, 400), 350).is_err());
    assert_eq!(face.last_source_sequence(), 3);
    std::fs::remove_file(directory.join("observed-history-0000-0003.json")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn durable_restart_revalidates_prefix_and_ignores_interrupted_temporary_write() {
    let key = SigningKey::from_bytes(&[21; 32]);
    let mut collector =
        Collector::from_authenticated_scope([3; 96], &key.verifying_key().to_bytes()).unwrap();
    let directory =
        std::env::temp_dir().join(format!("conduit-history-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).unwrap();
    let record = ObservedRecord {
        sequence: 1,
        observed_at_boot_ms: 731,
        disposition: conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
        quantities: None,
    }
    .encode()
    .unwrap();
    let bytes = signed(&key, collector.scope, collector.forms, 0, 1, &record);
    let path = directory.join("observed-history-0000-0001.json");
    std::fs::write(path.with_extension("pending-old"), b"torn").unwrap();
    persist_batch(&path, &bytes).unwrap();
    persist_batch(&path, &bytes).unwrap();
    assert!(persist_batch(&path, b"different").is_err());
    collector.restore(&directory).unwrap();
    assert_eq!((collector.after, collector.observed_ms), (1, 731));
    // A foreign or corrupt later receipt cannot partially advance restart state.
    std::fs::write(directory.join("observed-history-0001-0002.json"), b"torn").unwrap();
    assert!(collector.restore(&directory).is_err());
    assert_eq!((collector.after, collector.observed_ms), (1, 731));
    let mut foreign =
        Collector::from_authenticated_scope([4; 96], &key.verifying_key().to_bytes()).unwrap();
    assert!(foreign.restore(&directory).is_err());
    assert_eq!(foreign.after, 0);
    std::fs::remove_dir_all(directory).unwrap();
}

#[allow(dead_code)] // Shared target module includes recovery paths outside this fixture.
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_recovery.rs"]
mod native_recovery;

#[test]
fn prior_boot_outbox_reopens_exact_committed_prefix_without_programming() {
    use conduit_environment_runtime::display_evidence_journal::{
        Erased, Journal, Refusal, Storage,
    };
    #[derive(Clone)]
    struct Media(std::rc::Rc<std::cell::RefCell<(Vec<u8>, usize)>>);
    impl Storage for Media {
        fn capacity(&self) -> u32 {
            self.0.borrow().0.len() as u32
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> std::result::Result<(), Refusal> {
            out.copy_from_slice(&self.0.borrow().0[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> std::result::Result<(), Refusal> {
            let mut media = self.0.borrow_mut();
            media.1 += 1;
            for (out, byte) in media.0[at as usize..at as usize + bytes.len()]
                .iter_mut()
                .zip(bytes)
            {
                assert_eq!(*out & byte, *byte);
                *out &= byte;
            }
            Ok(())
        }
    }
    let media = Media(std::rc::Rc::new(std::cell::RefCell::new((
        vec![255; 65536],
        0,
    ))));
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let basis = observed_record::descriptor(&contract).unwrap();
    let mut work = [0; 536];
    let mut journal = Journal::create_prepared(
        Erased::prepare_retained(media.clone()).ok().unwrap(),
        &[9; 96],
        &basis,
        960,
        &mut work,
    )
    .unwrap();
    let record = ObservedRecord {
        sequence: 1,
        observed_at_boot_ms: 919,
        disposition: conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
        quantities: None,
    }
    .encode()
    .unwrap();
    journal.append(&record, &mut work).unwrap();
    let extent = journal.retained_extent();
    drop(journal);
    // A power interruption has programmed an uncommitted next record prefix.
    media.0.borrow_mut().0[extent as usize] = b'J';
    let writes = media.0.borrow().1;
    let (mut recovered, scope, sequence) =
        native_recovery::reopen(media.clone(), &basis, &mut work).unwrap();
    assert_eq!(scope, [9; 96]);
    assert_eq!(sequence, 1);
    assert!(recovered.interrupted_tail());
    let mut cursor = recovered.replay_cursor();
    let mut out = [0; observed_record::BYTES];
    assert_eq!(
        recovered.replay(&mut cursor, &mut out, &mut work).unwrap(),
        Some(out.len())
    );
    assert_eq!(out, record);
    assert!(recovered
        .replay(&mut cursor, &mut out, &mut work)
        .unwrap()
        .is_none());
    assert_eq!(media.0.borrow().1, writes);
}

#[allow(dead_code)] // Shared target module also supports streamed checkpoint rewrites.
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_body_checkpoint.rs"]
mod checkpoint;

#[test]
fn complete_body_and_private_seed_survive_each_successor_power_cut() {
    use checkpoint::{Checkpoint, Refusal, Storage};
    #[derive(Clone)]
    struct Media {
        bytes: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
        budget: Option<usize>,
        fail_after: bool,
    }
    impl Media {
        fn operation(&mut self) -> std::result::Result<(), Refusal> {
            if let Some(left) = self.budget.as_mut() {
                if *left == 0 {
                    return Err(Refusal::Storage);
                }
                *left -= 1;
            }
            Ok(())
        }
    }
    impl Storage for Media {
        fn capacity(&self) -> u32 {
            self.bytes.borrow().len() as u32
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> std::result::Result<(), Refusal> {
            assert_eq!(at % 4, 0);
            assert_eq!(out.len() % 4, 0);
            out.copy_from_slice(&self.bytes.borrow()[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> std::result::Result<(), Refusal> {
            assert_eq!(at % 4, 0);
            assert_eq!(bytes.len() % 4, 0);
            // Interrupt at each physical word, including the final commit word.
            for (i, word) in bytes.chunks(4).enumerate() {
                self.operation()?;
                let mut media = self.bytes.borrow_mut();
                for (old, new) in media[at as usize + i * 4..at as usize + i * 4 + 4]
                    .iter_mut()
                    .zip(word)
                {
                    assert_eq!(*old & new, *new);
                    *old &= new;
                }
                if self.fail_after && self.budget == Some(0) {
                    return Err(Refusal::Storage);
                }
            }
            Ok(())
        }
        fn erase_bank(&mut self, bank: u8) -> std::result::Result<(), Refusal> {
            for sector in 0..8 {
                self.operation()?;
                let start = usize::from(bank) * 32768 + sector * 4096;
                self.bytes.borrow_mut()[start..start + 4096].fill(255);
                if self.fail_after && self.budget == Some(0) {
                    return Err(Refusal::Storage);
                }
            }
            Ok(())
        }
    }

    let original = Media {
        bytes: std::rc::Rc::new(std::cell::RefCell::new(vec![255; 65536])),
        budget: None,
        fail_after: false,
    };
    let mut journal = Checkpoint::open(original.clone()).unwrap();
    assert_eq!(journal.generation(), None);
    assert_eq!(journal.payload_length(&[3; 32]).unwrap(), None);
    journal
        .save(&[3; 32], &[6; 32], b"oldest retired biography")
        .unwrap();
    journal
        .save(&[3; 32], &[7; 32], b"complete previous mutable biography")
        .unwrap();
    let baseline = original.bytes.borrow().clone();
    // Every sector erase and physical program word, including commit last.
    for (cut, fail_after) in (0..=64).flat_map(|cut| [(cut, false), (cut, true)]) {
        let media = Media {
            bytes: std::rc::Rc::new(std::cell::RefCell::new(baseline.clone())),
            budget: Some(cut),
            fail_after,
        };
        let mut journal = Checkpoint::open(media.clone()).unwrap();
        let saved = journal
            .save(&[3; 32], &[8; 32], b"complete successor mutable biography")
            .is_ok();
        let mut reopened = Checkpoint::open(Media {
            budget: None,
            ..media
        })
        .unwrap();
        let mut seed = [0; 32];
        let mut evidence = [0; 128];
        let n = reopened
            .load(&[3; 32], &mut seed, &mut evidence)
            .unwrap()
            .unwrap();
        let mut streamed_seed = [99; 32];
        let streamed = reopened
            .load_streamed(&[3; 32], &mut streamed_seed, |reader| {
                let mut bytes = Vec::new();
                let mut chunk = [0; 128];
                loop {
                    let count = reader.read(&mut chunk)?;
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                }
                chunk.fill(0);
                Ok(bytes)
            })
            .unwrap()
            .unwrap();
        assert_eq!(streamed, &evidence[..n]);
        assert_eq!(streamed_seed, seed);
        if saved {
            assert_eq!(reopened.generation(), Some(3));
        }
        if reopened.generation() == Some(3) {
            assert_eq!(&evidence[..n], b"complete successor mutable biography");
            assert_eq!(seed, [8; 32]);
        } else {
            assert_eq!(reopened.generation(), Some(2));
            assert_eq!(&evidence[..n], b"complete previous mutable biography");
            assert_eq!(seed, [7; 32]);
        }
        let mut untouched = [99; 32];
        assert!(reopened
            .load(&[4; 32], &mut untouched, &mut evidence)
            .is_err());
        assert_eq!(untouched, [99; 32]);
        assert!(reopened
            .load_streamed(&[4; 32], &mut streamed_seed, |_| Ok(()))
            .is_err());
        assert_eq!(streamed_seed, [0; 32]);
    }
    let complete = vec![23; 32768 - 160];
    journal.save(&[3; 32], &[9; 32], &complete).unwrap();
    let mut output = vec![0; complete.len()];
    let mut seed = [0; 32];
    assert_eq!(
        journal.load(&[3; 32], &mut seed, &mut output).unwrap(),
        Some(complete.len())
    );
    assert_eq!(output, complete);
    assert_eq!(seed, [9; 32]);
    let count = journal
        .load_streamed(&[3; 32], &mut seed, |reader| {
            let mut total = 0;
            let mut chunk = [0; 128];
            loop {
                let count = reader.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                assert!(chunk[..count].iter().all(|byte| *byte == 23));
                total += count;
            }
            chunk.fill(0);
            Ok(total)
        })
        .unwrap()
        .unwrap();
    assert_eq!(count, complete.len());
    assert_eq!(seed, [9; 32]);
    let oversized = vec![0; 32768 - 159];
    assert!(journal.save(&[3; 32], &[9; 32], &oversized).is_err());
    assert!(journal.save(&[3; 32], &[0; 32], b"body").is_err());
    assert_eq!(journal.generation(), Some(3));
}

#[test]
fn recorder_reconnects_from_durable_cursor_after_network_and_process_interruption() {
    fn serve(batches: Vec<(u32, Vec<u8>)>) -> (SocketAddr, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            for (after, bytes) in batches {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = [0; 512];
                let mut n = 0;
                while n < request.len() && !request[..n].ends_with(b"\r\n\r\n") {
                    let read = socket.read(&mut request[n..n + 1]).unwrap();
                    assert_ne!(read, 0);
                    n += read;
                }
                assert!(std::str::from_utf8(&request[..n])
                    .unwrap()
                    .starts_with(&format!("GET /history?after={after} ")));
                socket
                    .write_all(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n")
                    .unwrap();
                socket.write_all(&bytes).unwrap();
            }
        });
        (address, worker)
    }
    let key = SigningKey::from_bytes(&[22; 32]);
    let mut collector =
        Collector::from_authenticated_scope([5; 96], &key.verifying_key().to_bytes()).unwrap();
    let directory =
        std::env::temp_dir().join(format!("conduit-history-reconnect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let record = |sequence, clock| {
        ObservedRecord {
            sequence,
            observed_at_boot_ms: clock,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap()
    };
    let (address, worker) = serve(vec![
        (
            0,
            signed(&key, collector.scope, collector.forms, 0, 1, &record(1, 10)),
        ),
        (1, signed(&key, collector.scope, collector.forms, 1, 1, &[])),
    ]);
    collector.collect(address, &directory).unwrap();
    worker.join().unwrap();
    assert!(collector.collect(address, &directory).is_err());
    assert_eq!(collector.after, 1);
    let mut restarted =
        Collector::from_authenticated_scope([5; 96], &key.verifying_key().to_bytes()).unwrap();
    let (address, worker) = serve(vec![
        (
            1,
            signed(&key, restarted.scope, restarted.forms, 1, 2, &record(2, 20)),
        ),
        (2, signed(&key, restarted.scope, restarted.forms, 2, 2, &[])),
    ]);
    restarted.collect(address, &directory).unwrap();
    worker.join().unwrap();
    assert_eq!((restarted.after, restarted.observed_ms), (2, 20));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn root_signed_epoch_directory_delegates_only_same_body_forms_and_unique_scopes() {
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let key = SigningKey::from_bytes(&[7; 32]);
    let scope = [3; 96];
    let forms = [4; 32];
    let collector = Collector {
        scope,
        forms,
        key: key.verifying_key(),
        after: 0,
        observed_ms: 0,
    };
    let mut directory = [0; wire::DIRECTORY_BYTES];
    directory[..32].copy_from_slice(&scope[..32]);
    directory[32] = 1;
    directory[33] = 2;
    directory[34..130].copy_from_slice(&scope);
    directory[130..162].copy_from_slice(&forms);
    directory[162..166].copy_from_slice(&12u32.to_le_bytes());
    let document = |bytes: &[u8; wire::DIRECTORY_BYTES], signer: &SigningKey| {
        serde_json::to_vec(
            &serde_json::json!({"profile":"observed-epochs@1", "directory":hex(bytes),
            "signature":hex(&signer.sign(&wire::directory_transcript(bytes)).to_bytes())}),
        )
        .unwrap()
    };
    assert_eq!(
        collector
            .verify_directory(&document(&directory, &key))
            .unwrap(),
        vec![(2, scope)]
    );
    assert!(collector
        .verify_directory(&document(&directory, &SigningKey::from_bytes(&[8; 32])))
        .is_err());
    for offset in [0, 34, 130, wire::DIRECTORY_BYTES - 1] {
        let mut changed = directory;
        changed[offset] ^= 1;
        assert!(collector
            .verify_directory(&document(&changed, &key))
            .is_err());
    }
    let mut duplicated = directory;
    duplicated[32] = 2;
    duplicated[166..299].copy_from_slice(&directory[33..166]);
    assert!(collector
        .verify_directory(&document(&duplicated, &key))
        .is_err());
    assert_eq!(
        wire::epoch_cursor(5, 960).and_then(wire::split_cursor),
        Some((5, 960))
    );
    assert!(wire::split_cursor(wire::DIRECTORY_CURSOR).is_none());
    assert!(wire::epoch_cursor(6, 0).is_none());
    assert!(wire::epoch_cursor(0, 961).is_none());
}

#[test]
fn epoch_collection_reconnects_with_separate_original_boot_cursors() {
    epoch_collection_reconnects(true);
}
#[test]
fn renewed_play_on_same_boot_has_independent_durable_cursor() {
    epoch_collection_reconnects(false);
}
fn epoch_collection_reconnects(change_boot: bool) {
    let key = SigningKey::from_bytes(&[23; 32]);
    let scope = [5; 96];
    let collector =
        Collector::from_authenticated_scope(scope, &key.verifying_key().to_bytes()).unwrap();
    let mut old_scope = scope;
    if change_boot {
        old_scope[32..64].fill(7);
    }
    old_scope[64..].fill(9);
    let forms = collector.forms;
    let mut directory = [0; wire::DIRECTORY_BYTES];
    directory[..32].copy_from_slice(&scope[..32]);
    directory[32] = 2;
    for (entry, (slot, scope)) in directory[33..299]
        .as_chunks_mut::<133>()
        .0
        .iter_mut()
        .zip([(0, scope), (3, old_scope)])
    {
        entry[0] = slot;
        entry[1..97].copy_from_slice(&scope);
        entry[97..129].copy_from_slice(&forms);
        entry[129..].copy_from_slice(&1u32.to_le_bytes());
    }
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let listing = serde_json::to_vec(
        &serde_json::json!({"profile":"observed-epochs@1", "directory":hex(&directory),
        "signature":hex(&key.sign(&wire::directory_transcript(&directory)).to_bytes())}),
    )
    .unwrap();
    let record = |clock| {
        ObservedRecord {
            sequence: 1,
            observed_at_boot_ms: clock,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap()
    };
    let current = record(500);
    let old = record(90_000);
    let batches = vec![
        (wire::DIRECTORY_CURSOR, listing.clone()),
        (0, signed(&key, scope, forms, 0, 1, &current)),
        (1, signed(&key, scope, forms, 1, 1, &[])),
        (
            wire::epoch_cursor(3, 0).unwrap(),
            signed(&key, old_scope, forms, 0, 1, &old),
        ),
        (
            wire::epoch_cursor(3, 1).unwrap(),
            signed(&key, old_scope, forms, 1, 1, &[]),
        ),
        (wire::DIRECTORY_CURSOR, listing),
        (1, signed(&key, scope, forms, 1, 1, &[])),
        (
            wire::epoch_cursor(3, 1).unwrap(),
            signed(&key, old_scope, forms, 1, 1, &[]),
        ),
    ];
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        for (cursor, body) in batches {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            assert!(std::str::from_utf8(&request)
                .unwrap()
                .starts_with(&format!("GET /history?after={cursor} ")));
            socket
                .write_all(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n")
                .unwrap();
            socket.write_all(&body).unwrap();
        }
    });
    let path = std::env::temp_dir().join(format!(
        "conduit-history-epochs-{}-{change_boot}",
        std::process::id()
    ));
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    assert_eq!(collector.collect_epochs(address, &path).unwrap(), 2);
    // Fresh collectors restore fsynced per-scope cursors; no record is relabeled.
    let restarted =
        Collector::from_authenticated_scope(scope, &key.verifying_key().to_bytes()).unwrap();
    assert_eq!(restarted.collect_epochs(address, &path).unwrap(), 2);
    worker.join().unwrap();
    let restored = |scope: [u8; 96]| {
        let name = scope
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut restored =
            Collector::from_authenticated_scope(scope, &key.verifying_key().to_bytes()).unwrap();
        restored
            .restore(&path.join(format!("epoch-{name}")))
            .unwrap();
        (restored.after, restored.observed_ms)
    };
    assert_eq!(restored(scope), (1, 500));
    assert_eq!(restored(old_scope), (1, 90_000));
    std::fs::remove_dir_all(path).unwrap();
}

#[path = "tests/transport.rs"]
mod transport;
