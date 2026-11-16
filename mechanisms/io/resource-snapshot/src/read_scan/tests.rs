use super::*;

fn recorded(index: u32, end: u32, bytes: &[u8]) -> ReadResult<'_> {
    ReadResult::Recorded {
        index,
        captured_end: end,
        record: bytes,
    }
}
#[test]
fn growing_tail_does_not_extend_scan_and_payload_is_borrowed() {
    let bytes = [7; 200];
    let mut cursor = RecordScanCursor::new(0, 2).unwrap();
    assert_eq!(
        cursor.accept(0, recorded(0, 2, &bytes)),
        Ok(RecordScanItem::Recorded {
            index: 0,
            record: &bytes
        })
    );
    assert_eq!(cursor.captured_end(), Some(2));
    let item = cursor.accept(1, recorded(1, 99, &bytes)).unwrap();
    let RecordScanItem::Recorded { record, .. } = item else {
        panic!("record");
    };
    assert_eq!(record.as_ptr(), bytes.as_ptr());
    assert_eq!(cursor.captured_end(), Some(2));
    assert_eq!(cursor.remaining(), Some(0));
    assert_eq!(cursor.next_index(), None);
    assert_eq!(
        cursor.accept(2, ReadResult::End(99)),
        Err(RecordScanRefusal::Complete)
    );
}
#[test]
fn every_gap_advances_once_and_is_returned_explicitly() {
    let mut cursor = RecordScanCursor::new(4, 3).unwrap();
    for (index, reason) in [
        (4, GapReason::Vacant),
        (5, GapReason::UnpublishedOrDamaged),
        (6, GapReason::Corrupt),
    ] {
        assert_eq!(
            cursor.accept(
                index,
                ReadResult::Gap {
                    index,
                    captured_end: 7,
                    reason
                }
            ),
            Ok(RecordScanItem::Gap { index, reason })
        );
    }
    assert_eq!(cursor.remaining(), Some(0));
}
#[test]
fn empty_ranges_largest_admitted_index_and_bound_overflow() {
    for first in [0, u32::MAX - 1] {
        let mut cursor = RecordScanCursor::new(first, 1).unwrap();
        assert_eq!(
            cursor.accept(first, ReadResult::End(first)),
            Ok(RecordScanItem::End)
        );
        assert_eq!(cursor.next_index(), None);
        assert_eq!(cursor.remaining(), Some(0));
    }
    assert_eq!(
        RecordScanCursor::new(u32::MAX, 1),
        Err(RecordScanRefusal::Overflow)
    );
    assert_eq!(
        RecordScanCursor::new(u32::MAX - 1, 2),
        Err(RecordScanRefusal::Overflow)
    );
    let mut cursor = RecordScanCursor::new(u32::MAX - 1, 1).unwrap();
    cursor
        .accept(u32::MAX - 1, recorded(u32::MAX - 1, u32::MAX, &[]))
        .unwrap();
    assert_eq!(cursor.next_index(), None);
    assert_eq!(
        RecordScanCursor::new(0, 0),
        Err(RecordScanRefusal::InvalidBound)
    );
}
#[test]
fn refused_results_do_not_advance_or_pin() {
    use RecordScanRefusal as R;
    let cases = [
        (0, recorded(1, 2, &[]), R::WrongIndex),
        (1, recorded(1, 2, &[]), R::WrongIndex),
        (0, recorded(0, 0, &[]), R::InvalidExtent),
        (0, recorded(0, 3, &[]), R::RecordBoundExceeded),
        (0, ReadResult::End(1), R::UnexpectedEnd),
        (0, ReadResult::Busy, R::Busy),
        (0, ReadResult::Cancelled, R::Cancelled),
        (0, ReadResult::RecoveryRequired, R::RecoveryRequired),
        (0, ReadResult::Unavailable, R::Unavailable),
        (0, recorded(0, 1, &[0; 201]), R::RecordTooLarge),
    ];
    for (request, response, error) in cases {
        let mut cursor = RecordScanCursor::new(0, 2).unwrap();
        assert_eq!(cursor.accept(request, response), Err(error));
        assert_eq!(cursor, RecordScanCursor::new(0, 2).unwrap());
        cursor.accept(0, recorded(0, 1, &[])).unwrap();
        assert_eq!(cursor.next_index(), None);
    }
}
#[test]
fn tail_regression_replay_and_early_end_refuse_after_progress() {
    use RecordScanRefusal as R;
    let mut cursor = RecordScanCursor::new(0, 3).unwrap();
    cursor.accept(0, recorded(0, 3, &[])).unwrap();
    cursor.accept(1, recorded(1, 9, &[])).unwrap();
    for (request, response, error) in [
        (2, recorded(2, 8, &[]), R::TailRegressed),
        (2, ReadResult::End(2), R::TailRegressed),
        (2, ReadResult::End(3), R::UnexpectedEnd),
        (1, recorded(1, 9, &[]), R::WrongIndex),
        (2, recorded(1, 9, &[]), R::WrongIndex),
    ] {
        assert_eq!(cursor.accept(request, response), Err(error));
        assert_eq!(cursor.next_index(), Some(2));
        assert_eq!(cursor.captured_end(), Some(3));
    }
    cursor.accept(2, recorded(2, 9, &[])).unwrap();
    assert_eq!(cursor.next_index(), None);
}
#[test]
fn existing_typed_decoder_feeds_cursor_without_copying_record() {
    let mut encoder = crate::read_result::Encoder::new().unwrap();
    let bytes = encoder.encode(recorded(5, 6, &[42; 200])).unwrap();
    let decoded = crate::read_result::decode(bytes).unwrap();
    let ReadResult::Recorded {
        record: original, ..
    } = decoded
    else {
        panic!("record");
    };
    let mut cursor = RecordScanCursor::new(5, 1).unwrap();
    let RecordScanItem::Recorded { record, index } = cursor.accept(5, decoded).unwrap() else {
        panic!("record");
    };
    assert_eq!(index, 5);
    assert_eq!(record.as_ptr(), original.as_ptr());
    assert_eq!(record, &[42; 200]);
    assert_eq!(cursor.next_index(), None);
}
