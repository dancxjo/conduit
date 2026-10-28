//! Existing Body ambient admission proof, without membership or effect authority.
use conduit_body::{ambient_admission_transcript, validate_pico_challenge, PicoAdmissionChallenge};
use conduit_core::OfferGeneration;
use ed25519_dalek::{Signer, SigningKey};

// This UART carrier admits a larger offer than the Pico USB carrier; the
// borrowed challenge/proof documents and Body verifier remain unchanged.
pub const MAXIMUM_ADVERTISEMENT_BYTES: usize = 8192;
// Independently bounded wrapper fields: protocol, friendly label, verifying key
// and freshness counter. The bare advertisement keeps its existing 8 KiB cap.
pub const MAXIMUM_ENVELOPE_BYTES: usize = MAXIMUM_ADVERTISEMENT_BYTES + 320;
pub struct NativeAdmission {
    key: SigningKey,
    activated: bool,
}
impl NativeAdmission {
    /// Caller supplies independently initialized hardware entropy. No fallback.
    pub fn new(mut seed: [u8; 32]) -> Result<Self, ()> {
        if seed == [0; 32] {
            return Err(());
        }
        #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
        {
            let retained = crate::native_restart::select_signer_seed(seed);
            seed.fill(0);
            seed = retained.map_err(|_| ())?;
        }
        let key = SigningKey::from_bytes(&seed);
        #[cfg(all(
            any(feature = "local-evidence-store", feature = "observation-store"),
            not(all(feature = "body-image-root", feature = "body-persistence"))
        ))]
        crate::native_observed_http::install(key.clone())?;
        seed.fill(0);
        Ok(Self {
            key,
            activated: !cfg!(all(
                feature = "body-image-root",
                feature = "body-persistence"
            )),
        })
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn activate_after_checkpoint(&mut self) -> Result<(), &'static str> {
        if !crate::native_restart::publication_ready() {
            return Err("restart-publication-not-durable");
        }
        if !self.activated {
            #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
            crate::native_observed_http::install(self.key.clone())
                .map_err(|_| "restart-history-signer")?;
            self.activated = true;
        }
        Ok(())
    }
    fn ready(&self) -> bool {
        #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
        {
            self.activated && crate::native_restart::publication_ready()
        }
        #[cfg(not(all(feature = "body-image-root", feature = "body-persistence")))]
        {
            self.activated
        }
    }
    pub fn verifying_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn sign_joined_host(
        &self,
        admitted: &crate::native_body_join::DurablyAdmittedHost,
    ) -> Result<[u8; 64], &'static str> {
        if !self.ready() {
            return Err("body-join-not-durable");
        }
        Ok(self.key.sign(&admitted.transcript()).to_bytes())
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn sign_current_observer(
        &self,
        observer: &crate::native_body_join::recovery::VerifiedCurrentObserver,
    ) -> Result<[u8; 64], &'static str> {
        if !self.ready() {
            return Err("body-recovery-not-durable");
        }
        Ok(self.key.sign(&observer.transcript()).to_bytes())
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn sign_return_challenge(
        &self,
        draft: &crate::native_body_join::returning::DurableChallenge,
    ) -> Result<[u8; 64], &'static str> {
        if !self.ready() {
            return Err("body-return-not-durable");
        }
        Ok(self.key.sign(&draft.transcript()).to_bytes())
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn sign_returned_observer(
        &self,
        draft: &crate::native_body_join::returning::DurableReturn,
    ) -> Result<[u8; 64], &'static str> {
        if !self.ready() {
            return Err("body-return-not-durable");
        }
        Ok(self.key.sign(&draft.transcript()).to_bytes())
    }
    /// Cold biography custody endorsement is separate from current-Boot
    /// membership and execution. Only the typed verified ACK draft is signable.
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn sign_biography_custody(
        &self,
        draft: &crate::native_body_join::biography_acceptance::AcceptedDraft,
    ) -> Result<[u8; 64], &'static str> {
        if draft.root() != &self.verifying_key() {
            return Err("biography-endorsement-root");
        }
        Ok(self.key.sign(&draft.transcript()).to_bytes())
    }
    #[cfg(all(
        feature = "local-appliance",
        feature = "body-persistence",
        target_os = "none"
    ))]
    pub(crate) fn sign_network_snapshot(
        &self,
        draft: &crate::native_play::journal::snapshot::Draft,
        owner: &conduit_body::SealedBodyLifecycleSession,
        boot: &conduit_core::BootId,
    ) -> Result<[u8; 240], &'static str> {
        if !self.ready() || draft.root() != &self.verifying_key() {
            return Err("network-snapshot-live-signer");
        }
        crate::native_restart::observed::check_live_owner(owner, &self.verifying_key())?;
        let (epoch, predecessor) = crate::native_root::network_snapshot::current_chain(
            &owner.evidence().body_id,
            &self.verifying_key(),
        )?;
        if !draft.matches_scope(crate::native_play::journal::snapshot::Scope {
            body: &owner.evidence().body_id,
            boot,
            epoch,
            predecessor,
        }) {
            return Err("network-snapshot-stale-scope");
        }
        Ok(draft.signed(self.key.sign(&draft.transcript()).to_bytes()))
    }
    /// Only an actual frozen-journal draft and durably current local Body may
    /// reach this signer. The caller retains the actual Root-owned session.
    #[cfg(all(
        feature = "body-image-root",
        feature = "body-persistence",
        any(feature = "local-evidence-store", feature = "observation-store")
    ))]
    pub(crate) fn sign_closed_observed_offer(
        &self,
        draft: crate::native_observed_archive::custody_acceptance::ClosedOfferDraft,
        owner: &conduit_body::SealedBodyLifecycleSession,
    ) -> Result<
        crate::native_observed_archive::custody_acceptance::wire::Offer<'static>,
        &'static str,
    > {
        if !self.ready() || draft.root() != &self.verifying_key() {
            return Err("observed-live-signer");
        }
        crate::native_restart::observed::check_live_owner(owner, &self.verifying_key())?;
        crate::native_restart::with_admissions(|manager| {
            if manager.body_id != owner.evidence().body_id {
                return Err("observed-live-manager");
            }
            Ok(())
        })?;
        let signature = self.key.sign(&draft.transcript()).to_bytes();
        let offer = draft.signed(signature);
        use sha2::Digest;
        if offer.scope.0[..32]
            != sha2::Sha256::digest(owner.evidence().body_id.as_str().as_bytes())[..]
        {
            return Err("observed-offer-body");
        }
        Ok(offer)
    }
    #[cfg(all(
        feature = "body-image-root",
        feature = "body-persistence",
        any(feature = "local-evidence-store", feature = "observation-store")
    ))]
    pub(crate) fn acknowledge_closed_observed(
        &self,
        draft: &crate::native_observed_archive::custody_acceptance::ClosedOfferDraft,
        request: &crate::native_observed_archive::custody_acceptance::wire::Request<'_>,
        owner: &conduit_body::SealedBodyLifecycleSession,
    ) -> Result<[u8; 416], &'static str> {
        if !self.ready() || draft.root() != &self.verifying_key() {
            return Err("observed-live-signer");
        }
        crate::native_restart::observed::check_live_owner(owner, &self.verifying_key())?;
        let accepted = crate::native_restart::with_admissions(|manager| {
            crate::native_observed_archive::custody_acceptance::verify_ack(
                draft,
                manager,
                &owner.evidence().membership,
                request,
            )
        })?;
        if accepted.root() != &self.verifying_key() {
            return Err("observed-accepted-root");
        }
        let signature = self.key.sign(&accepted.transcript()).to_bytes();
        Ok(accepted.signed(signature))
    }
    /// Root supplies this only after exact reservation/lifecycle admission.
    pub fn sign_owner_receipt(&self, receipt: &crate::native_owner_receipt::Receipt) -> [u8; 64] {
        assert!(
            self.ready(),
            "Root signing requires durable current Body custody"
        );
        self.key.sign(&receipt.transcript()).to_bytes()
    }
    #[cfg(feature = "display-physical-check")]
    pub fn sign_display_evidence(&self, offer: &crate::native_display_evidence::Offer) -> [u8; 64] {
        assert!(
            self.ready(),
            "Root signing requires durable current Body custody"
        );
        self.key.sign(&offer.transcript()).to_bytes()
    }
    /// Expiry is checked by the Body authority in its own clock domain. This
    /// responder validates the exact current scope and signs no install/start.
    pub fn sign(
        &self,
        challenge: &PicoAdmissionChallenge<'_>,
        host: &str,
        boot: &str,
        generation: u64,
    ) -> Result<[u8; 64], ()> {
        if !self.ready() || !validate_pico_challenge(challenge, host, boot, generation) {
            return Err(());
        }
        Ok(self
            .key
            .sign(&ambient_admission_transcript(
                "ambient-admission-v1",
                challenge.body_id,
                challenge.admission_id,
                host,
                boot,
                OfferGeneration(generation),
                &challenge.nonce,
                challenge.expires_at_millis,
            ))
            .to_bytes())
    }
}
