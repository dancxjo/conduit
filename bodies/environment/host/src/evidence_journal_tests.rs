use conduit_environment_runtime::display_evidence_journal::{
    Journal, Refusal, Storage, WORKSPACE_BYTES,
};
#[derive(Clone)]
struct Nor {
    bytes: Vec<u8>,
    remaining: Option<usize>,
}
impl Storage for Nor {
    fn capacity(&self) -> u32 {
        self.bytes.len() as u32
    }
    fn read(&mut self, at: u32, output: &mut [u8]) -> Result<(), Refusal> {
        output.copy_from_slice(
            self.bytes
                .get(at as usize..at as usize + output.len())
                .ok_or(Refusal::Media)?,
        );
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        assert!(at.is_multiple_of(4) && bytes.len().is_multiple_of(4));
        for (i, &byte) in bytes.iter().enumerate() {
            if self.remaining == Some(0) {
                return Err(Refusal::Media);
            }
            let output = self.bytes.get_mut(at as usize + i).ok_or(Refusal::Media)?;
            assert_eq!(
                *output & byte,
                byte,
                "NOR must never rewrite a programmed bit"
            );
            *output &= byte;
            if let Some(n) = self.remaining.as_mut() {
                *n -= 1;
            }
        }
        Ok(())
    }
}
fn nor() -> Nor {
    Nor {
        bytes: vec![255; 8192],
        remaining: None,
    }
}
#[test]
fn power_cut_at_every_append_byte_preserves_committed_prefix() {
    let scope = [3; 96];
    let basis = [0; 1168];
    let mut work = [0; WORKSPACE_BYTES];
    let mut page = basis;
    page[8] = 1;
    let mut original = Journal::create(nor(), &scope, &basis, 20, &mut work).unwrap();
    original.append(&page, &mut work).unwrap();
    let original = original.into_storage();
    page[8] = 2;
    let mut probe = Journal::recover(original.clone(), &scope, &basis, 20, &mut work).unwrap();
    let extent = probe.append(&page, &mut work).unwrap().extent() as usize;
    for cut in 0..=extent {
        let mut media = original.clone();
        media.remaining = Some(cut);
        let mut journal = Journal::recover(media, &scope, &basis, 20, &mut work).unwrap();
        let result = journal.append(&page, &mut work);
        assert_eq!(result.is_ok(), cut == extent);
        if result.is_err() {
            assert_eq!(journal.append(&page, &mut work), Err(Refusal::Torn));
        }
        let mut media = journal.into_storage();
        media.remaining = None;
        let mut recovered = Journal::recover(media, &scope, &basis, 20, &mut work).unwrap();
        assert_eq!(
            recovered.committed_count(),
            if cut == extent { 2 } else { 1 }
        );
        let mut cursor = recovered.replay_cursor();
        let mut output = [0; 2064];
        assert_eq!(
            recovered
                .replay(&mut cursor, &mut output, &mut work)
                .unwrap(),
            Some(1168)
        );
        let mut first = basis;
        first[8] = 1;
        assert_eq!(&output[..1168], &first);
        if cut == extent {
            assert_eq!(
                recovered
                    .replay(&mut cursor, &mut output, &mut work)
                    .unwrap(),
                Some(1168)
            );
            assert_eq!(&output[..1168], &page);
        }
        assert_eq!(
            recovered
                .replay(&mut cursor, &mut output, &mut work)
                .unwrap(),
            None
        );
        if cut > 0 && cut < extent {
            assert!(recovered.interrupted_tail());
        }
    }
}
#[test]
fn stale_scope_corrupt_media_and_capacity_do_not_release_custody() {
    let scope = [3; 96];
    let basis = [0; 1168];
    let mut work = [0; WORKSPACE_BYTES];
    let mut journal = Journal::create(nor(), &scope, &basis, 1, &mut work).unwrap();
    let record = journal.append(&basis, &mut work).unwrap();
    assert_eq!(record.sequence(), 0);
    assert_eq!(journal.append(&basis, &mut work), Err(Refusal::Capacity));
    let original = journal.into_storage();
    assert!(matches!(
        Journal::recover(original.clone(), &[4; 96], &basis, 1, &mut work),
        Err(Refusal::Scope)
    ));
    let mut wrong = basis;
    wrong[0] = 1;
    assert!(matches!(
        Journal::recover(original.clone(), &scope, &wrong, 1, &mut work),
        Err(Refusal::Scope)
    ));
    let mut corrupt = original.clone();
    corrupt.bytes[record.offset() as usize + 8] ^= 1;
    assert!(matches!(
        Journal::recover(corrupt, &scope, &basis, 1, &mut work),
        Err(Refusal::Corrupt)
    ));
    assert!(matches!(
        Journal::create(original, &scope, &basis, 1, &mut work),
        Err(Refusal::Corrupt)
    ));
}

