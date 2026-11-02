//! Authenticated local callers may carry an already prepared private Seed.
//! Readiness, manufacture, transfer and admission remain separate evidence.
use super::Owner;
use conduit_body_make::JoiningHostPackageReceipt;
use conduit_host_make::{
    BodyBoundArtifactIdentity, DeploymentCarrierDescriptor, DeploymentCarrierKind,
    DeploymentRealizationReceipt, ARTIFACT_DOWNLOAD_IMPLEMENTATION, DEPLOYMENT_CARRIER_SCHEMA,
};
use std::path::Path;

impl Owner {
    pub(crate) fn export_prepared_seed(
        &mut self,
        destination: &Path,
    ) -> Result<DeploymentRealizationReceipt, String> {
        if self.host.is_playing() {
            return Err("lull the Body before carrying a prepared Seed".into());
        }
        if !destination.is_absolute() {
            return Err("Seed export requires an explicit absolute destination".into());
        }
        let products = self
            .creche_products
            .as_mut()
            .ok_or("no prepared Seed is retained")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Seed invitation clock unavailable")?
            .as_millis()
            .try_into()
            .map_err(|_| "Seed invitation clock overflow")?;
        products
            .invitation
            .inspect(now)
            .map_err(|e| format!("Seed invitation is no longer usable: {e:?}"))?;
        let receipt: JoiningHostPackageReceipt = serde_json::from_slice(&products.receipt)
            .map_err(|_| "retained result is not a verified Seed package")?;
        let manifest = &products.spore.manifest;
        if manifest.body_id != self.session.evidence().body_id.as_str()
            || products.invitation.body_id.as_str() != manifest.body_id
            || !matches!(&manifest.binding, conduit_body_make::SporeBinding::SelfJoining { invitation_id } if invitation_id == products.invitation.invitation_id.as_str())
            || !matches!(self.session.evidence().body.events.first(), Some(conduit_body::BodyLifecycleEvent::Born { sign_id, .. }) if sign_id.as_str() == manifest.source_identity)
            || receipt.image_content_sha256 != manifest.image_content_digest
            || manifest.image_id != products.spore.image.manifest.image_id
            || receipt.artifact_bytes == 0
            || receipt.artifact_bytes > 64 * 1024 * 1024
        {
            return Err("retained Seed differs from the current Body or verified artifact".into());
        }
        let identity = BodyBoundArtifactIdentity {
            target_id: manifest.target.clone(),
            image_id: manifest.image_id.clone(),
            image_content_sha256: receipt.image_content_sha256,
            spore_id: manifest.spore_id.clone(),
            artifact_content_sha256: receipt.artifact_content_sha256,
            artifact_bytes: receipt.artifact_bytes,
        };
        let carrier = DeploymentCarrierDescriptor {
            schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
            carrier_id: "installed/seed-download".into(),
            target_id: manifest.target.clone(),
            kind: DeploymentCarrierKind::ArtifactDownload,
            implementation_id: ARTIFACT_DOWNLOAD_IMPLEMENTATION.into(),
            maximum_artifact_bytes: identity.artifact_bytes,
            requires_explicit_authority: false,
            verifies_written_bytes: true,
        };
        let started = std::time::Instant::now();
        conduit_host_make::download_body_bound_artifact_from(
            &carrier,
            &identity,
            &mut products.artifact,
            destination,
            || started.elapsed() >= std::time::Duration::from_secs(2),
        )
        .map_err(|e| format!("Seed export refused: {e:?}"))
    }
}
