//! One affine request slot; its original deadline never restarts on completion.
use alloc::vec::Vec;
use conduit_environment_http::face_export::{Reply, Request};
const LIFETIME_MS: u64 = 5000;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    pub(super) request: Request,
    until: u64,
}
pub(super) struct Work(Key);
impl Work {
    pub(super) fn request(&self) -> Request {
        self.0.request
    }
    pub(super) fn while_current<T>(
        &self,
        now: u64,
        consume: impl FnOnce() -> Result<T, &'static str>,
    ) -> Result<T, &'static str> {
        if !self.is_current(now) {
            return Err("face-export-expired");
        }
        consume()
    }
    pub(super) fn is_current(&self, now: u64) -> bool {
        now < self.0.until
    }
}
enum Slot {
    Empty,
    Pending(Key),
    InFlight(Key),
    Completed(Key, Result<Vec<u8>, &'static str>),
}
pub(super) struct State {
    slot: Slot,
}
impl State {
    pub(super) const fn new() -> Self {
        Self { slot: Slot::Empty }
    }
    pub(super) fn request(&mut self, request: Request, now: u64) -> Reply {
        self.expire(now);
        match &self.slot {
            Slot::Empty => {
                let Some(until) = now.checked_add(LIFETIME_MS) else {
                    return Reply::Unavailable;
                };
                self.slot = Slot::Pending(Key { request, until });
                Reply::Pending
            }
            Slot::Pending(work) | Slot::InFlight(work) => {
                if work.request == request && now < work.until {
                    Reply::Pending
                } else {
                    Reply::Unavailable
                }
            }
            Slot::Completed(work, _) if work.request != request => Reply::Unavailable,
            Slot::Completed(_, _) => {
                let Slot::Completed(_, result) = core::mem::replace(&mut self.slot, Slot::Empty)
                else {
                    unreachable!()
                };
                match result {
                    Ok(bytes) => Reply::Offer(bytes),
                    Err(
                        "face-export-memory"
                        | "face-export-no-producer"
                        | "face-export-unavailable"
                        | "face-export-not-durable",
                    ) => Reply::Unavailable,
                    Err(_) => Reply::Refused,
                }
            }
        }
    }
    pub(super) fn begin(&mut self, now: u64) -> Option<Work> {
        self.expire(now);
        let Slot::Pending(work) = self.slot else {
            return None;
        };
        self.slot = Slot::InFlight(work);
        Some(Work(work))
    }
    pub(super) fn complete(&mut self, work: Work, now: u64, result: Result<Vec<u8>, &'static str>) {
        if !matches!(&self.slot, Slot::InFlight(actual) if *actual == work.0) {
            return;
        }
        self.slot = if work.is_current(now) {
            Slot::Completed(work.0, result)
        } else {
            Slot::Empty
        };
    }
    pub(super) fn expire(&mut self, now: u64) {
        // In-flight ownership cannot be released until its actual work returns.
        if matches!(&self.slot, Slot::Pending(work) | Slot::Completed(work, _) if now >= work.until)
        {
            self.slot = Slot::Empty;
        }
    }
}
