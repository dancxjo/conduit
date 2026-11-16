use super::*;

#[test]
fn all_media_outcomes_round_trip_without_changing_prepared_backing() {
    let mut encoder = Encoder::new().unwrap();
    let backing = encoder.backing().unwrap();
    let record = [0xa5; MAXIMUM_RECORD_BYTES];
    let outcomes = [
        ReadResult::Recorded {
            index: u32::MAX - 1,
            captured_end: u32::MAX,
            record: &record,
        },
        ReadResult::Gap {
            index: 0,
            captured_end: 1,
            reason: GapReason::Vacant,
        },
        ReadResult::Gap {
            index: 1,
            captured_end: 2,
            reason: GapReason::UnpublishedOrDamaged,
        },
        ReadResult::Gap {
            index: 2,
            captured_end: 3,
            reason: GapReason::Corrupt,
        },
        ReadResult::End(u32::MAX),
        ReadResult::Cancelled,
        ReadResult::Busy,
        ReadResult::RecoveryRequired,
        ReadResult::Unavailable,
    ];
    for outcome in outcomes {
        for _ in 0..2 {
            let bytes = encoder.encode(outcome).unwrap();
            assert!(bytes.len() <= READ_RESULT_BYTES);
            assert_eq!(decode(bytes).unwrap(), outcome);
            assert_eq!(encoder.backing(), Some(backing));
        }
    }
}

#[test]
fn recorded_payload_is_borrowed_and_cancelled_is_prelinked_without_data() {
    let mut encoder = Encoder::new().unwrap();
    let bytes = encoder
        .encode(ReadResult::Recorded {
            index: 3,
            captured_end: 4,
            record: &[7; 200],
        })
        .unwrap();
    let ReadResult::Recorded { record, .. } = decode(bytes).unwrap() else {
        panic!()
    };
    let start = bytes.as_ptr() as usize;
    assert!((start..start + bytes.len()).contains(&(record.as_ptr() as usize)));
    assert_eq!(record, &[7; 200]);
    assert_eq!(decode(cancelled()).unwrap(), ReadResult::Cancelled);
    assert_eq!(encoder.encode(ReadResult::Cancelled).unwrap(), cancelled());
}

#[test]
fn invalid_indices_and_oversized_payload_refuse_without_replacing_last_result() {
    let mut encoder = Encoder::new().unwrap();
    encoder.encode(ReadResult::End(7)).unwrap();
    let prior = encoder.encoded().to_vec();
    for outcome in [
        ReadResult::Recorded {
            index: 2,
            captured_end: 2,
            record: &[],
        },
        ReadResult::Recorded {
            index: 0,
            captured_end: 1,
            record: &[0; 201],
        },
        ReadResult::Gap {
            index: 9,
            captured_end: 8,
            reason: GapReason::Corrupt,
        },
    ] {
        assert!(encoder.encode(outcome).is_err());
        assert_eq!(encoder.encoded(), prior);
    }
    assert!(decode(&[]).is_err());
    let mut damaged = cancelled().to_vec();
    damaged.push(0);
    assert!(decode(&damaged).is_err());
}

#[test]
fn decoder_refuses_count_overflow_and_semantically_invalid_order() {
    use conduit_core::{
        validate_canonical_structured_value as validate, PreparedStructuredComposer as Composer,
    };
    let mut count = Composer::try_from_static_schema(
        include_bytes!(concat!(env!("OUT_DIR"), "/read-count.type")),
        READ_COUNT_BYTES,
        READ_COUNT_BYTES * 2,
    )
    .unwrap();
    let mut result =
        Composer::try_from_static_schema(RESULT_SCHEMA, READ_RESULT_BYTES, READ_RESULT_BYTES * 2)
            .unwrap();
    let count = count
        .leaf(&conduit_core::encode_count(u64::from(u32::MAX) + 1))
        .unwrap();
    assert!(decode(result.variant("end", validate(count).unwrap()).unwrap()).is_err());
    // The media result Type alone cannot establish index/end ordering.
    let mut encoder = Encoder::new().unwrap();
    let mut bytes = encoder
        .encode(ReadResult::Recorded {
            index: 0,
            captured_end: 1,
            record: &[],
        })
        .unwrap()
        .to_vec();
    // Locate the exact primitive index through the borrowed canonical traversal,
    // then alter its eight-byte primitive without guessing schema offsets.
    let validator =
        conduit_core::BorrowedStructuredValueValidator::new(RESULT_SCHEMA, READ_RESULT_BYTES)
            .unwrap();
    let value = validator.validate(&bytes).unwrap();
    let index = value
        .variant_payload("recorded")
        .unwrap()
        .unwrap()
        .record_field("index")
        .unwrap()
        .unwrap()
        .primitive_bytes("value/u64")
        .unwrap();
    let offset = index.as_ptr() as usize - bytes.as_ptr() as usize;
    bytes[offset..offset + 8].copy_from_slice(&conduit_core::encode_count(1));
    assert!(decode(&bytes).is_err());
}

#[test]
fn worst_case_wire_and_prepared_storage_are_explicit_measured_profiles() {
    let mut encoder = Encoder::new().unwrap();
    let bytes = encoder
        .encode(ReadResult::Recorded {
            index: 0,
            captured_end: 1,
            record: &[0; 200],
        })
        .unwrap();
    assert!(bytes.len() <= READ_RESULT_BYTES);
    println!(
        "read canonical: maximum={}, recorded200={}, recorded_header={}, prepared_backing={}",
        READ_RESULT_BYTES,
        bytes.len(),
        bytes.len() - 200,
        encoder.backing().unwrap()
    );
    // The one cache is actual retained payload; linked schema and borrowed
    // traversal carry no heap tables or nested intermediate outputs.
    assert_eq!(encoder.backing(), Some(READ_RESULT_BYTES));
}

#[test]
fn selected_physical_index_is_required_for_records_and_gaps() {
    let mut encoder = Encoder::new().unwrap();
    for outcome in [
        ReadResult::Recorded {
            index: 3,
            captured_end: 9,
            record: &[1, 2],
        },
        ReadResult::Gap {
            index: 3,
            captured_end: 9,
            reason: GapReason::Corrupt,
        },
    ] {
        let bytes = encoder.encode(outcome).unwrap();
        assert_eq!(decode_for_index(3, bytes), Ok(outcome));
        assert_eq!(
            decode_for_index(4, bytes),
            Err(CorrelatedReadRefusal::WrongIndex {
                requested: 4,
                received: 3
            })
        );
    }
}

#[test]
fn end_cannot_skip_requested_records_and_refusals_are_preserved() {
    let mut encoder = Encoder::new().unwrap();
    let bytes = encoder.encode(ReadResult::End(9)).unwrap();
    assert_eq!(
        decode_for_index(8, bytes),
        Err(CorrelatedReadRefusal::InconsistentEnd {
            requested: 8,
            captured_end: 9
        })
    );
    assert_eq!(decode_for_index(9, bytes), Ok(ReadResult::End(9)));
    assert_eq!(decode_for_index(10, bytes), Ok(ReadResult::End(9)));
    for outcome in [
        ReadResult::Cancelled,
        ReadResult::Busy,
        ReadResult::RecoveryRequired,
        ReadResult::Unavailable,
    ] {
        assert_eq!(
            decode_for_index(0, encoder.encode(outcome).unwrap()),
            Ok(outcome)
        );
    }
    assert!(matches!(
        decode_for_index(0, b"not a canonical record"),
        Err(CorrelatedReadRefusal::Schema(_))
    ));
}
