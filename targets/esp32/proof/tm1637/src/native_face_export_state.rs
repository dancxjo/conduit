//! Read-only response storage expires independently of subsequent HTTP input.
use alloc::vec::Vec;
use conduit_environment_http::face_export::Request;
pub(super) struct State {
    pub(super) pending: Option<Request>,
    pub(super) completed: Option<(Request, u64, Result<Vec<u8>, &'static str>)>,
}
impl State {
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
