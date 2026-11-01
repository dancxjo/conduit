//! Retained same-Body Seed production from an explicitly selected, opened artifact.
//! The artifact is a prior ordinary Make result. This does not build a target,
//! open a path, grant source/tool access, deploy, observe Boot, or admit a member.
use std::io::{Read, Seek, SeekFrom, Write};

use conduit_body::{BodyBiographyEvidence, BodyLifecycleEvent, SpawnInvitationClaim};
use conduit_host_make::{
    CheckedHostConfiguration, HostImage, MakeCatalog, MakePackageSet, SporeOutputKind,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    check_joining_host, seal_prebuilt_body_spore_with_content_digest, BuiltSpore,
    JoiningHostSelection, SelectedPrebuiltContent,
};

/// Owner-reviewed immutable artifact identity, not an authority minted by this
/// module. The caller must obtain permission to open and retain these bytes.
pub struct ReviewedJoiningArtifact<'a> {
    pub image: &'a HostImage,
    pub image_manifest: &'a [u8],
    pub artifact_digest: &'a str,
    pub artifact_bytes: u64,
    pub source_identity: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoiningProductionBounds {
    pub maximum_artifact_bytes: u64,
    pub maximum_manifest_bytes: usize,
    pub maximum_receipt_bytes: usize,
}

/// Explicit operation input. Public invitation evidence contains no joining
/// secret; issuance and private provisioning remain with the Body owner.
pub struct JoiningProductionRequest<'a> {
    pub request_id: &'a str,
    pub body: &'a BodyBiographyEvidence,
    pub invitation: &'a SpawnInvitationClaim,
    pub configuration: &'a CheckedHostConfiguration,
    pub output: SporeOutputKind,
    pub artifact: ReviewedJoiningArtifact<'a>,
    pub bounds: JoiningProductionBounds,
}

/// Retains the actual opened artifact and exact canonical Seed/receipt bytes.
/// A prepared result proves neither deployment nor Boot nor membership.
pub struct ProducedJoiningHost<R> {
    pub artifact: R,
    pub spore: BuiltSpore,
    pub spore_manifest: Vec<u8>,
    pub receipt: Vec<u8>,
}

#[derive(Serialize)]
struct Receipt<'a> {
    schema: &'static str,
    request_id: &'a str,
    body_id: &'a str,
    invitation_id: &'a str,
    configuration_id: &'a str,
    image_id: &'a str,
    image_source_identity: &'a str,
    spore_id: &'a str,
    spore_manifest_digest: String,
    artifact_digest: &'a str,
    artifact_bytes: u64,
    disposition: &'static str,
}

