use super::*;

fn envelope(sequence: u64) -> Envelope {
    Envelope {
        instance: 7,
        sequence,
    }
}
fn turn(
    budget: &mut OperationBudget,
    sequence: u64,
    request: Request,
    rtc: u32,
    response: Response,
) -> u32 {
    let cap = budget.before(envelope(sequence), request).unwrap();
    budget.observe(envelope(sequence), cap, rtc, true).unwrap();
    budget.respond(response).unwrap();
    cap
}
fn written() -> OperationBudget {
    let mut budget = OperationBudget::new(envelope(10), 3, [42; 32]).unwrap();
    turn(
        &mut budget,
        10,
        Request::Begin { bytes: 3 },
        1,
        Response::Ready,
    );
    turn(
        &mut budget,
        11,
        Request::Write {
            offset: 0,
            bytes: 3,
        },
        2,
        Response::Ready,
    );
    budget
}

#[test]
fn full_operation_debits_every_phase_without_renewing() {
    let mut budget = written();
    assert_eq!(budget.digest(), &[42; 32]);
    assert_eq!(
        turn(&mut budget, 12, Request::Preflight, 20, Response::Ready),
        61
    );
    assert!(!budget.complete());
    assert_eq!(
        turn(
            &mut budget,
            13,
            Request::Commit,
            30,
            Response::Blob { bytes: 2 }
        ),
        41
    );
    assert!(!budget.complete());
    assert_eq!(
        turn(
            &mut budget,
            14,
            Request::Read {
                offset: 0,
                maximum: 2
            },
            1,
            Response::Chunk {
                offset: 0,
                bytes: 2,
                total: 2
            }
        ),
        11
    );
    assert!(budget.complete());
    assert!(
        budget
            .before(envelope(15), Request::Begin { bytes: 3 })
            .is_err()
    );
}

#[test]
fn zero_irq_turns_still_require_finite_monotone_progress() {
    let mut budget = OperationBudget::new(envelope(1), TRANSFER_BYTES + 1, [0; 32]).unwrap();
    turn(
        &mut budget,
        1,
        Request::Begin {
            bytes: (TRANSFER_BYTES + 1) as u32,
        },
        0,
        Response::Ready,
    );
    turn(
        &mut budget,
        2,
        Request::Write {
            offset: 0,
            bytes: TRANSFER_BYTES as u32,
        },
        0,
        Response::Ready,
    );
    assert!(
        budget
            .before(
                envelope(3),
                Request::Write {
                    offset: 0,
                    bytes: TRANSFER_BYTES as u32
                }
            )
            .is_err()
    );
    assert!(
        budget
            .before(
                envelope(3),
                Request::Write {
                    offset: TRANSFER_BYTES as u32,
                    bytes: 1
                }
            )
            .is_err()
    );
}

#[test]
fn stale_foreign_duplicate_and_missing_receipts_are_terminal() {
    for foreign in [
        Envelope {
            instance: 8,
            sequence: 12,
        },
        envelope(11),
        envelope(13),
    ] {
        let mut budget = written();
        let cap = budget.before(envelope(12), Request::Preflight).unwrap();
        assert!(budget.observe(foreign, cap, 0, true).is_err());
        assert!(budget.observe(envelope(12), cap, 0, true).is_err());
    }
    let mut budget = written();
    assert!(budget.respond(Response::Ready).is_err());
    let mut budget = written();
    let cap = budget.before(envelope(12), Request::Preflight).unwrap();
    budget.observe(envelope(12), cap, 0, true).unwrap();
    assert!(budget.observe(envelope(12), cap, 0, true).is_err());
}

#[test]
fn reordered_skipped_partial_and_duplicate_phases_refuse() {
    for request in [
        Request::Commit,
        Request::Begin { bytes: 3 },
        Request::Write {
            offset: 0,
            bytes: 3,
        },
        Request::Read {
            offset: 0,
            maximum: 1,
        },
    ] {
        let mut budget = written();
        assert!(budget.before(envelope(12), request).is_err());
    }
    let mut budget = OperationBudget::new(envelope(1), 3, [0; 32]).unwrap();
    turn(
        &mut budget,
        1,
        Request::Begin { bytes: 3 },
        0,
        Response::Ready,
    );
    assert!(
        budget
            .before(
                envelope(2),
                Request::Write {
                    offset: 0,
                    bytes: 2
                }
            )
            .is_err()
    );
}

#[test]
fn changed_cap_overrun_and_native_fault_cannot_replenish() {
    for (admitted, consumed, yielded) in [(64, 0, true), (61, 62, true), (61, 61, false)] {
        let mut budget = written();
        budget.before(envelope(12), Request::Preflight).unwrap();
        assert!(
            budget
                .observe(envelope(12), admitted, consumed, yielded)
                .is_err()
        );
        assert!(budget.before(envelope(12), Request::Preflight).is_err());
    }
    let mut budget = written();
    budget.before(envelope(12), Request::Preflight).unwrap();
    budget.observe(envelope(12), 61, 61, true).unwrap();
    assert!(budget.respond(Response::Ready).is_err());
    assert!(budget.before(envelope(13), Request::Commit).is_err());
}

#[test]
fn oversized_or_mismatched_replies_cannot_complete_publication() {
    for response in [
        Response::Ready,
        Response::Blob { bytes: 0 },
        Response::Blob {
            bytes: MAX_BLOB as u32 + 1,
        },
    ] {
        let mut budget = written();
        turn(&mut budget, 12, Request::Preflight, 0, Response::Ready);
        let cap = budget.before(envelope(13), Request::Commit).unwrap();
        budget.observe(envelope(13), cap, 0, true).unwrap();
        assert!(budget.respond(response).is_err());
        assert!(!budget.complete());
    }
    let mut budget = written();
    turn(&mut budget, 12, Request::Preflight, 0, Response::Ready);
    turn(
        &mut budget,
        13,
        Request::Commit,
        0,
        Response::Blob { bytes: 2 },
    );
    let cap = budget
        .before(
            envelope(14),
            Request::Read {
                offset: 0,
                maximum: 2,
            },
        )
        .unwrap();
    budget.observe(envelope(14), cap, 0, true).unwrap();
    assert!(
        budget
            .respond(Response::Chunk {
                offset: 0,
                bytes: 2,
                total: 3
            })
            .is_err()
    );
}

#[test]
fn subchunk_replies_cannot_extend_zero_irq_operation_forever() {
    let mut budget = written();
    turn(&mut budget, 12, Request::Preflight, 0, Response::Ready);
    turn(
        &mut budget,
        13,
        Request::Commit,
        0,
        Response::Blob { bytes: 100 },
    );
    // Profile permits at most eight reply turns even if the guest returns less
    // than requested. The ninth read refuses before another native entry.
    for offset in 0..8 {
        turn(
            &mut budget,
            14 + u64::from(offset),
            Request::Read {
                offset,
                maximum: 100 - offset,
            },
            0,
            Response::Chunk {
                offset,
                bytes: 1,
                total: 100,
            },
        );
    }
    assert!(
        budget
            .before(
                envelope(22),
                Request::Read {
                    offset: 8,
                    maximum: 92
                }
            )
            .is_err()
    );
    assert!(!budget.complete());
}
