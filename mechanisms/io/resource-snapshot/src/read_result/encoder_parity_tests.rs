use super::*;
fn parity(new: &mut Encoder, old: &mut legacy::Encoder, outcome: ReadResult<'_>) {
    let before = new.backing();
    assert_eq!(new.encode(outcome).unwrap(), old.encode(outcome).unwrap());
    assert_eq!(decode(new.encoded()).unwrap(), outcome);
    assert_eq!(new.backing(), before);
}
#[test]
fn entire_read_domain_parity_keeps_original_canonical_bytes_with_one_cache() {
    let mut new = Encoder::new().unwrap();
    let mut old = legacy::Encoder::new().unwrap();
    println!(
        "read encoder target layout: new_inline={}, legacy_inline={}, linked_composer_inline={}, new_heap={}, legacy_heap={}",
        core::mem::size_of::<Encoder>(),
        core::mem::size_of::<legacy::Encoder>(),
        core::mem::size_of::<conduit_core::PreparedLinkedStructuredComposer<'static>>(),
        new.backing().unwrap(),
        old.backing().unwrap()
    );
    for captured_end in [1, 2, u32::MAX] {
        for index in [0, captured_end - 1] {
            for length in 0..=MAXIMUM_RECORD_BYTES {
                let record = [0x5a; MAXIMUM_RECORD_BYTES];
                parity(
                    &mut new,
                    &mut old,
                    ReadResult::Recorded {
                        index,
                        captured_end,
                        record: &record[..length],
                    },
                );
            }
            for reason in [
                GapReason::Vacant,
                GapReason::UnpublishedOrDamaged,
                GapReason::Corrupt,
            ] {
                parity(
                    &mut new,
                    &mut old,
                    ReadResult::Gap {
                        index,
                        captured_end,
                        reason,
                    },
                );
            }
        }
    }
    for end in [0, 1, u32::MAX] {
        parity(&mut new, &mut old, ReadResult::End(end));
    }
    for outcome in [
        ReadResult::Cancelled,
        ReadResult::Busy,
        ReadResult::RecoveryRequired,
        ReadResult::Unavailable,
    ] {
        parity(&mut new, &mut old, outcome);
    }
    assert_eq!(new.backing(), Some(READ_RESULT_BYTES));
    assert_eq!(old.backing(), Some(2260));
    assert_eq!(new.encode(ReadResult::Cancelled).unwrap(), cancelled());
}
#[test]
fn failure_keeps_exact_previous_retry_bytes_and_encoding_allocates_nothing() {
    let mut new = Encoder::new().unwrap();
    new.encode(ReadResult::End(7)).unwrap();
    let before = new.encoded().to_vec();
    for outcome in [
        ReadResult::Gap {
            index: 1,
            captured_end: 1,
            reason: GapReason::Vacant,
        },
        ReadResult::Recorded {
            index: 0,
            captured_end: 1,
            record: &[0; 201],
        },
    ] {
        assert!(new.encode(outcome).is_err());
        assert_eq!(new.encoded(), before);
    }
    let writer =
        conduit_core::PreparedLinkedStructuredComposer::new(RESULT_SCHEMA, READ_RESULT_BYTES)
            .unwrap();
    let mut short = [0xa5; 1];
    let unit = V::Empty;
    assert!(writer
        .encode_into(
            V::Variant {
                tag: "cancelled",
                payload: &unit
            },
            &mut short
        )
        .is_err());
    assert_eq!(short, [0xa5; 1]);
    // Reuse the crate's existing allocator observation, installed only in tests.
    let (_, observation) = crate::append::tests::allocation_probe::observe(|| {
        new.encode(ReadResult::Recorded {
            index: 0,
            captured_end: 1,
            record: &[1; 200],
        })
        .map(|bytes| bytes.len())
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
}
