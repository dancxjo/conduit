//! Root owns its installed Body; reports follow retained ownership.
use super::NativeRoot;
impl NativeRoot {
    #[cfg(feature = "body-image-root")]
    pub fn install_linked_body(&mut self) -> Result<(), &'static str> {
        if self.body.is_some() {
            return Err("body-image-already-installed");
        }
        let owner = crate::native_body_image::load(self)?;
        self.body = Some(owner);
        #[cfg(feature = "body-persistence")]
        self.admission.activate_after_checkpoint()?;
        Ok(())
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn restart_verifying_key(&self) -> [u8; 32] {
        self.admission.verifying_key()
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub fn checkpoint_body(&mut self) -> Result<(), &'static str> {
        let key = self.admission.verifying_key();
        let body = self.body.as_mut().ok_or("restart-body-not-installed")?;
        crate::native_restart::checkpoint(body, &key)?;
        self.admission.activate_after_checkpoint()
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn body_join_scope(
        &self,
    ) -> Result<
        (
            &conduit_core::HostId,
            &conduit_core::BootId,
            &conduit_body::BodyId,
            [u8; 32],
        ),
        &'static str,
    > {
        let (host, boot, _) = self.offer_scope();
        let body = self.body.as_ref().ok_or("body-join-not-installed")?;
        Ok((
            host,
            boot,
            &body.evidence().body_id,
            self.admission.verifying_key(),
        ))
    }
    /// Private target provisioning only: the returned secret authorizes one
    /// observer admission. Root must explicitly install its serving route.
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn issue_body_invitation(
        &mut self,
        mut secret: [u8; 32],
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
    ) -> Result<crate::native_body_join::PrivateBodyInvitation, &'static str> {
        let prepared = conduit_body::SpawnInvitationSecret::from_csprng_bytes(secret);
        secret.fill(0);
        let secret = prepared.map_err(|_| "body-join-entropy")?;
        let (host, boot, _) = self.offer_scope();
        let (host, boot) = (host.clone(), boot.clone());
        let key = self.admission.verifying_key();
        let body = self.body.as_mut().ok_or("body-join-not-installed")?;
        crate::native_body_join::issue(
            body,
            &key,
            &host,
            &boot,
            secret,
            nonce,
            now_millis,
            expires_at_millis,
        )
    }

    /// Strict bounded observer carrier; never decode rich capability metadata
    /// merely to reject it. A signed receipt follows canonical durable admission.
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn admit_body_observer(
        &mut self,
        request: &[u8],
        now_millis: u64,
    ) -> Result<crate::native_joined_host_wire::JoinedHostReceipt, &'static str> {
        let request = crate::native_body_join::wire::decode(request)?;
        self.admit_body_observer_request(&request, now_millis)
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn admit_body_observer_request(
        &mut self,
        request: &conduit_body::PortableSpawnAdmissionRequest,
        now_millis: u64,
    ) -> Result<crate::native_joined_host_wire::JoinedHostReceipt, &'static str> {
        let (host, boot, _) = self.offer_scope();
        let (host, boot) = (host.clone(), boot.clone());
        let key = self.admission.verifying_key();
        let body = self.body.as_mut().ok_or("body-join-not-installed")?;
        let admitted =
            crate::native_body_join::admit(body, &key, &host, &boot, request, now_millis)?;
        let signature = self.admission.sign_joined_host(&admitted)?;
        Ok(admitted.signed(signature))
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn recover_body_observer(
        &self,
        request: &crate::native_body_join::recovery::Request,
    ) -> Result<crate::native_body_join::recovery::Receipt, &'static str> {
        let (host, boot, _, key) = self.body_join_scope()?;
        let body = self.body.as_ref().ok_or("body-join-not-installed")?;
        let verified = crate::native_restart::with_admissions(|manager| {
            crate::native_body_join::recovery::verify_current(
                manager,
                &body.evidence().membership,
                request,
                host,
                boot,
                &key,
            )
        })?;
        let signature = self.admission.sign_current_observer(&verified)?;
        Ok(verified.signed(signature))
    }
    pub fn body_installed(&self) -> bool {
        #[cfg(feature = "body-image-root")]
        {
            self.body.is_some()
        }
        #[cfg(not(feature = "body-image-root"))]
        {
            false
        }
    }
}
