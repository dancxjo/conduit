//! One bounded transport request. Queueing and cached responses grant no custody.
use alloc::vec::Vec;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Key {
    pub(super) slot: u8,
    ack: Option<[u8; 32]>,
}
pub(super) struct Request {
    pub(super) key: Key,
    pub(super) ack: Option<Vec<u8>>,
}
impl Request {
    pub(super) fn offer(slot: u8) -> Self {
        Self {
            key: Key { slot, ack: None },
            ack: None,
        }
    }
    pub(super) fn acknowledge(slot: u8, bytes: Vec<u8>) -> Result<Self, ()> {
        if slot > 5 || bytes.is_empty() || bytes.len() > 768 {
            return Err(());
        }
        Ok(Self {
            key: Key {
                slot,
                ack: Some(Sha256::digest(&bytes).into()),
            },
            ack: Some(bytes),
        })
    }
}
pub(super) enum Response<T> {
    Pending,
    Completed(Result<T, ()>),
    Busy,
}
/// Diagnostic counts wrap without affecting request custody or queue decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Counts {
    pub(super) submissions: u32,
    pub(super) cache_hits: u32,
    pub(super) takes: u32,
    pub(super) completions: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub(super) counts: Counts,
    // 0xff = absent; bit 7 = ACK, remaining bits = slot. No hashes or payloads.
    pub(super) pending: u8,
    pub(super) inflight: u8,
    pub(super) completed: u8,
    // 0 = absent, 1 = accepted, 2 = refused.
    pub(super) result: u8,
}
fn diagnostic_key(key: Option<Key>) -> u8 {
    key.map_or(0xff, |key| {
        key.slot | if key.ack.is_some() { 0x80 } else { 0 }
    })
}
pub(super) struct Queue<T> {
    counts: Counts,
    pending: Option<Request>,
    inflight: Option<Key>,
    completed: Option<(Key, Result<T, ()>)>,
}
impl<T: Copy> Queue<T> {
    pub(super) const fn new() -> Self {
        Self {
            counts: Counts {
                submissions: 0,
                cache_hits: 0,
                takes: 0,
                completions: 0,
            },
            pending: None,
            inflight: None,
            completed: None,
        }
    }
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            counts: self.counts,
            pending: diagnostic_key(self.pending.as_ref().map(|request| request.key)),
            inflight: diagnostic_key(self.inflight),
            completed: diagnostic_key(self.completed.as_ref().map(|(key, _)| *key)),
            result: self
                .completed
                .as_ref()
                .map_or(0, |(_, result)| if result.is_ok() { 1 } else { 2 }),
        }
    }
    pub(super) fn submit(&mut self, request: Request) -> Response<T> {
        self.counts.submissions = self.counts.submissions.wrapping_add(1);
        if let Some((key, result)) = &self.completed {
            if *key == request.key {
                self.counts.cache_hits = self.counts.cache_hits.wrapping_add(1);
                let result = *result;
                // An empty slot can become closed later. A refusal is delivered
                // once; accepted receipts remain retryable after a lost response.
                if result.is_err() {
                    self.completed = None;
                }
                return Response::Completed(result);
            }
        }
        if let Some(key) = self
            .pending
            .as_ref()
            .map(|request| request.key)
            .or(self.inflight)
        {
            return if key == request.key {
                Response::Pending
            } else {
                Response::Busy
            };
        }
        // A different request replaces the single response cache immediately.
        // Retaining the prior JSON offer while an ACK owns its request and
        // inspector overlaps two preparation lifetimes and can consume the
        // independently admitted Mask child workspace on the real ESP32.
        self.completed = None;
        self.pending = Some(request);
        Response::Pending
    }
    pub(super) fn take(&mut self) -> Option<Request> {
        if self.inflight.is_some() {
            return None;
        }
        let request = self.pending.take()?;
        self.counts.takes = self.counts.takes.wrapping_add(1);
        self.inflight = Some(request.key);
        Some(request)
    }
    pub(super) fn complete(&mut self, key: Key, result: Result<T, ()>) {
        assert!(
            self.inflight == Some(key),
            "completion must own the in-flight request"
        );
        self.counts.completions = self.counts.completions.wrapping_add(1);
        self.inflight = None;
        self.completed = Some((key, result));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn ack(slot: u8, bytes: &[u8]) -> Request {
        Request::acknowledge(slot, bytes.to_vec()).unwrap()
    }
    #[test]
    fn diagnostics_follow_real_transitions_without_consuming_work() {
        let mut queue = Queue::<u8>::new();
        let empty = Snapshot {
            counts: Counts::default(),
            pending: 255,
            inflight: 255,
            completed: 255,
            result: 0,
        };
        assert_eq!(queue.snapshot(), empty);
        assert!(matches!(
            queue.submit(ack(2, b"private")),
            Response::Pending
        ));
        assert_eq!(queue.snapshot().pending, 130);
        assert!(matches!(queue.submit(Request::offer(2)), Response::Busy));
        let before = queue.snapshot();
        assert_eq!(
            queue.snapshot(),
            before,
            "inspection cannot grant or consume work"
        );
        let request = queue.take().unwrap();
        assert!(queue.take().is_none());
        assert_eq!(queue.snapshot().inflight, 130);
        assert_eq!(queue.snapshot().pending, 255);
        queue.complete(request.key, Err(()));
        assert_eq!(queue.snapshot().completed, 130);
        assert_eq!(queue.snapshot().result, 2);
        assert!(matches!(
            queue.submit(ack(2, b"private")),
            Response::Completed(Err(()))
        ));
        assert_eq!(
            queue.snapshot(),
            Snapshot {
                counts: Counts {
                    submissions: 3,
                    cache_hits: 1,
                    takes: 1,
                    completions: 1
                },
                ..empty
            }
        );
        assert!(queue.take().is_none());
        assert!(matches!(queue.submit(Request::offer(0)), Response::Pending));
        let request = queue.take().unwrap();
        queue.complete(request.key, Ok(7));
        assert!(matches!(
            queue.submit(Request::offer(0)),
            Response::Completed(Ok(7))
        ));
        assert_eq!(queue.snapshot().completed, 0);
        assert_eq!(queue.snapshot().result, 1);
        assert_eq!(
            queue.snapshot().counts,
            Counts {
                submissions: 5,
                cache_hits: 2,
                takes: 2,
                completions: 2
            }
        );
    }
    #[test]
    fn diagnostic_counters_wrap_without_changing_protocol() {
        assert_eq!(core::mem::size_of::<Counts>(), 16);
        let mut queue = Queue::<u8>::new();
        queue.counts = Counts {
            submissions: u32::MAX,
            cache_hits: u32::MAX,
            takes: u32::MAX,
            completions: u32::MAX,
        };
        assert!(matches!(queue.submit(Request::offer(0)), Response::Pending));
        let request = queue.take().unwrap();
        queue.complete(request.key, Ok(9));
        assert!(matches!(
            queue.submit(Request::offer(0)),
            Response::Completed(Ok(9))
        ));
        assert_eq!(
            queue.snapshot().counts,
            Counts {
                submissions: 1,
                ..Counts::default()
            }
        );
    }
    #[test]
    fn duplicate_request_is_pending_and_other_requests_cannot_replace_actual_work() {
        let mut queue = Queue::<[u8; 7]>::new();
        assert!(matches!(
            queue.submit(ack(1, b"original")),
            Response::Pending
        ));
        assert!(matches!(queue.submit(ack(1, b"changed")), Response::Busy));
        assert!(matches!(queue.submit(Request::offer(1)), Response::Busy));
        let request = queue.take().unwrap();
        assert_eq!(request.ack.as_deref(), Some(&b"original"[..]));
        assert!(queue.take().is_none());
        assert!(matches!(
            queue.submit(ack(1, b"original")),
            Response::Pending
        ));
        queue.complete(request.key, Ok(*b"receipt"));
        for _ in 0..2 {
            assert!(
                matches!(queue.submit(ack(1,b"original")), Response::Completed(Ok(bytes)) if bytes==*b"receipt")
            );
        }
        assert!(queue.take().is_none());
        assert!(matches!(
            queue.submit(ack(2, b"original")),
            Response::Pending
        ));
        assert!(
            queue.completed.is_none(),
            "prior response cannot overlap new preparation"
        );
    }
    #[test]
    fn refused_empty_slot_can_be_inspected_again_after_rotation() {
        let mut queue = Queue::<[u8; 7]>::new();
        queue.submit(Request::offer(3));
        let request = queue.take().unwrap();
        queue.complete(request.key, Err(()));
        assert!(matches!(
            queue.submit(Request::offer(3)),
            Response::Completed(Err(()))
        ));
        assert!(matches!(queue.submit(Request::offer(3)), Response::Pending));
        assert!(queue.take().is_some());
    }
    #[test]
    fn transport_bounds_are_checked_before_queue_ownership() {
        for (slot, length) in [(6, 1), (1, 0), (1, 769)] {
            assert!(Request::acknowledge(slot, alloc::vec![0;length]).is_err());
        }
        assert!(Request::acknowledge(5, alloc::vec![0;768]).is_ok());
    }
}
