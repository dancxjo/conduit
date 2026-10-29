//! Incremental exact-ACK exchange. An interrupted session never renews its budget.
use super::{receipt::CommittedNetworkCustody, *};
use std::{
    os::fd::AsRawFd,
    time::{Duration, Instant},
};
const ATTEMPTS: usize = 451;
const LIFETIME: Duration = Duration::from_secs(900);
const SESSION: &str = "incremental-ack-session.bin";
const COMPLETED: &str = "incremental-ack-completed.json";

pub(crate) enum Start {
    Ready(Acknowledgement),
    Complete(CommittedNetworkCustody),
    /// The original session may have reached Root. Evidence and exact ACK are
    /// retained, but this entrance cannot resume or renew its transaction window.
    Interrupted,
}
// One exact 480-byte signed cursor is a finite inline proof. Boxing only the
// successful result would add a lifecycle allocation without bounding more work.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Progress {
    Pending,
    Complete(CommittedNetworkCustody),
}
pub(crate) struct Acknowledgement {
    token: VerifiedDurableSnapshot,
    credential: MembershipCredential,
    ack: Vec<u8>,
    _lock: File,
    deadline: Instant,
    remaining: usize,
    terminal: bool,
}
impl Acknowledgement {
    /// The ACK must already come from the ordinary admitted signer. This method
    /// checks exact scope/bytes; only Root verifies current signing authority.
    pub(crate) fn open(
        token: VerifiedDurableSnapshot,
        credential: MembershipCredential,
        ack: Vec<u8>,
    ) -> Result<Start> {
        token.revalidate()?;
        if ack.len() != REQUEST
            || token
                .encode_fields(&credential, ack[564..].try_into().unwrap())?
                .as_slice()
                != ack.as_slice()
        {
            return Err("network durable ACK scope".into());
        }
        let lock_path = token.directory.join("incremental-ack.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&lock_path)?;
        private(&lock_path, false)?;
        // SAFETY: retained owned descriptor. File drop releases its lock.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("network ACK already running".into());
        }
        immutable(&token.directory.join("ack.bin"), &ack)?;
        File::open(&token.directory)?.sync_all()?;
        if reopen(&token.directory.join("ack.bin"), REQUEST)? != ack {
            return Err("network durable ACK changed".into());
        }
        let mut marker = [0; 88];
        marker[..8].copy_from_slice(b"NCACKR1\0");
        marker[8..40].copy_from_slice(&token.offer);
        marker[40..72].copy_from_slice(&digest(&ack));
        marker[72..80].copy_from_slice(&(ATTEMPTS as u64).to_le_bytes());
        marker[80..88].copy_from_slice(&LIFETIME.as_secs().to_le_bytes());
        let marker_path = token.directory.join(SESSION);
        if marker_path.try_exists()? {
            if reopen(&marker_path, marker.len())? != marker {
                return Err("network ACK session correlation differs".into());
            }
            let completed = token.directory.join(COMPLETED);
            if completed.try_exists()? {
                let response = live::input(&completed, 1408)?;
                let receipt = token.retain_committed_receipt(&credential, &ack, &response)?;
                if receipt.bytes()[8] != 2 {
                    return Err("network ACK completion is not Clean".into());
                }
                return Ok(Start::Complete(receipt));
            }
            return Ok(Start::Interrupted);
        }
        if token.directory.join(COMPLETED).try_exists()? {
            return Err("network ACK completion has no session".into());
        }
        // Reserve the WHOLE finite allowance durably before any POST. This
        // marker is never removed/replaced; an interrupted process cannot gain
        // even one fresh attempt, regardless of wall-clock changes.
        immutable(&marker_path, &marker)?;
        File::open(&token.directory)?.sync_all()?;
        if reopen(&marker_path, marker.len())? != marker {
            return Err("network ACK session changed".into());
        }
        Ok(Start::Ready(Self {
            token,
            credential,
            ack,
            _lock: lock,
            deadline: Instant::now() + LIFETIME,
            remaining: ATTEMPTS,
            terminal: false,
        }))
    }
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Exactly zero or one POST, carrying the same reopened ACK. The transport
    /// must enforce the supplied min(caller budget, original transaction end).
    /// Late responses cannot establish completion. Retry pacing is caller-owned.
    pub(crate) fn advance(
        &mut self,
        operation_deadline: Instant,
        post: impl FnMut(&[u8], Instant) -> Result<(u16, Vec<u8>)>,
    ) -> Result<Progress> {
        let result = self.advance_inner(operation_deadline, post);
        if result.is_err() {
            self.terminal = true;
        }
        result
    }
    fn advance_inner(
        &mut self,
        operation_deadline: Instant,
        mut post: impl FnMut(&[u8], Instant) -> Result<(u16, Vec<u8>)>,
    ) -> Result<Progress> {
        if self.terminal {
            return Err("network ACK already terminal".into());
        }
        let now = Instant::now();
        if now >= self.deadline {
            return Err("network custody absolute deadline".into());
        }
        if self.remaining == 0 {
            return Err("network custody retry bound".into());
        }
        if operation_deadline <= now {
            return Ok(Progress::Pending);
        }
        self.token.revalidate()?;
        if reopen(&self.token.directory.join("ack.bin"), REQUEST)?.as_slice() != self.ack.as_slice()
        {
            return Err("network retry ACK changed".into());
        }
        let deadline = operation_deadline.min(self.deadline());
        if Instant::now() >= deadline {
            return Ok(Progress::Pending);
        }
        self.remaining -= 1;
        let response = post(&self.ack, deadline);
        if Instant::now() >= deadline {
            return Ok(Progress::Pending);
        }
        if response.as_ref().is_ok_and(|(_, bytes)| bytes.len() > 1408) {
            return Err("network receipt response capacity".into());
        }
        match response {
            Ok((200, response)) => {
                let receipt =
                    self.token
                        .retain_committed_receipt(&self.credential, &self.ack, &response)?;
                if receipt.bytes()[8] != 2 {
                    return Ok(Progress::Pending);
                }
                immutable(&self.token.directory.join(COMPLETED), &response)?;
                File::open(&self.token.directory)?.sync_all()?;
                let reopened = reopen(&self.token.directory.join(COMPLETED), response.len())?;
                let verified =
                    self.token
                        .verify_committed_receipt(&self.credential, &self.ack, &reopened)?;
                if verified.bytes() != receipt.bytes() {
                    return Err("network ACK completion differs".into());
                }
                self.terminal = true;
                Ok(Progress::Complete(verified))
            }
            Ok((202 | 503, _)) | Err(_) => Ok(Progress::Pending),
            Ok((status, _)) => Err(format!("network custody HTTP status {status}").into()),
        }
    }
}
#[cfg(test)]
#[path = "ack_tests.rs"]
mod tests;
