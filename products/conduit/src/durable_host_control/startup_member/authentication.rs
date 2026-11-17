//! Original owner authentication; neither membership nor TLS is a startup grant.
use super::*;
use sha2::Digest;
impl DurableHostRuntime {
    pub(super) fn begin_startup_member_resume(
        &mut self,
        session: Box<Session>,
        mut challenge: Box<StartupOwnerChallenge>,
    ) -> Result<Reply, String> {
        let now = crate::durable_host::current_time_millis()?;
        let expired = self.startup_member.as_ref().is_some_and(|run| {
            run.prepared.is_none()
                && run.scope.is_none()
                && match &run.authentication {
                    Authentication::Failed => true,
                    Authentication::Pending { challenge, .. }
                    | Authentication::Authenticated(challenge) => {
                        challenge.federation.expires_at_millis <= now
                    }
                }
        });
        if expired {
            self.startup_member.take();
        }
        if self.startup_member.is_some()
            || self.remote_fragment.is_some()
            || self.pool_member.is_some()
        {
            return Err("member execution custody is already retained".into());
        }
        self.clear_cancellation_signal()?;
        let root = self
            .birth_root
            .as_deref()
            .ok_or("member service has no installed state")?;
        let retained = crate::durable_host::current_joined_credential(root)?;
        let pinned = crate::durable_host::current_joined_owner_pin(root)?;
        let actual = self.host.advertisement();
        let actual_offer_digest: [u8; 32] = sha2::Sha256::digest(
            serde_json::to_vec(actual).map_err(|_| "startup offer encoding failed")?,
        )
        .into();
        if challenge.member_offer_digest != actual_offer_digest {
            return Err("startup challenge differs from current observed offer".into());
        }
        if session.id == [0; 32]
            || session.member != retained
            || retained.host_id != actual.host_id
            || retained.boot_id != actual.boot_id
            || session.owner != pinned
            || challenge.federation.responder != pinned
            || challenge.body_id != retained.body_id
            || challenge.part_id != retained.part_id
            || challenge.credential_id != retained.credential_id
            || challenge.federation.initiator.host_id != actual.host_id
            || challenge.federation.initiator.boot_id != actual.boot_id
            || challenge.federation.initiator.offer_generation != actual.offer_generation
            || !matches!(&self.host, HostSource::Bare(_))
        {
            return Err("startup resume differs from original pinned owner/member".into());
        }
        let now = crate::durable_host::current_time_millis()?;
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce).map_err(|_| "startup challenge randomness unavailable")?;
        if nonce == [0; 32] {
            return Err("startup challenge nonce is invalid".into());
        }
        challenge.federation.nonce = nonce;
        challenge.federation.expires_at_millis = now
            .checked_add(30_000)
            .ok_or("startup challenge expiry overflow")?;
        let challenge = Box::new(challenge.bind_session()?);
        let member_signature = crate::durable_host::sign_startup_member_challenge(
            root, &retained, actual, &challenge, now,
        )?;
        let reply = Reply::Challenge {
            session_id: session.id,
            challenge: challenge.clone(),
            member_signature: Signature(member_signature),
        };
        let remaining_frames = challenge.federation.maximum_frames;
        self.startup_member = Some(MemberRun {
            session: *session,
            scope: None,
            prepared: None,
            control: Default::default(),
            cancellation_request: Some(conduit_std_host::RunControlRequestId::new(
                "member/startup-cancel",
            )?),
            cancellation_play: [0; 64],
            remaining_frames,
            authentication: Authentication::Pending {
                challenge,
                member_signature,
            },
        });
        Ok(reply)
    }
    pub(super) fn complete_startup_member_resume(
        &mut self,
        session_id: [u8; 32],
        challenge: Box<StartupOwnerChallenge>,
        owner_signature: Signature,
    ) -> Result<Reply, String> {
        let current = self
            .startup_member
            .as_mut()
            .ok_or("startup challenge is absent")?;
        if current.session.id != session_id
            || !matches!(&current.authentication, Authentication::Pending { challenge: original, .. }
                    if **original == *challenge)
        {
            return Err("startup response differs from original pending challenge".into());
        }
        let pending = std::mem::replace(&mut current.authentication, Authentication::Failed);
        let Authentication::Pending {
            challenge: original,
            member_signature,
        } = pending
        else {
            return Err("startup challenge was already consumed".into());
        };
        if current.session.id != session_id || *challenge != *original {
            return Err("startup response differs from original challenge".into());
        }
        let root = self
            .birth_root
            .as_deref()
            .ok_or("member service has no installed state")?;
        let pinned = crate::durable_host::current_joined_owner_pin(root)?;
        validate_current(
            &current.session,
            &challenge,
            self.host.advertisement(),
            root,
        )?;
        challenge.validate(&pinned, crate::durable_host::current_time_millis()?)?;
        let signature = owner_signature.0;
        conduit_body::FederatedReceivingSession::establish(
            challenge.federation.clone(),
            member_signature,
            signature,
            crate::durable_host::current_time_millis()?,
        )
        .map_err(|_| "startup original owner authentication failed")?;
        current.authentication = Authentication::Authenticated(challenge);
        Ok(Reply::Resumed {
            session_id,
            member_offer_digest: original.member_offer_digest,
        })
    }
}

pub(super) fn validate_current(
    session: &Session,
    challenge: &StartupOwnerChallenge,
    actual: &conduit_core::HostAdvertisement,
    root: &Path,
) -> Result<(), String> {
    let credential = crate::durable_host::current_joined_credential(root)?;
    let pinned = crate::durable_host::current_joined_owner_pin(root)?;
    let digest: [u8; 32] = sha2::Sha256::digest(
        serde_json::to_vec(actual).map_err(|_| "startup offer encoding failed")?,
    )
    .into();
    if session.member != credential
        || session.owner != pinned
        || challenge.federation.responder != pinned
        || challenge.member_offer_digest != digest
        || challenge.federation.initiator.host_id != actual.host_id
        || challenge.federation.initiator.boot_id != actual.boot_id
        || challenge.federation.initiator.offer_generation != actual.offer_generation
        || credential.host_id != actual.host_id
        || credential.boot_id != actual.boot_id
    {
        return Err("startup authentication no longer matches current member/owner".into());
    }
    challenge.validate(&pinned, crate::durable_host::current_time_millis()?)
}
