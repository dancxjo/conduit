use super::*;

struct Storage {
    flash: Flash,
    reads: usize,
    programs: usize,
    fail_read: bool,
}
impl NorStorage for Storage {
    fn capacity(&self) -> u32 {
        self.flash.capacity()
    }
    fn program_unit(&self) -> u32 {
        self.flash.program_unit()
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), NorRefusal> {
        self.reads += 1;
        if core::mem::take(&mut self.fail_read) {
            return Err(NorRefusal::Media);
        }
        self.flash.read(at, out)
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), NorRefusal> {
        self.programs += 1;
        self.flash.program(at, bytes)
    }
}
fn residence() -> PreparedNorRecordResidence<Storage> {
    PreparedNorRecordResidence::prepare(
        Storage {
            flash: Flash::new(4),
            reads: 0,
            programs: 0,
            fail_read: false,
        },
        RecordResidenceId([7; 32]),
        0,
        4,
    )
    .ok()
    .unwrap()
}

#[test]
fn read_window_captures_tail_and_allows_appends_without_extending_scan() {
    let mut owner = residence();
    owner.recover().unwrap();
    owner.append(b"first").unwrap();
    owner.append(b"second").unwrap();
    let reads = owner.storage.reads;
    let mut window = owner
        .capture_read_window(DurableRecordIndex(0), u32::MAX)
        .unwrap();
    assert_eq!(owner.storage.reads, reads);
    assert_eq!(window.first(), DurableRecordIndex(0));
    assert_eq!(window.end(), DurableRecordIndex(2));
    assert_eq!(window.maximum_media_reads(), 4);
    assert_eq!(owner.append(b"later"), Ok(DurableRecordIndex(2)));
    assert!(matches!(
        owner.read_window_next(&mut window).unwrap(),
        Some(RecordRead::Committed {
            index: DurableRecordIndex(0),
            payload: b"first"
        })
    ));
    assert_eq!(window.remaining(), 1);
    assert!(matches!(
        owner.read_window_next(&mut window).unwrap(),
        Some(RecordRead::Committed {
            index: DurableRecordIndex(1),
            payload: b"second"
        })
    ));
    let reads = owner.storage.reads;
    assert_eq!(owner.read_window_next(&mut window), Ok(None));
    assert_eq!(owner.storage.reads, reads);
    assert_eq!(owner.remaining_slots(), Some(1));
}

#[test]
fn read_window_exposes_every_physical_gap_and_never_programs_media() {
    let mut owner = residence();
    owner.recover().unwrap();
    for value in [b"one", b"two", b"end"] {
        owner.append(value).unwrap();
    }
    owner.storage.flash.bytes[..512].fill(255);
    owner.storage.flash.bytes[512 + 256] = 0;
    let recovery = owner.recover().unwrap();
    assert_eq!(recovery.vacant_gaps, 1);
    assert_eq!(recovery.unpublished_or_damaged, 1);
    let mut window = owner.capture_read_window(DurableRecordIndex(0), 3).unwrap();
    let before = owner.storage.flash.bytes.clone();
    let programs = owner.storage.programs;
    let reads = owner.storage.reads;
    assert_eq!(
        owner.read_window_next(&mut window),
        Ok(Some(RecordRead::Vacant {
            index: DurableRecordIndex(0)
        }))
    );
    assert!(matches!(
        owner.read_window_next(&mut window),
        Ok(Some(RecordRead::Gap {
            index: DurableRecordIndex(1),
            ..
        }))
    ));
    assert!(matches!(
        owner.read_window_next(&mut window),
        Ok(Some(RecordRead::Committed {
            index: DurableRecordIndex(2),
            payload: b"end"
        }))
    ));
    assert_eq!(owner.storage.reads - reads, 6);
    assert_eq!(owner.storage.programs, programs);
    assert_eq!(owner.storage.flash.bytes, before);
}

#[test]
fn read_refusal_does_not_advance_range_or_supply_automatic_retry_work() {
    let mut owner = residence();
    owner.recover().unwrap();
    owner.append(b"one").unwrap();
    let mut window = owner.capture_read_window(DurableRecordIndex(0), 1).unwrap();
    owner.storage.fail_read = true;
    assert_eq!(
        owner.read_window_next(&mut window),
        Err(RecordResidenceRefusal::Media(NorRefusal::Media))
    );
    assert_eq!(window.remaining(), 1);
    assert!(matches!(
        owner.read_window_next(&mut window),
        Ok(Some(RecordRead::Committed {
            index: DurableRecordIndex(0),
            payload: b"one"
        }))
    ));
    assert_eq!(window.remaining(), 0);
}

#[test]
fn read_window_refuses_unrecovered_foreign_and_invalid_scopes() {
    let mut owner = residence();
    assert!(matches!(
        owner.capture_read_window(DurableRecordIndex(0), 1),
        Err(RecordResidenceRefusal::RecoveryRequired)
    ));
    owner.recover().unwrap();
    owner.append(b"one").unwrap();
    assert!(owner.capture_read_window(DurableRecordIndex(0), 0).is_err());
    assert!(owner.capture_read_window(DurableRecordIndex(2), 1).is_err());
    let mut empty = owner.capture_read_window(DurableRecordIndex(1), 1).unwrap();
    assert_eq!(owner.read_window_next(&mut empty), Ok(None));
    let mut window = owner.capture_read_window(DurableRecordIndex(0), 1).unwrap();
    owner.identity = RecordResidenceId([8; 32]);
    assert_eq!(
        owner.read_window_next(&mut window),
        Err(RecordResidenceRefusal::ForeignResidence)
    );
    assert_eq!(window.remaining(), 1);
    owner.identity = RecordResidenceId([7; 32]);
    owner.recovered = false;
    assert_eq!(
        owner.read_window_next(&mut window),
        Err(RecordResidenceRefusal::RecoveryRequired)
    );
    assert_eq!(window.remaining(), 1);
}

#[test]
fn reading_and_cancellation_allocate_nothing_and_do_not_move_append_tail() {
    let mut owner = residence();
    owner.recover().unwrap();
    owner.append(b"one").unwrap();
    let ((), observation) = allocation_probe::observe(|| {
        let mut window = owner.capture_read_window(DurableRecordIndex(0), 1).unwrap();
        assert!(matches!(
            owner.read_window_next(&mut window),
            Ok(Some(RecordRead::Committed { .. }))
        ));
    });
    assert_eq!((observation.allocations, observation.reallocations), (0, 0));
    let _cancelled = owner.capture_read_window(DurableRecordIndex(0), 1).unwrap();
    assert_eq!(owner.append(b"two"), Ok(DurableRecordIndex(1)));
}