/// Stream-verifies the opened artifact against its reviewed identity and binds
/// it to the original Body. `now_millis` must use the invitation's clock basis;
/// it is sampled again after reading, before sealing. Cancellation is checked
/// before every bounded read and before retaining a successful result. The
/// caller owns synchronous execution scheduling and cancellation/drain custody.
pub fn produce_joining_host<R: Read + Seek>(
    request: JoiningProductionRequest<'_>,
    mut artifact: R,
    catalog: &MakeCatalog,
    packages: &MakePackageSet,
    mut now_millis: impl FnMut() -> u64,
    mut cancelled: impl FnMut() -> bool,
) -> Result<ProducedJoiningHost<R>, String> {
    let bounds = request.bounds;
    if request.request_id.trim().is_empty()
        || request.request_id.len() > 256
        || bounds.maximum_artifact_bytes == 0
        || !(1..=1024 * 1024).contains(&bounds.maximum_manifest_bytes)
        || !(1..=16 * 1024 * 1024).contains(&bounds.maximum_receipt_bytes)
        || request.artifact.artifact_bytes == 0
        || request.artifact.artifact_bytes > bounds.maximum_artifact_bytes
        || request.artifact.image_manifest.len() > bounds.maximum_manifest_bytes
        || request.artifact.source_identity.is_empty()
        || request.artifact.image.manifest.source_identity != request.artifact.source_identity
    {
        return Err(
            "joining Host production input exceeds its admitted bounds or source identity".into(),
        );
    }
    // Refuse invalid identity/catalogue/invitation before reading any artifact.
    let selection = || JoiningHostSelection {
        configuration: request.configuration,
        output: request.output.clone(),
        deployment: None,
    };
    check_joining_host(
        request.body,
        request.invitation,
        selection(),
        now_millis(),
        catalog,
        packages,
    )?;
    if cancelled() {
        return Err("joining Host production cancelled".into());
    }
    artifact
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("joining artifact rewind: {e}"))?;
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        if cancelled() {
            return Err("joining Host production cancelled".into());
        }
        // At most the reviewed length plus one probe byte is ever consumed.
        let remaining = request.artifact.artifact_bytes - bytes;
        let capacity = remaining.min(buffer.len() as u64) as usize;
        let capacity = if capacity == 0 { 1 } else { capacity };
        let count = artifact
            .read(&mut buffer[..capacity])
            .map_err(|e| format!("joining artifact read: {e}"))?;
        if count == 0 {
            break;
        }
        if count as u64 > remaining {
            return Err("joining artifact exceeds reviewed length".into());
        }
        bytes += count as u64;
        hash.update(&buffer[..count]);
    }
    let digest = format!("sha256:{:x}", hash.finalize());
    if bytes != request.artifact.artifact_bytes || digest != request.artifact.artifact_digest {
        return Err("joining artifact differs from reviewed bytes".into());
    }
    artifact
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("joining artifact retention rewind: {e}"))?;
    if cancelled() {
        return Err("joining Host production cancelled".into());
    }
    let construction = check_joining_host(
        request.body,
        request.invitation,
        selection(),
        now_millis(),
        catalog,
        packages,
    )?;
    let birth_sign = match request.body.body.events.first() {
        Some(BodyLifecycleEvent::Born { sign_id, .. }) => sign_id.as_str(),
        _ => return Err("joining Host production lacks original Birth Sign".into()),
    };
    let spore = seal_prebuilt_body_spore_with_content_digest(
        &construction,
        &request.configuration.configuration().name,
        birth_sign,
        request.artifact.image,
        SelectedPrebuiltContent {
            image_manifest_bytes: request.artifact.image_manifest,
            image_content_digest: &digest,
        },
        catalog,
        packages,
    )
    .map_err(|e| format!("joining Host Seed sealing: {e:?}"))?;
    let spore_manifest = bounded_json(&spore.manifest, bounds.maximum_manifest_bytes)?;
    let receipt = bounded_json(
        &Receipt {
            schema: "conduit.make/joining-host-production@1",
            request_id: request.request_id,
            body_id: request.body.body_id.as_str(),
            invitation_id: request.invitation.invitation_id.as_str(),
            configuration_id: request.configuration.configuration_id(),
            image_id: &spore.image.manifest.image_id,
            image_source_identity: request.artifact.source_identity,
            spore_id: &spore.manifest.spore_id,
            spore_manifest_digest: format!("sha256:{:x}", Sha256::digest(&spore_manifest)),
            artifact_digest: &digest,
            artifact_bytes: bytes,
            disposition: "prepared",
        },
        bounds.maximum_receipt_bytes,
    )?;
    if cancelled() {
        return Err("joining Host production cancelled".into());
    }
    Ok(ProducedJoiningHost {
        artifact,
        spore,
        spore_manifest,
        receipt,
    })
}

fn bounded_json(value: &impl Serialize, maximum: usize) -> Result<Vec<u8>, String> {
    struct Bounded {
        bytes: Vec<u8>,
        maximum: usize,
    }
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other(
                    "joining Host evidence bound exhausted",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Bounded {
        bytes: Vec::new(),
        maximum,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|e| format!("joining Host bounded evidence: {e}"))?;
    Ok(writer.bytes)
}

#[cfg(test)]
mod tests;
