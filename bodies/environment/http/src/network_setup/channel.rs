//! Root-owned custody shared by socket writers and one selected private source.
//! This prepares no radio policy, authority or Play.
use super::{NetworkSetupMailbox, NetworkSetupRefusal};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    mutex::{Mutex, MutexGuard},
};

pub struct NetworkSetupChannel {
    mailbox: Mutex<CriticalSectionRawMutex, NetworkSetupMailbox>,
}
impl NetworkSetupChannel {
    pub const fn new(maximum_submissions: u32) -> Self {
        Self {
            mailbox: Mutex::new(NetworkSetupMailbox::new(maximum_submissions)),
        }
    }
    /// The mutable borrow creates one reader and the finite socket writer's
    /// handle. Root may clone writers during preparation for its admitted tasks.
    /// The reader is not Clone; its possession alone does not grant a Host Call.
    pub fn split(&mut self) -> (NetworkSetupWriter<'_>, NetworkSetupReader<'_>) {
        (
            NetworkSetupWriter(&self.mailbox),
            NetworkSetupReader(&self.mailbox),
        )
    }
}
#[derive(Clone)]
pub struct NetworkSetupWriter<'a>(&'a Mutex<CriticalSectionRawMutex, NetworkSetupMailbox>);
impl NetworkSetupWriter<'_> {
    /// Root preparation can inspect custody without waiting behind a socket.
    pub fn try_lock(
        &self,
    ) -> Result<MutexGuard<'_, CriticalSectionRawMutex, NetworkSetupMailbox>, NetworkSetupReadRefusal>
    {
        self.0.try_lock().map_err(|_| NetworkSetupReadRefusal::Busy)
    }
    /// Root must bound the entire connection, including lock acquisition. The
    /// ordinary HTTP response guard retains exclusive custody across flush.
    pub async fn lock(&self) -> MutexGuard<'_, CriticalSectionRawMutex, NetworkSetupMailbox> {
        self.0.lock().await
    }
}
pub struct NetworkSetupReader<'a>(&'a Mutex<CriticalSectionRawMutex, NetworkSetupMailbox>);
#[derive(Debug, PartialEq, Eq)]
pub enum NetworkSetupReadRefusal {
    Busy,
    Input(NetworkSetupRefusal),
}
impl NetworkSetupReader<'_> {
    /// Borrow the actual private mailbox without waiting. Selected-source
    /// preparation/transfer can preflight and consume the same acknowledged
    /// submission under one lock; a busy response guard preserves all custody.
    pub fn try_lock(
        &self,
    ) -> Result<MutexGuard<'_, CriticalSectionRawMutex, NetworkSetupMailbox>, NetworkSetupReadRefusal>
    {
        self.0.try_lock().map_err(|_| NetworkSetupReadRefusal::Busy)
    }
    /// Root preparation borrows the same input that the selected source will
    /// later consume. Socket custody and flush must already have completed.
    /// This never waits or transfers the request out of the mailbox.
    pub fn prepare_acknowledged<T>(
        &self,
        prepare: impl FnOnce(conduit_net::NetworkJoinRequest<'_>) -> T,
    ) -> Result<T, NetworkSetupReadRefusal> {
        let mailbox = self
            .0
            .try_lock()
            .map_err(|_| NetworkSetupReadRefusal::Busy)?;
        mailbox
            .prepare_acknowledged(prepare)
            .map_err(NetworkSetupReadRefusal::Input)
    }
    /// Readiness is observable only after the socket releases the response guard
    /// and mailbox lock. Busy is distinct from no acknowledged input.
    pub fn has_acknowledged(&self) -> Result<bool, NetworkSetupReadRefusal> {
        let mailbox = self
            .0
            .try_lock()
            .map_err(|_| NetworkSetupReadRefusal::Busy)?;
        Ok(mailbox.has_acknowledged())
    }
    /// Called by the selected credential owner, after its possession and current
    /// Plan/Play checks. No waiting, cloning or allocation occurs here.
    pub fn take_acknowledged(
        &mut self,
        output: &mut [u8],
    ) -> Result<usize, NetworkSetupReadRefusal> {
        let mut mailbox = self
            .0
            .try_lock()
            .map_err(|_| NetworkSetupReadRefusal::Busy)?;
        mailbox
            .take_acknowledged(output)
            .map_err(NetworkSetupReadRefusal::Input)
    }
}