#[test]
fn erased_leading_word_does_not_make_torn_storage_reusable() {
    let scope = [3; 96];
    let basis = [0; 1168];
    let mut work = [0; WORKSPACE_BYTES];
    let mut journal = Journal::create(nor(), &scope, &basis, 20, &mut work).unwrap();
    let record = journal.append(&basis, &mut work).unwrap();
    let mut media = journal.into_storage();
    media.bytes[(record.offset() + record.extent()) as usize + 12] = 0;
    let mut recovered = Journal::recover(media, &scope, &basis, 20, &mut work).unwrap();
    assert_eq!(recovered.committed_count(), 1);
    assert!(recovered.interrupted_tail());
    assert_eq!(recovered.append(&basis, &mut work), Err(Refusal::Torn));
}

#[test]
fn uncertain_commit_is_recovered_without_claiming_a_second_record() {
    struct Uncertain(Nor, bool);
    impl Storage for Uncertain {
        fn capacity(&self) -> u32 {
            self.0.capacity()
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
            self.0.read(at, out)
        }
        fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
            self.0.program(at, bytes)?;
            if self.1 && bytes == b"done" {
                Err(Refusal::Media)
            } else {
                Ok(())
            }
        }
    }
    let scope = [3; 96];
    let basis = [0; 1168];
    let mut work = [0; WORKSPACE_BYTES];
    let journal = Journal::create(Uncertain(nor(), false), &scope, &basis, 20, &mut work).unwrap();
    let mut media = journal.into_storage();
    media.1 = true;
    let mut journal = Journal::recover(media, &scope, &basis, 20, &mut work).unwrap();
    assert_eq!(journal.append(&basis, &mut work), Err(Refusal::Media));
    assert_eq!(journal.committed_count(), 0); // no false producer acknowledgement
    let mut media = journal.into_storage();
    media.1 = false;
    let mut recovered = Journal::recover(media, &scope, &basis, 20, &mut work).unwrap();
    assert_eq!(recovered.committed_count(), 1); // reconcile the exact old page
    let mut cursor = recovered.replay_cursor();
    let mut output = [0; 1168];
    assert_eq!(
        recovered
            .replay(&mut cursor, &mut output, &mut work)
            .unwrap(),
        Some(1168)
    );
    assert_eq!(output, basis);
    assert_eq!(
        recovered
            .replay(&mut cursor, &mut output, &mut work)
            .unwrap(),
        None
    );
}

#[test]
fn erased_preparation_failure_retains_old_region_for_recovery() {
    use conduit_environment_runtime::display_evidence_journal::Erased;
    let scope = [3; 96];
    let basis = [0; 1168];
    let mut work = [0; WORKSPACE_BYTES];
    let mut journal = Journal::create(nor(), &scope, &basis, 1, &mut work).unwrap();
    journal.append(&basis, &mut work).unwrap();
    let media = journal.into_storage();
    let expected = media.bytes.clone();
    let (media, error) = match Erased::prepare_retained(media) {
        Ok(_) => panic!("old custody must not be reused"),
        Err(pair) => pair,
    };
    assert_eq!(error, Refusal::Corrupt);
    assert_eq!(media.bytes, expected);
    let recovered = Journal::recover(media, &scope, &basis, 1, &mut work).unwrap();
    assert_eq!(recovered.committed_count(), 1);
}

