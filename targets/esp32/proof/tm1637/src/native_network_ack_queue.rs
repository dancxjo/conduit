//! One bounded binary acknowledgment. Queue identity never grants custody.
use alloc::vec::Vec;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key([u8; 32]);
pub(crate) struct Request {
    pub(crate) key: Key,
    pub(crate) bytes: Vec<u8>,
}
#[derive(Clone, Copy)]
pub(crate) struct Receipt {
    pub(crate) generation: u64,
    pub(crate) offer: [u8; 32],
}
pub(crate) enum Response {
    Pending,
    Busy,
    Refused,
    Completed(Receipt),
}
pub(crate) struct Queue {
    pending: Option<Request>,
    inflight: Option<Key>,
    completed: Option<(Key, Result<Receipt, ()>)>,
}
impl Queue {
    pub(crate) const fn new() -> Self {
        Self {
            pending: None,
            inflight: None,
            completed: None,
        }
    }
    pub(crate) fn submit(&mut self, bytes: Vec<u8>) -> Response {
        if bytes.len() != 628 {
            return Response::Refused;
        }
        let key = Key(Sha256::digest(&bytes).into());
        if let Some((prior, result)) = self.completed {
            if prior == key {
                return match result {
                    Ok(receipt) => Response::Completed(receipt),
                    Err(()) => {
                        self.completed = None;
                        Response::Refused
                    }
                };
            }
        }
        if let Some(prior) = self.pending.as_ref().map(|r| r.key).or(self.inflight) {
            return if prior == key {
                Response::Pending
            } else {
                Response::Busy
            };
        }
        self.completed = None;
        self.pending = Some(Request { key, bytes });
        Response::Pending
    }
    pub(crate) fn take(&mut self) -> Option<Request> {
        if self.inflight.is_some() {
            return None;
        }
        let request = self.pending.take()?;
        self.inflight = Some(request.key);
        Some(request)
    }
    pub(crate) fn complete(&mut self, key: Key, result: Result<Receipt, ()>) {
        assert!(
            self.inflight == Some(key),
            "completion must own queued acknowledgment"
        );
        self.inflight = None;
        self.completed = Some((key, result));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_preserve_actual_request_and_commit_cache() {
        let mut q = Queue::new();
        assert!(matches!(q.submit(alloc::vec![1;628]), Response::Pending));
        let r = q.take().unwrap();
        assert!(matches!(q.submit(alloc::vec![2;628]), Response::Busy));
        assert!(matches!(q.submit(alloc::vec![1;628]), Response::Pending));
        assert!(q.take().is_none());
        q.complete(
            r.key,
            Ok(Receipt {
                generation: 7,
                offer: [3; 32],
            }),
        );
        for _ in 0..2 {
            assert!(matches!(
                q.submit(alloc::vec![1;628]),
                Response::Completed(Receipt { generation: 7, .. })
            ));
        }
        assert!(matches!(q.submit(alloc::vec![2;628]), Response::Pending));
        assert!(q.completed.is_none());
    }
    #[test]
    fn invalid_extent_never_queues_and_refusals_can_be_retried() {
        let mut q = Queue::new();
        for n in [0, 627, 629] {
            assert!(matches!(q.submit(alloc::vec![0;n]), Response::Refused));
        }
        assert!(q.take().is_none());
        q.submit(alloc::vec![1;628]);
        let r = q.take().unwrap();
        q.complete(r.key, Err(()));
        assert!(matches!(q.submit(alloc::vec![1;628]), Response::Refused));
        assert!(matches!(q.submit(alloc::vec![1;628]), Response::Pending));
    }
}
