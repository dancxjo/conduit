//! Installed facts plus one real current-Boot Root attempt, no online receipt.
use super::*;
pub struct ContinuationParts<'a, 's> {
    pub bootstrap: &'a b::MemberBootstrap<'s>,
    pub resources: &'a mut ResourceAdmissionOwner,
    pub configuration: &'a Configuration,
    /// Original empty assignment backing; bytes confer no command attribution.
    /// Keep output immutable while FreshContinuationScope/Play borrows it.
    pub workspace: &'a mut [u8],
}
impl InstalledContinuationAcceptance {
    /// Borrow descriptive bytes from the SAME independently read accepted media.
    /// The opaque Root verification is retained; this does not grant execution.
    pub fn artifact<'a>(
        &self,
        prefix: &'a [u8; PREFIX_BYTES],
        scratch: &mut [u8; PREFIX_BYTES],
    ) -> Result<artifact::Artifact<'a>, E> {
        if Sha256::digest(prefix).as_slice() != self.prefix_digest {
            return Err(E::Signature);
        }
        artifact::decode_installed(prefix, scratch).map_err(E::Artifact)
    }
}
impl<'s, const N: usize> Root<'s, N> {
    /// Verify permission using original Root-verified accepted generations and
    /// credential, actual fresh inventory, and checked installed Source pins.
    /// `actual` is caller-owned preparation backing; equality with the original
    /// Root inventory is checked before borrowing it into the fresh proof.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_installed_continuation<'a>(
        &self,
        accepted: &'a InstalledContinuationAcceptance,
        prefix: &'a [u8; PREFIX_BYTES],
        scratch: &mut [u8; PREFIX_BYTES],
        actual: &'a HostAdvertisement,
        source: &[u8],
        pins: &[b::ContinuationBundleEntry<'_>],
    ) -> Result<b::VerifiedContinuationIntent<'a>, E> {
        self.verify_installed_continuation_digest(
            accepted,
            prefix,
            scratch,
            actual,
            Sha256::digest(source).into(),
            pins,
        )
    }
    /// Verify against the independently compiled Make Source digest without
    /// retaining the authored Source text on the MCU.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_installed_continuation_digest<'a>(
        &self,
        accepted: &'a InstalledContinuationAcceptance,
        prefix: &'a [u8; PREFIX_BYTES],
        scratch: &mut [u8; PREFIX_BYTES],
        actual: &'a HostAdvertisement,
        source_digest: [u8; 32],
        pins: &[b::ContinuationBundleEntry<'_>],
    ) -> Result<b::VerifiedContinuationIntent<'a>, E> {
        if actual != self.resources.host() {
            return Err(E::Scope);
        }
        self.verify_installed_continuation_boot_digest(
            accepted,
            prefix,
            scratch,
            actual.boot_id.as_str(),
            source_digest,
            pins,
        )
    }
    /// Borrow only separately funded Boot text, checked against THIS original
    /// Root. All other installed facts come from Root's verified acceptance and
    /// configuration. No caller-provided inventory or identity is trusted.
    ///
    /// The result does not borrow Root, so subsequent full admission uses its
    /// original mutable ledger without retaining a duplicate Host advertisement.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_installed_continuation_boot_digest<'a>(
        &self,
        accepted: &'a InstalledContinuationAcceptance,
        prefix: &'a [u8; PREFIX_BYTES],
        scratch: &mut [u8; PREFIX_BYTES],
        actual_boot: &'a str,
        source_digest: [u8; 32],
        pins: &[b::ContinuationBundleEntry<'_>],
    ) -> Result<b::VerifiedContinuationIntent<'a>, E> {
        self.check_anchor(&accepted.anchor)?;
        if actual_boot != self.resources.host().boot_id.as_str() {
            return Err(E::Scope);
        }
        let candidate = accepted.artifact(prefix, scratch)?;
        self.check_artifact_credential(&candidate, &accepted.anchor.credential)?;
        let workset_digest =
            b::continuation_bundle_digest(source_digest, pins).map_err(|_| E::Scope)?;
        let maximum = self
            .config
            .execution
            .and_then(|p| p.continuation)
            .ok_or(E::Scope)?;
        let verified = b::verify_continuation_intent(
            candidate.intent,
            &candidate.signature_bytes(),
            Some(b::ContinuationPolicy {
                issuer_host: self.seed.owner.host_id.as_str(),
                issuer_key: self.seed.owner.verifying_key,
                maximum,
            }),
            b::TrustedInstalledContinuationFacts {
                body_id: accepted.anchor.credential.body_id.as_str(),
                part_id: accepted.anchor.credential.part_id.as_str(),
                host_id: accepted.anchor.credential.host_id.as_str(),
                credential_id: accepted.anchor.credential.credential_id.as_str(),
                membership_generation: accepted.anchor.membership_generation,
                intent_generation: accepted.anchor.intent_generation,
                source_digest,
                workset_digest,
                image_id: self.config.image_id,
                configuration_id: self.config.configuration_id,
                actual_boot,
            },
        )
        .map_err(|e| match e {
            b::ContinuationIntentRefusal::Generation => E::Generation,
            b::ContinuationIntentRefusal::Signature => E::Signature,
            b::ContinuationIntentRefusal::Bounds => E::Bounds,
            _ => E::Scope,
        })?;
        b::check_installed_continuation_bundle(&verified, source_digest, pins)
            .map_err(|_| E::Scope)?;
        Ok(verified)
    }
    /// Consume exactly one original Root continuation attempt in THIS Boot. A
    /// failed preparation never makes another epoch or silently becomes Playing.
    /// These parts convey no approval: existing FreshScope/native grants/full
    /// admission/one-use Start are all still required before provider withdrawal.
    pub fn take_continuation_parts<'a>(
        &'a mut self,
        accepted: &InstalledContinuationAcceptance,
    ) -> Result<ContinuationParts<'a, 's>, E> {
        self.check_anchor(&accepted.anchor)?;
        if self.continuation_attempted
            || self.receipt.is_some()
            || self.input.is_retained()
            || self.config.execution.and_then(|p| p.continuation).is_none()
            || self.seed.admission.credential() != Some(&accepted.anchor.credential)
            || !self.resources.admissions().is_empty()
            || !self.resources.assignments().is_empty()
        {
            return Err(E::Scope);
        }
        let workspace = self.input.writable().map_err(|_| E::Scope)?;
        self.continuation_attempted = true;
        Ok(ContinuationParts {
            bootstrap: &self.seed,
            resources: &mut self.resources,
            configuration: &self.config,
            workspace,
        })
    }
}