#[test]
fn small_typed_record_workspace_is_exact_and_short_buffers_preserve_custody() {
    let scope = [9; 96];
    let basis = [0; 21];
    let mut work = [0; 136];
    let mut journal = Journal::create(nor(), &scope, &basis, 960, &mut work).unwrap();
    let mut record = [0; 21];
    record[0] = 1;
    assert_eq!(
        journal.append(&record, &mut [0; 23]),
        Err(Refusal::Capacity)
    );
    assert_eq!(journal.committed_count(), 0);
    journal.append(&record, &mut work).unwrap();
    let media = journal.into_storage();
    let mut journal = Journal::recover(media, &scope, &basis, 960, &mut work).unwrap();
    let mut cursor = journal.replay_cursor();
    let mut output = [0; 21];
    assert_eq!(
        journal.replay(&mut cursor, &mut output, &mut [0; 7]),
        Err(Refusal::Capacity)
    );
    assert_eq!(
        journal.replay(&mut cursor, &mut output, &mut work).unwrap(),
        Some(21)
    );
    assert_eq!(output, record);
}

#[test]
fn overnight_observations_fit_and_replay_without_a_remote_consumer() {
    use conduit_environment_runtime::observed_record::{self, ObservedRecord};
    use conduit_environment_runtime::recording::RecordingDisposition;
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let basis = observed_record::descriptor(&contract).unwrap();
    let scope = [7; 96];
    let media = Nor {
        bytes: vec![255; 65536],
        remaining: None,
    };
    let mut work = [0; 536];
    let mut journal = Journal::create(media, &scope, &basis, 960, &mut work).unwrap();
    for sequence in 1..=960 {
        let record = ObservedRecord {
            sequence,
            observed_at_boot_ms: sequence * 60000,
            disposition: RecordingDisposition::Available,
            quantities: Some((
                conduit_core::Quantity::new(20, conduit_core::Unit::Celsius),
                conduit_core::Quantity::new(59, conduit_core::Unit::Percent),
            )),
        }
        .encode()
        .unwrap();
        journal.append(&record, &mut work).unwrap();
    }
    assert_eq!(journal.committed_count(), 960);
    let media = journal.into_storage();
    let mut recovered = Journal::recover(media, &scope, &basis, 960, &mut work).unwrap();
    let mut indexed = recovered.replay_cursor_at(959).unwrap();
    let mut last = [0; observed_record::BYTES];
    recovered
        .replay(&mut indexed, &mut last, &mut work)
        .unwrap();
    assert_eq!(ObservedRecord::decode(&last).unwrap().sequence, 960);
    assert!(recovered.replay_cursor_at(961).is_err());
    let mut cursor = recovered.replay_cursor();
    let mut output = [0; observed_record::BYTES];
    for sequence in 1..=960 {
        assert_eq!(
            recovered
                .replay(&mut cursor, &mut output, &mut work)
                .unwrap(),
            Some(observed_record::BYTES)
        );
        let record = ObservedRecord::decode(&output).unwrap();
        assert_eq!(record.sequence, sequence);
        assert_eq!(record.observed_at_boot_ms, sequence * 60000);
    }
    assert_eq!(
        recovered
            .replay(&mut cursor, &mut output, &mut work)
            .unwrap(),
        None
    );
}

#[test]
fn indexed_replay_refuses_mixed_record_geometry() {
    let basis = [0; 21];
    let mut work = [0; WORKSPACE_BYTES];
    let mut journal = Journal::create(nor(), &[5; 96], &basis, 5, &mut work).unwrap();
    journal.append(&basis, &mut work).unwrap();
    journal.append(&[1; 100], &mut work).unwrap();
    journal.append(&basis, &mut work).unwrap();
    assert!(journal.replay_cursor_at(1).is_err());
    let media = journal.into_storage();
    let recovered = Journal::recover(media, &[5; 96], &basis, 5, &mut work).unwrap();
    assert!(recovered.replay_cursor_at(1).is_err());
}
