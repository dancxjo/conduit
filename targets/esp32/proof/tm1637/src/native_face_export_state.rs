//! Read-only response storage expires independently of subsequent HTTP input.
use alloc::vec::Vec;
use conduit_environment_http::face_export::{Reply, Request};
pub(super) struct State {
    pub(super) pending: Option<Request>,
    pub(super) completed: Option<(Request, u64, Result<Vec<u8>, &'static str>)>,
}
impl State {
    pub(super) fn request(&mut self, request: Request, now: u64) -> Reply {
        self.expire(now);
        if let Some((key, _, _)) = self.completed.as_ref() {
            if *key != request {
                // Keep the finite response for its original reader. Another
                // consumer cannot discard completed work before collection.
                return Reply::Unavailable;
            }
            return match self.completed.take().unwrap().2 {
                Ok(bytes) => Reply::Offer(bytes),
                Err(
                    "face-export-memory"
                    | "face-export-no-producer"
                    | "face-export-unavailable"
                    | "face-export-not-durable",
                ) => Reply::Unavailable,
                Err(_) => Reply::Refused,
            };
        }
        if let Some(pending) = self.pending {
            return if pending == request {
                Reply::Pending
            } else {
                Reply::Unavailable
            };
        }
        self.pending = Some(request);
        Reply::Pending
    }

    pub(super) fn expire(&mut self, now: u64) {
        if self
            .completed
            .as_ref()
            .is_some_and(|(_, until, _)| now >= *until)
        {
            self.completed = None;
        }
    }
}
