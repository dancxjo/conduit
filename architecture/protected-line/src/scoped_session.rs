//! Affine custody of one original admission slot, not Line or Body authority.
use super::ProtectedSessionAdmission;
use crate::{
    establish_protected_session_async, AsyncProtectedFrameCarrier, ProtectedLineError,
    ProtectedSession, Role, SessionBinding, SessionDisposition, SessionEvidence, SessionLimits,
};

struct Slot<'a, const SESSIONS: usize> {
    pool: &'a mut ProtectedSessionAdmission<SESSIONS>,
    index: usize,
}
impl<const SESSIONS: usize> Drop for Slot<'_, SESSIONS> {
    fn drop(&mut self) {
        // admit checked this index and occupied it. The exclusive borrow prevents
        // any other owner from retiring/reassigning it during this guard's life.
        self.pool.occupied[self.index] = false;
    }
}

/// One occupied slot in the ORIGINAL pool, acquired once before handshake IO.
/// Dropping this owner, including inside an unpolled or cancelled establishment
/// future, releases that exact slot. It is not peer membership or Fore authority.
///
/// The exclusive pool borrow deliberately prevents concurrent admission through
/// that pool until the attempt/session is dropped or consumed by close/cancel.
/// The original pool cannot be reused while this affine loan is retained:
/// ```compile_fail
/// use conduit_protected_line::ProtectedSessionAdmission;
/// fn overlapping(pool: &mut ProtectedSessionAdmission<2>) {
///     let attempt = pool.scoped_attempt().unwrap();
///     pool.admit().unwrap();
///     drop(attempt);
/// }
/// ```
/// There is no internal sharing or automatic carrier close. Caller-owned IO
/// deadlines, original carrier possession and carrier retirement remain separate.
pub struct ScopedProtectedSessionAttempt<'a, const SESSIONS: usize> {
    slot: Slot<'a, SESSIONS>,
}
impl<const SESSIONS: usize> ProtectedSessionAdmission<SESSIONS> {
    pub fn scoped_attempt(
        &mut self,
    ) -> Result<ScopedProtectedSessionAttempt<'_, SESSIONS>, ProtectedLineError> {
        // Preserve the existing validated policy and capacity semantics.
        let index = self.admit()?;
        Ok(ScopedProtectedSessionAttempt {
            slot: Slot { pool: self, index },
        })
    }
}
impl<'a, const SESSIONS: usize> ScopedProtectedSessionAttempt<'a, SESSIONS> {
    /// Use this pool's exact validated policy. Any handshake error or dropped
    /// future releases its original slot; success keeps the SAME guard occupied.
    pub async fn establish_async<C: AsyncProtectedFrameCarrier>(
        self,
        carrier: &mut C,
        role: Role,
        binding: &SessionBinding,
        preshared_key: [u8; 32],
        ephemeral_private_key: [u8; 32],
    ) -> Result<ScopedProtectedSession<'a, SESSIONS>, ProtectedLineError> {
        let policy = self.slot.pool.policy();
        let session = establish_protected_session_async(
            carrier,
            role,
            binding,
            policy,
            preshared_key,
            ephemeral_private_key,
        )
        .await?;
        Ok(ScopedProtectedSession {
            session,
            slot: Some(self.slot),
        })
    }
}

/// Existing protected crypto together with its original admission occupancy.
/// No constructor accepts an arbitrary session or a claimed slot. Crypto cannot
/// be extracted or replaced; evidence grants no membership, Fore or execution.
/// The pool remains exclusively borrowed even after a terminal crypto failure
/// until this wrapper is dropped, although its slot is released immediately.
pub struct ScopedProtectedSession<'a, const SESSIONS: usize> {
    session: ProtectedSession,
    slot: Option<Slot<'a, SESSIONS>>,
}
impl<const SESSIONS: usize> ScopedProtectedSession<'_, SESSIONS> {
    pub fn limits(&self) -> SessionLimits {
        self.session.limits()
    }
    pub fn maximum_frame_bytes(&self) -> usize {
        self.session.maximum_frame_bytes()
    }
    pub fn evidence(&self) -> SessionEvidence<'_> {
        self.session.evidence()
    }
    /// Bind an upper-layer proof only while this channel retains its original
    /// slot and open crypto. The hash alone grants no peer or service authority.
    pub fn channel_binding(&self) -> Result<&[u8; 32], ProtectedLineError> {
        if self.slot.is_none() || self.session.evidence().disposition != SessionDisposition::Open {
            return Err(ProtectedLineError::Closed);
        }
        Ok(self.session.channel_binding())
    }
    pub fn seal(
        &mut self,
        plaintext: &[u8],
        output: &mut [u8],
    ) -> Result<usize, ProtectedLineError> {
        let result = self.session.seal(plaintext, output);
        self.release_if_retired();
        result
    }
    pub fn open(&mut self, frame: &[u8], output: &mut [u8]) -> Result<usize, ProtectedLineError> {
        let result = self.session.open(frame, output);
        self.release_if_retired();
        result
    }
    fn release_if_retired(&mut self) {
        if self.session.evidence().disposition != SessionDisposition::Open {
            self.slot.take();
        }
    }
    /// Retire crypto and this original slot; carrier close remains caller-owned.
    pub fn close(mut self) {
        self.session.close();
    }
    /// Retire crypto and this original slot; carrier cancellation remains caller-owned.
    pub fn cancel(mut self) {
        self.session.cancel();
    }
}
impl<const SESSIONS: usize> Drop for ScopedProtectedSession<'_, SESSIONS> {
    fn drop(&mut self) {
        // Quiesce both crypto directions before returning original capacity.
        self.session.cancel();
        self.slot.take();
    }
}

#[cfg(test)]
#[path = "scoped_session_tests.rs"]
mod tests;
