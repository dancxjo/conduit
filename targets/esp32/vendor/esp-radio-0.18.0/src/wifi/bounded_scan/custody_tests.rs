use super::*;
#[test]
fn cancellation_and_stop_return_never_release_without_terminal_and_list_clear() {
    let mut custody = Custody::quiescent();
    assert!(custody.begin());
    assert!(!custody.begin());
    assert!(!custody.is_quiescent()); // dropping a borrowing wait changes nothing
    assert!(custody.request_stop());
    for _ in 0..8 {
        assert!(!custody.request_stop());
        assert!(!custody.is_quiescent());
        assert!(!custody.cleared());
    }
    let event = Completion {
        status: 1,
        number: 0,
        scan_id: 255,
    };
    custody.observed(event);
    assert_eq!(custody.completion(), Some(event));
    assert!(!custody.is_quiescent()); // list-clear refusal keeps exact custody
    assert!(!custody.request_stop());
    assert!(custody.cleared());
    assert!(custody.is_quiescent());
    assert!(custody.begin());
}
#[test]
fn lag_and_unknown_start_outcome_quarantine_even_if_a_later_event_arrives() {
    for stop in [false, true] {
        let mut custody = Custody::quiescent();
        custody.begin();
        if stop {
            assert!(custody.request_stop());
        }
        custody.quarantine();
        custody.observed(Completion {
            status: 0,
            number: 1,
            scan_id: 42,
        });
        assert!(custody.is_quarantined());
        assert!(!custody.cleared());
        assert!(!custody.is_quiescent());
        assert!(!custody.begin());
    }
}
#[test]
fn sdk_completion_id_is_observed_not_predicted_and_duplicates_do_not_reassign() {
    let mut custody = Custody::quiescent();
    for scan_id in [255, 0, 0, 199] {
        assert!(custody.begin());
        let event = Completion {
            status: 0,
            number: 16,
            scan_id,
        };
        custody.observed(event);
        assert_eq!(custody.completion(), Some(event));
        assert!(custody.cleared());
    }
    custody.observed(Completion {
        status: 0,
        number: 16,
        scan_id: 199,
    });
    assert!(custody.is_quarantined());
}
#[test]
fn actual_total_bounds_copy_and_preserves_exact_prefix_without_hidden_allocation() {
    for total in [0, 1, 15, 16, 17, 255, u16::MAX] {
        let mut output = [u32::MAX; 32];
        let mut reads = 0;
        let count = copy_prefix(&mut output, total, &mut || {
            let value = reads;
            reads += 1;
            Ok::<_, ()>(value)
        })
        .unwrap();
        assert_eq!(count, usize::from(total).min(16));
        assert_eq!(reads as usize, count);
        for (index, value) in output.iter().enumerate() {
            assert_eq!(
                *value,
                if index < count {
                    index as u32
                } else {
                    u32::MAX
                }
            );
        }
    }
}
#[test]
fn every_sdk_read_refusal_stops_without_reading_or_overwriting_later_slots() {
    for failure in 0..16 {
        let mut output = [u8::MAX; 16];
        let mut reads = 0;
        assert_eq!(
            copy_prefix(&mut output, 32, &mut || {
                let at = reads;
                reads += 1;
                if at == failure { Err(at) } else { Ok(at) }
            }),
            Err(failure)
        );
        assert_eq!(reads, failure + 1);
        assert!(output[usize::from(failure)..].iter().all(|v| *v == u8::MAX));
    }
}
#[test]
fn raw_ssid_and_unknown_authentication_are_preserved_without_open_downgrade() {
    for authentication in [0, 1, 2, 3, 6, 99, u32::MAX] {
        let input = [255; 32];
        let record = Record::from_sdk(&input, -127, authentication).unwrap();
        assert_eq!(record.ssid, input);
        assert_eq!(record.ssid_len, 32);
        assert_eq!(record.rssi, -127);
        assert_eq!(record.authentication, authentication);
    }
    assert!(Record::from_sdk(&[255; 33], -5, 0).is_none());
    let record = Record::from_sdk(b"name\0garbage", -5, 0).unwrap();
    assert_eq!(record.ssid_len, 4);
    assert_eq!(&record.ssid[..4], b"name");
}
