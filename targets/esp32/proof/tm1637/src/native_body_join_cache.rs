//! Transport cache lifetime only: callers obtain deadlines from typed Root
//! challenges. Neither payload JSON nor an invalid proof supplies eviction.
/// Best-effort retirement of response construction slack after successful
/// serialization. Allocation pressure must preserve the original receipt owner.
pub(super) fn compact(bytes: alloc::vec::Vec<u8>) -> alloc::vec::Vec<u8> {
    if bytes.capacity() == bytes.len() {
        return bytes;
    }
    let mut exact = alloc::vec::Vec::new();
    if exact.try_reserve_exact(bytes.len()).is_err() {
        return bytes;
    }
    exact.extend_from_slice(&bytes);
    exact
}

/// Match canonical challenge expiry: the exact deadline remains usable, and
/// only now > deadline expires. None denotes a successful durable receipt.
pub(super) fn prune<T>(
    completed: &mut Option<T>,
    now: u64,
    deadline: impl FnOnce(&T) -> Option<u64>,
) -> bool {
    if completed
        .as_ref()
        .and_then(deadline)
        .is_some_and(|at| now > at)
    {
        *completed = None;
        true
    } else {
        false
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, PartialEq)]
    struct Completed {
        ticket: u32,
        digest: [u8; 32],
        bytes: Vec<u8>,
        deadline: Option<u64>,
    }
    struct Mailbox {
        completed: Option<Completed>,
        pending: Option<u32>,
        inflight: Option<u32>,
    }
    fn mailbox(deadline: Option<u64>) -> Mailbox {
        Mailbox {
            completed: Some(Completed {
                ticket: 2,
                digest: [7; 32],
                bytes: b"Root signed result".to_vec(),
                deadline,
            }),
            pending: None,
            inflight: None,
        }
    }
    #[test]
    fn signed_challenge_cache_survives_exact_deadline_but_not_later() {
        let mut state = mailbox(Some(372_838));
        assert!(!prune(&mut state.completed, 342_838, |done| done.deadline));
        assert!(!prune(&mut state.completed, 372_838, |done| done.deadline));
        assert_eq!(state.completed.as_ref().unwrap().ticket, 2);
        assert_eq!(state.completed.as_ref().unwrap().digest, [7; 32]);
        assert!(prune(&mut state.completed, 372_839, |done| done.deadline));
        assert!(state.completed.is_none());
    }
    #[test]
    fn durable_receipt_bytes_remain_retained_without_a_challenge_deadline() {
        let mut state = mailbox(None);
        let before = state.completed.as_ref().unwrap().bytes.clone();
        assert!(!prune(&mut state.completed, u64::MAX, |done| done.deadline));
        assert_eq!(state.completed.as_ref().unwrap().bytes, before);
        assert_eq!(state.completed.as_ref().unwrap().ticket, 2);
    }
    #[test]
    fn pruning_old_challenge_leaves_pending_and_inflight_custody_untouched() {
        let mut state = mailbox(Some(10));
        state.pending = Some(3);
        state.inflight = Some(4);
        assert!(prune(&mut state.completed, 11, |done| done.deadline));
        assert_eq!(state.pending, Some(3));
        assert_eq!(state.inflight, Some(4));
        assert!(!prune(&mut state.completed, 12, |done| done.deadline));
    }

    #[test]
    fn completed_join_receipt_compaction_preserves_bytes_identity_and_lifetime_under_pressure() {
        struct Completed {
            ticket: u32,
            digest: [u8; 32],
            deadline: Option<u64>,
            bytes: Vec<u8>,
        }
        for refuse in [false, true] {
            let mut bytes = Vec::with_capacity(2048);
            bytes.extend_from_slice(b"exact already committed signed receipt");
            let original_pointer = bytes.as_ptr();
            let original_capacity = bytes.capacity();
            let length = bytes.len();
            let refusal =
                refuse.then(|| crate::allocation_probe::refuse_next_allocation_size(length));
            let (bytes, allocation) = crate::allocation_probe::observe(|| compact(bytes));
            drop(refusal);
            assert_eq!(bytes, b"exact already committed signed receipt");
            if refuse {
                assert_eq!(bytes.as_ptr(), original_pointer);
                assert_eq!(bytes.capacity(), original_capacity);
                assert_eq!(allocation.allocations, 0);
                assert_eq!(allocation.deallocations, 0);
            } else {
                assert_eq!(bytes.capacity(), length);
                assert_eq!(allocation.deallocated_bytes, original_capacity);
                assert_eq!(allocation.allocations, 1);
            }
            let mut completed = Some(Completed {
                ticket: 17,
                digest: [42; 32],
                deadline: None,
                bytes,
            });
            assert!(!prune(&mut completed, u64::MAX, |done| done.deadline));
            let done = completed.as_ref().unwrap();
            assert_eq!(done.ticket, 17);
            assert_eq!(done.digest, [42; 32]);
            assert_eq!(done.bytes, b"exact already committed signed receipt");
            completed.as_mut().unwrap().deadline = Some(100);
            assert!(!prune(&mut completed, 100, |done| done.deadline));
            assert!(prune(&mut completed, 101, |done| { done.deadline }));
        }
    }
}
