//! One volatile page submission for the existing network/credentials Host Call.
//! This mailbox grants no authority, changes no radio mode and schedules no Play.
mod channel;
mod form;
mod page;
mod request;
pub use channel::{
    NetworkSetupChannel, NetworkSetupReadRefusal, NetworkSetupReader, NetworkSetupWriter,
};
pub use form::{NetworkSetupInput, NetworkSetupLimits, NetworkSetupRefusal};
pub use page::{network_setup_page, PendingNetworkResponse};
pub use request::{NetworkHttpRefusal, NetworkHttpRequest, NetworkRequestBuffer};

/// Opaque local correlation, not a Host/Boot identity or an authority token.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Submission(u32);
struct Pending {
    input: NetworkSetupInput,
    submission: Submission,
    acknowledged: bool,
}
pub struct NetworkSetupMailbox {
    pending: Option<Pending>,
    accepted: u32,
    maximum_submissions: u32,
}
impl NetworkSetupMailbox {
    pub const fn new(maximum_submissions: u32) -> Self {
        Self {
            pending: None,
            accepted: 0,
            maximum_submissions,
        }
    }
    /// The HTTP response must be successfully flushed before acknowledge. The
    /// independently admitted credentials Host Call may consume it only afterward.
    pub fn submit(&mut self, input: NetworkSetupInput) -> Result<Submission, NetworkSetupRefusal> {
        if self.pending.is_some() {
            return Err(NetworkSetupRefusal::Busy);
        }
        if self.accepted >= self.maximum_submissions {
            return Err(NetworkSetupRefusal::Exhausted);
        }
        self.accepted += 1;
        let submission = Submission(self.accepted);
        self.pending = Some(Pending {
            input,
            submission,
            acknowledged: false,
        });
        Ok(submission)
    }
    pub fn acknowledge(&mut self, submission: Submission) -> Result<(), NetworkSetupRefusal> {
        let pending = self
            .pending
            .as_mut()
            .ok_or(NetworkSetupRefusal::NoSubmission)?;
        if pending.submission != submission {
            return Err(NetworkSetupRefusal::StaleSubmission);
        }
        if pending.acknowledged {
            return Err(NetworkSetupRefusal::AlreadyAcknowledged);
        }
        pending.acknowledged = true;
        Ok(())
    }
    pub fn cancel(&mut self, submission: Submission) -> Result<(), NetworkSetupRefusal> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(NetworkSetupRefusal::NoSubmission)?;
        if pending.submission != submission {
            return Err(NetworkSetupRefusal::StaleSubmission);
        }
        self.pending = None;
        Ok(())
    }
    /// Root can use this readiness fact to start the independently admitted
    /// control Play. It does not expose the request or select any implementation.
    pub fn has_acknowledged(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.acknowledged)
    }
    /// Root preparation may borrow the acknowledged request to construct the
    /// selected implementation's SDK configuration before Play. This is not a
    /// credentials Host Call: it neither consumes input nor authorizes effects.
    /// The callback is synchronous, and its result cannot borrow secret storage.
    /// Preparation failure preserves the request for explicit cancellation or
    /// a later selected read. Root must separately bound preparation allocation.
    ///
    /// Secret references cannot escape the preparation callback:
    /// ```compile_fail
    /// use conduit_environment_http::network_setup::NetworkSetupMailbox;
    /// let mailbox = NetworkSetupMailbox::new(1);
    /// let leaked = mailbox.prepare_acknowledged(|request| request.credential);
    /// ```
    pub fn prepare_acknowledged<T>(
        &self,
        prepare: impl FnOnce(conduit_net::NetworkJoinRequest<'_>) -> T,
    ) -> Result<T, NetworkSetupRefusal> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(NetworkSetupRefusal::NoSubmission)?;
        if !pending.acknowledged {
            return Err(NetworkSetupRefusal::NotAcknowledged);
        }
        Ok(prepare(pending.input.request()))
    }
    /// Only the selected credentials Host Call should invoke this. The caller
    /// owns the admitted output buffer and its subsequent secret-byte lifetime.
    /// Output pressure preserves the pending request; successful transfer consumes it.
    pub fn take_acknowledged(&mut self, output: &mut [u8]) -> Result<usize, NetworkSetupRefusal> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(NetworkSetupRefusal::NoSubmission)?;
        if !pending.acknowledged {
            return Err(NetworkSetupRefusal::NotAcknowledged);
        }
        let length = conduit_net::encode_network_join_request(pending.input.request(), output)
            .map_err(NetworkSetupRefusal::Wire)?;
        self.pending = None;
        Ok(length)
    }
}
