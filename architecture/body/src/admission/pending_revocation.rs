//! Retirement of one owner-held invitation before provisioning is delivered.
use super::{AdmissionManager, SpawnInvitation};
use ed25519_dalek::SigningKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingInvitationRevocationRefusal {
    WrongBody,
    UnknownInvitation,
    ClaimMismatch,
    SecretMismatch,
    AlreadyUsed,
}

impl AdmissionManager {
    /// Remove only an exact, unused invitation whose private secret is retained
    /// by this owner. Expired invitations may also be retired. The monotonic
    /// issuance sequence, other pending work, receipts and Parts are untouched.
    ///
    /// The owner must drain any external provisioning/delivery job first and
    /// must not revoke after committing delivery of the invitation. This call
    /// does not cancel a transport or retract already-delivered private bytes.
    /// Spawn admission is synchronous under the same exclusive manager borrow:
    /// revocation first makes a queued proof unknown; completed admission first
    /// makes revocation refuse. No admitted Part is revoked by this operation.
    pub fn revoke_pending_spawn_invitation(
        &mut self,
        invitation: &SpawnInvitation,
    ) -> Result<(), PendingInvitationRevocationRefusal> {
        use PendingInvitationRevocationRefusal::*;
        if invitation.body_id != self.body_id {
            return Err(WrongBody);
        }
        let index = self
            .invitations
            .iter()
            .position(|record| record.invitation_id == invitation.invitation_id)
            .ok_or(UnknownInvitation)?;
        let record = &self.invitations[index];
        if record.nonce != invitation.nonce
            || record.issued_at_millis != invitation.issued_at_millis
            || record.expires_at_millis != invitation.expires_at_millis
        {
            return Err(ClaimMismatch);
        }
        if SigningKey::from_bytes(&invitation.secret.0)
            .verifying_key()
            .to_bytes()
            != record.verifying_key
        {
            return Err(SecretMismatch);
        }
        if record.used {
            return Err(AlreadyUsed);
        }
        self.invitations.remove(index);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
