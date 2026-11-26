//! Environmental HTTP representation over shared bounded Show lease custody.
use crate::PreparedHttpShow;
pub use conduit_face::ShowCustodyRefusal as HttpShowRefusal;
pub struct HttpShowMailbox {
    inner: conduit_face::ShowMailbox<PreparedHttpShow>,
}
pub struct HttpShowPublisher<'a> {
    inner: conduit_face::ShowPublisher<'a, PreparedHttpShow>,
}
pub struct HttpShowControl<'a> {
    inner: conduit_face::ShowControl<'a, PreparedHttpShow>,
}
impl Default for HttpShowMailbox {
    fn default() -> Self {
        Self::new()
    }
}
impl HttpShowMailbox {
    pub const fn new() -> Self {
        Self {
            inner: conduit_face::ShowMailbox::new(),
        }
    }
    pub fn claim(
        &self,
        maximum_publications: u32,
        maximum_lifetime_ms: u64,
    ) -> Result<(HttpShowPublisher<'_>, HttpShowControl<'_>), HttpShowRefusal> {
        let (publisher, control) = self
            .inner
            .claim(maximum_publications, maximum_lifetime_ms)?;
        Ok((
            HttpShowPublisher { inner: publisher },
            HttpShowControl { inner: control },
        ))
    }
    pub fn response(
        &self,
        ap_ready: bool,
        now_ms: u64,
        json: bool,
    ) -> Result<Option<heapless::String<2048>>, HttpShowRefusal> {
        self.inner
            .with_current(now_ms, |show| show.response(ap_ready, now_ms, json))
    }
}
impl HttpShowPublisher<'_> {
    pub fn publish(
        &mut self,
        show: PreparedHttpShow,
        from_ms: u64,
        until_ms: u64,
    ) -> Result<(), HttpShowRefusal> {
        let revision = show.face_revision();
        self.inner.publish(show, revision, from_ms, until_ms)
    }
}
impl HttpShowControl<'_> {
    pub fn withdraw(&self) {
        self.inner.withdraw()
    }
}
#[cfg(test)]
#[path = "show_mailbox_tests.rs"]
mod tests;
