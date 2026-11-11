use super::*;
use alloc::{vec, vec::Vec};
#[path = "../../../../../architecture/plot/tests/common/allocation_probe.rs"]
pub(crate) mod allocation_probe;
mod read_window_tests;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[derive(Debug)]
struct Flash {
    bytes: Vec<u8>,
    cut: Option<usize>,
    wrong_program: Option<u32>,
    calls: usize,
    unit: u32,
}
impl Flash {
    fn new(slots: u32) -> Self {
        Self {
            bytes: vec![255; slots as usize * RECORD_SLOT_BYTES as usize],
            cut: None,
            wrong_program: None,
            calls: 0,
            unit: 256,
        }
    }
}
impl NorStorage for Flash {
    fn capacity(&self) -> u32 {
        self.bytes.len() as u32
    }
    fn program_unit(&self) -> u32 {
        self.unit
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), NorRefusal> {
        self.calls += 1;
        let from = self
            .bytes
            .get(at as usize..at as usize + out.len())
            .ok_or(NorRefusal::Capacity)?;
        out.copy_from_slice(from);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), NorRefusal> {
        self.calls += 1;
        assert_eq!(bytes.len(), 256);
        assert_eq!(at % 256, 0);
        let n = self.cut.map_or(bytes.len(), |left| left.min(bytes.len()));
        for (i, b) in bytes[..n].iter().enumerate() {
            self.bytes[at as usize + i] &= *b;
        }
        if let Some(left) = self.cut {
            self.cut = Some(left - n);
            if left <= bytes.len() {
                return Err(NorRefusal::Media);
            }
        }
        if self.wrong_program == Some(at) {
            self.bytes[at as usize] &= !2;
        }
        Ok(())
    }
}
fn prepare(flash: Flash) -> PreparedNorRecordResidence<Flash> {
    let slots = flash.capacity() / RECORD_SLOT_BYTES;
    PreparedNorRecordResidence::prepare(flash, RecordResidenceId([7; 32]), 0, slots)
        .ok()
        .unwrap()
}
#[test]
fn every_data_and_commit_cut_preserves_previous_records_and_exposes_uncertain_tail() {
    for cut in 0..=512 {
        let mut residence = prepare(Flash::new(3));
        assert_eq!(residence.recover().unwrap().next_index, 0);
        assert_eq!(residence.append(b"previous"), Ok(DurableRecordIndex(0)));
        let mut flash = residence.into_storage();
        flash.cut = Some(cut);
        let mut residence = prepare(flash);
        residence.recover().unwrap();
        assert!(matches!(
            residence.append(b"interrupted"),
            Err(RecordResidenceRefusal::Media(_))
        ));
        assert_eq!(
            residence.append(b"unsafe-retry"),
            Err(RecordResidenceRefusal::RecoveryRequired)
        );
        let mut flash = residence.into_storage();
        flash.cut = None;
        let mut reboot = prepare(flash);
        let report = reboot.recover().unwrap();
        assert_eq!(
            reboot.read(DurableRecordIndex(0)).unwrap(),
            RecordRead::Committed {
                index: DurableRecordIndex(0),
                payload: b"previous"
            }
        );
        if cut == 0 {
            assert_eq!(report.next_index, 1);
            assert_eq!(report.unpublished_or_damaged, 0);
        } else if cut == 512 {
            assert_eq!(report.next_index, 2);
            assert_eq!(report.committed, 2);
            assert_eq!(
                reboot.read(DurableRecordIndex(1)).unwrap(),
                RecordRead::Committed {
                    index: DurableRecordIndex(1),
                    payload: b"interrupted"
                }
            );
        } else {
            assert_eq!(report.next_index, 2);
            assert_eq!(report.committed, 1);
            assert_eq!(report.unpublished_or_damaged, 1);
            assert_eq!(
                reboot.read(DurableRecordIndex(1)).unwrap(),
                RecordRead::Gap {
                    index: DurableRecordIndex(1),
                    reason: RecordGap::UnpublishedOrDamaged
                }
            );
        }
        assert_eq!(
            reboot.append(b"after-recovery"),
            Ok(DurableRecordIndex(report.next_index))
        );
    }
}
#[test]
fn corruption_foreign_residence_and_vacant_holes_never_become_exact_history() {
    let mut residence = prepare(Flash::new(3));
    residence.recover().unwrap();
    residence.append(b"first").unwrap();
    residence.append(b"second").unwrap();
    let mut flash = residence.into_storage();
    flash.bytes[HEADER] ^= 1;
    let mut damaged = prepare(flash);
    let report = damaged.recover().unwrap();
    assert_eq!(report.corrupt, 1);
    assert_eq!(report.committed, 1);
    assert_eq!(report.next_index, 2);
    assert_eq!(
        damaged.read(DurableRecordIndex(0)).unwrap(),
        RecordRead::Gap {
            index: DurableRecordIndex(0),
            reason: RecordGap::Corrupt
        }
    );
    let flash = damaged.into_storage();
    let mut foreign = PreparedNorRecordResidence::prepare(flash, RecordResidenceId([8; 32]), 0, 3)
        .ok()
        .unwrap();
    assert_eq!(
        foreign.recover(),
        Err(RecordResidenceRefusal::ForeignResidence)
    );
    assert_eq!(
        foreign.append(b"foreign-write"),
        Err(RecordResidenceRefusal::RecoveryRequired)
    );
    let mut flash = foreign.into_storage();
    flash.bytes[..RECORD_SLOT_BYTES as usize].fill(255);
    let mut holes = prepare(flash);
    let report = holes.recover().unwrap();
    assert_eq!(report.vacant_gaps, 1);
    assert_eq!(report.committed, 1);
    assert_eq!(report.next_index, 2);
    assert_eq!(holes.append(b"third"), Ok(DurableRecordIndex(2)));
    assert_eq!(holes.append(b"full"), Err(RecordResidenceRefusal::Capacity));
}
#[test]
fn geometry_capacity_and_no_per_record_allocation_are_bounded() {
    let flash = Flash::new(2);
    let (returned, error) =
        PreparedNorRecordResidence::prepare(flash, RecordResidenceId([7; 32]), 1, 2)
            .err()
            .unwrap();
    assert_eq!(error, RecordResidenceRefusal::Geometry);
    assert_eq!(returned.calls, 0);
    let flash = Flash::new(2);
    let (returned, error) =
        PreparedNorRecordResidence::prepare(flash, RecordResidenceId([7; 32]), 0, u32::MAX)
            .err()
            .unwrap();
    assert_eq!(error, RecordResidenceRefusal::Capacity);
    assert_eq!(returned.calls, 0);
    let mut residence = prepare(returned);
    assert_eq!(
        residence.append(b"before-scan"),
        Err(RecordResidenceRefusal::RecoveryRequired)
    );
    assert_eq!(residence.maximum_recovery_reads(), 4);
    assert_eq!(residence.maximum_append_media_calls(), 6);
    let ((), allocations) = allocation_probe::observe(|| {
        assert_eq!(residence.recover().unwrap().next_index, 0);
        assert_eq!(
            residence.append(&[1; 201]),
            Err(RecordResidenceRefusal::Payload)
        );
        assert_eq!(residence.append(&[9; 200]), Ok(DurableRecordIndex(0)));
        assert_eq!(residence.append(b"last"), Ok(DurableRecordIndex(1)));
        assert_eq!(
            residence.append(b"full"),
            Err(RecordResidenceRefusal::Capacity)
        );
        assert_eq!(
            residence.read(DurableRecordIndex(0)).unwrap(),
            RecordRead::Committed {
                index: DurableRecordIndex(0),
                payload: &[9; 200]
            }
        );
        assert_eq!(residence.recover().unwrap().committed, 2);
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
}

#[test]
fn acknowledged_wrong_program_never_reports_a_durable_append() {
    for bad_at in [0, 256] {
        let mut flash = Flash::new(2);
        flash.wrong_program = Some(bad_at);
        let mut residence = prepare(flash);
        residence.recover().unwrap();
        assert_eq!(
            residence.append(b"must-verify"),
            Err(RecordResidenceRefusal::Media(NorRefusal::Corrupt))
        );
        let flash = residence.into_storage();
        if bad_at == 0 {
            assert!(flash.bytes[256..512].iter().all(|b| *b == 255));
        }
        let mut reopened = prepare(flash);
        let report = reopened.recover().unwrap();
        assert_eq!(report.committed, 0);
        assert_eq!(report.unpublished_or_damaged + report.corrupt, 1);
        assert_eq!(report.next_index, 1);
    }
}
