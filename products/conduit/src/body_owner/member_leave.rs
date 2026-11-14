//! Authenticated withdrawal of one current member attachment, shared by Masks.
//! The ingress must authenticate its participant or the installed owner before
//! calling this operation. A public credential alone is not leave authority.
use super::{state, Owner};
use conduit_body::{BodyBiographyEvidence, MembershipCredential};
use std::path::Path;

impl Owner {
    pub(crate) fn member_leave(
        &mut self,
        root: &Path,
        credential: &MembershipCredential,
    ) -> Result<BodyBiographyEvidence, String> {
        if credential.body_id != self.session.evidence().body_id
            || credential.host_id == self.host.advertisement().host_id
        {
            return Err("member leave differs from a remote Body member".into());
        }
        let current = self
            .session
            .evidence()
            .membership
            .parts
            .iter()
            .find(|part| part.part_id == credential.part_id)
            .filter(|part| part.state == conduit_body::MembershipState::Admitted)
            .and_then(|part| part.current.as_ref())
            .ok_or("member Part is no longer current")?;
        if current.host_id != credential.host_id || current.boot_id != credential.boot_id {
            return Err("member leave differs from current Host Boot".into());
        }
        let latest = self.admissions.as_ref().and_then(|manager| {
            manager
                .receipts
                .iter()
                .rev()
                .find(|receipt| receipt.credential.part_id == credential.part_id)
        });
        if latest.is_none_or(|receipt| receipt.credential != *credential) {
            return Err("member leave credential is no longer current".into());
        }
        let mut next = self.session.clone();
        let authority = self.host.advertisement();
        next.observe_host_lost(
            &credential.host_id,
            &credential.boot_id,
            &authority.host_id,
            &authority.boot_id,
        )
        .map_err(|error| format!("{error:?}"))?;
        // Commit biography and archive obligations before retiring in-memory
        // route witnesses or acknowledging withdrawal to the participant.
        state::retain_session(
            root,
            &mut next,
            self.last_execution.as_ref(),
            self.admissions.as_ref(),
            None,
            None,
        )?;
        self.session = next;
        self.current_host_offers.reconcile(self.session.evidence());
        self.retire_native_member_route(credential);
        Ok(self.session.evidence().clone())
    }
}
