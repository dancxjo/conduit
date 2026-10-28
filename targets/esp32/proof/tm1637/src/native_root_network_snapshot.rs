//! Root-only snapshot scope. HTTP requests cannot supply Body, Boot or epoch.
use super::*;
use crate::native_play::journal::{self, snapshot};

pub(crate) fn current_chain(
    body: &conduit_body::BodyId,
    root: &[u8; 32],
) -> Result<(u64, [u8; 32]), &'static str> {
    crate::native_restart::with_network_cursor(|cursor| {
        let Some(cursor) = cursor else {
            return Ok((0, [0; 32]));
        };
        let key =
            ed25519_dalek::VerifyingKey::from_bytes(root).map_err(|_| "network-snapshot-root")?;
        cursor.offer_chain(body, &key)
    })?
}
impl NativeRoot {
    /// Root supplies its own current Boot and sealed lifecycle owner. This is
    /// not yet published by HTTP and grants no network-region erase authority.
    pub(crate) fn commit_network_snapshot_custody(
        &self,
        draft: &snapshot::Draft,
        request: &snapshot::ack::Request<'_>,
    ) -> Result<crate::native_restart::network_pending::CommittedCustody, &'static str> {
        crate::native_restart::network_pending::commit_pending(
            self.body.as_ref().ok_or("network-snapshot-body")?,
            &self.boot,
            &self.admission.verifying_key(),
            draft,
            request,
        )
    }
    pub(crate) fn replay_network_snapshot_custody(
        &self,
        request: &snapshot::ack::Request<'_>,
    ) -> Result<Option<crate::native_restart::network_pending::CommittedCustody>, &'static str>
    {
        crate::native_restart::network_pending::replay_committed(
            self.body.as_ref().ok_or("network-snapshot-body")?,
            &self.admission.verifying_key(),
            request,
        )
    }
    pub(crate) fn begin_network_snapshot(&self) -> Result<snapshot::Verifier, &'static str> {
        let owner = self.body.as_ref().ok_or("network-snapshot-body")?;
        let root = self.admission.verifying_key();
        crate::native_restart::observed::check_live_owner(owner, &root)?;
        current_chain(&owner.evidence().body_id, &root)?;
        Ok(snapshot::Verifier::start(journal::inspect()?))
    }
    pub(crate) fn endorse_network_snapshot(
        &self,
        verified: snapshot::Verified,
    ) -> Result<(snapshot::Draft, [u8; 240]), &'static str> {
        let owner = self.body.as_ref().ok_or("network-snapshot-body")?;
        let root = self.admission.verifying_key();
        let (epoch, predecessor) = current_chain(&owner.evidence().body_id, &root)?;
        let draft = verified.offer(
            snapshot::Scope {
                body: &owner.evidence().body_id,
                boot: &self.boot,
                epoch,
                predecessor,
            },
            root,
        )?;
        let signed = self
            .admission
            .sign_network_snapshot(&draft, owner, &self.boot)?;
        Ok((draft, signed))
    }
}
