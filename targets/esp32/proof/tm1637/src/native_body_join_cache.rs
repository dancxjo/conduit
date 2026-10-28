//! Transport cache lifetime only: callers obtain deadlines from typed Root
//! challenges. Neither payload JSON nor an invalid proof supplies eviction.
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
}
