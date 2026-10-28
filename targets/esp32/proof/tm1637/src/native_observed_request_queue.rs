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
pub(super) struct Queue<T> {
    pending: Option<Request>,
    inflight: Option<Key>,
    completed: Option<(Key, Result<T, ()>)>,
}
impl<T: Copy> Queue<T> {
    pub(super) const fn new() -> Self {
        Self {
            pending: None,
            inflight: None,
            completed: None,
        }
    }
    pub(super) fn submit(&mut self, request: Request) -> Response<T> {
        if let Some((key, result)) = &self.completed {
            if *key == request.key {
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
        self.inflight = Some(request.key);
        Some(request)
    }
    pub(super) fn complete(&mut self, key: Key, result: Result<T, ()>) {
        assert!(
            self.inflight == Some(key),
            "completion must own the in-flight request"
        );
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
