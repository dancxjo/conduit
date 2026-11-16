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
        let index = self.owned_pending_spawn_invitation_index(invitation)?;
        self.invitations.remove(index);
        Ok(())
    }

    /// Check exact owner custody of an unused pending invitation without mutation.
    /// This is a point-in-time check, not a reservation, lease or issuer grant.
    /// It does not check the clock or establish time validity. Revalidate against
    /// this same manager and perform the existing expiry checks before sealing
    /// or delivery, including after any await.
    pub fn validate_owned_pending_spawn_invitation(
        &self,
        invitation: &SpawnInvitation,
    ) -> Result<(), PendingInvitationRevocationRefusal> {
        self.owned_pending_spawn_invitation_index(invitation)
            .map(|_| ())
    }

    fn owned_pending_spawn_invitation_index(
        &self,
        invitation: &SpawnInvitation,
    ) -> Result<usize, PendingInvitationRevocationRefusal> {
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
        Ok(index)
    }
}

#[cfg(test)]
mod tests;
