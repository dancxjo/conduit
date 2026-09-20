//! Installed artifact-only carrier for an already-built body-bound artifact.

use crate::{
    BodyBoundArtifactIdentity, CarrierTerminal, DeploymentCarrierDescriptor, DeploymentCarrierKind,
    DeploymentCarrierRefusal, DeploymentCarrierRequest, DeploymentRealizationReceipt,
    DEPLOYMENT_RECEIPT_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const COPY_BUFFER_BYTES: usize = 64 * 1024;
pub const ARTIFACT_DOWNLOAD_IMPLEMENTATION: &str = "conduit/download@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactDownloadRefusal {
    Carrier(DeploymentCarrierRefusal),
    UnsupportedCarrier,
    SourceUnavailable,
    DestinationUnavailable,
    DestinationExists,
    ExtentMismatch,
    ContentMismatch,
}

impl From<DeploymentCarrierRefusal> for ArtifactDownloadRefusal {
    fn from(value: DeploymentCarrierRefusal) -> Self {
        Self::Carrier(value)
    }
}

/// Copy one exact body-bound artifact without performing a consequential
/// realization. The destination is created atomically and never overwritten.
pub fn download_body_bound_artifact(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    source: &Path,
    destination: &Path,
) -> Result<DeploymentRealizationReceipt, ArtifactDownloadRefusal> {
    if descriptor.kind != DeploymentCarrierKind::ArtifactDownload
        || descriptor.implementation_id != ARTIFACT_DOWNLOAD_IMPLEMENTATION
    {
        return Err(ArtifactDownloadRefusal::UnsupportedCarrier);
    }
    descriptor.validate_request(&DeploymentCarrierRequest {
        carrier_id: &descriptor.carrier_id,
        target_id: &descriptor.target_id,
        artifact,
        explicit_authority: false,
        destructive_destination: None,
        confirmed_destination: None,
        destination_is_removable: None,
    })?;
    if destination.exists() {
        return Err(ArtifactDownloadRefusal::DestinationExists);
    }
    let source_metadata =
        fs::metadata(source).map_err(|_| ArtifactDownloadRefusal::SourceUnavailable)?;
    if !source_metadata.is_file() {
        return Err(ArtifactDownloadRefusal::SourceUnavailable);
    }
    if source_metadata.len() != artifact.artifact_bytes {
        return Err(ArtifactDownloadRefusal::ExtentMismatch);
    }
    let temporary = temporary_path(destination)?;
    let result = copy_exact(source, &temporary, artifact);
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    fs::rename(&temporary, destination).map_err(|_| {
        let _ = fs::remove_file(&temporary);
        ArtifactDownloadRefusal::DestinationUnavailable
    })?;
    let receipt = DeploymentRealizationReceipt {
        schema: DEPLOYMENT_RECEIPT_SCHEMA.into(),
        carrier_id: descriptor.carrier_id.clone(),
        implementation_id: descriptor.implementation_id.clone(),
        target_id: descriptor.target_id.clone(),
        spore_id: artifact.spore_id.clone(),
        artifact_content_sha256: artifact.artifact_content_sha256.clone(),
        terminal: CarrierTerminal::Completed,
        bytes_realized: artifact.artifact_bytes,
        byte_verification_completed: true,
        explicit_authority_observed: false,
        boot_observed: false,
        join_observed: false,
        membership_admitted: false,
    };
    descriptor.validate_receipt(artifact, &receipt)?;
    Ok(receipt)
}

fn temporary_path(destination: &Path) -> Result<PathBuf, ArtifactDownloadRefusal> {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ArtifactDownloadRefusal::DestinationUnavailable)?;
    Ok(destination.with_file_name(format!(".{file_name}.conduit-partial")))
}

fn copy_exact(
    source: &Path,
    temporary: &Path,
    artifact: &BodyBoundArtifactIdentity,
) -> Result<(), ArtifactDownloadRefusal> {
    let mut input = File::open(source).map_err(|_| ArtifactDownloadRefusal::SourceUnavailable)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .map_err(|_| ArtifactDownloadRefusal::DestinationUnavailable)?;
    let mut digest = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|_| ArtifactDownloadRefusal::SourceUnavailable)?;
        if count == 0 {
            break;
        }
        copied = copied
            .checked_add(count as u64)
            .ok_or(ArtifactDownloadRefusal::ExtentMismatch)?;
        if copied > artifact.artifact_bytes {
            return Err(ArtifactDownloadRefusal::ExtentMismatch);
        }
        digest.update(&buffer[..count]);
        output
            .write_all(&buffer[..count])
            .map_err(|_| ArtifactDownloadRefusal::DestinationUnavailable)?;
    }
    output
        .sync_all()
        .map_err(|_| ArtifactDownloadRefusal::DestinationUnavailable)?;
    if copied != artifact.artifact_bytes {
        return Err(ArtifactDownloadRefusal::ExtentMismatch);
    }
    let actual = format!("sha256:{:x}", digest.finalize());
    if actual != artifact.artifact_content_sha256 {
        return Err(ArtifactDownloadRefusal::ContentMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEPLOYMENT_CARRIER_SCHEMA;

    fn fixture(bytes: &[u8]) -> (DeploymentCarrierDescriptor, BodyBoundArtifactIdentity) {
        let target_id = "conduitos/x86_64/pc".to_owned();
        (
            DeploymentCarrierDescriptor {
                schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
                carrier_id: "conduit-carrier/download@1".into(),
                target_id: target_id.clone(),
                kind: DeploymentCarrierKind::ArtifactDownload,
                implementation_id: "conduit/download@1".into(),
                maximum_artifact_bytes: 1024,
                requires_explicit_authority: false,
                verifies_written_bytes: true,
            },
            BodyBoundArtifactIdentity {
                target_id,
                image_id: "image/reviewed".into(),
                image_content_sha256: format!("sha256:{:x}", Sha256::digest(b"generic")),
                spore_id: "spore/body-one/host-two".into(),
                artifact_content_sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
                artifact_bytes: bytes.len() as u64,
            },
        )
    }

    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("conduit-download-{name}-{}", std::process::id()))
    }

    #[test]
    fn exact_artifact_is_downloaded_without_claiming_boot_or_join() {
        let bytes = b"exact body-bound spore";
        let (descriptor, artifact) = fixture(bytes);
        let root = root("exact");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.img");
        let destination = root.join("download.img");
        fs::write(&source, bytes).unwrap();

        let receipt =
            download_body_bound_artifact(&descriptor, &artifact, &source, &destination).unwrap();
        assert_eq!(fs::read(destination).unwrap(), bytes);
        assert_eq!(receipt.terminal, CarrierTerminal::Completed);
        assert!(receipt.byte_verification_completed);
        assert!(!receipt.boot_observed);
        assert!(!receipt.join_observed);
        assert!(!receipt.membership_admitted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mismatched_content_and_existing_destination_refuse_without_overwrite() {
        let bytes = b"expected spore";
        let (descriptor, artifact) = fixture(bytes);
        let root = root("refusal");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.img");
        let destination = root.join("download.img");
        fs::write(&source, b"Expected spore").unwrap();
        assert_eq!(
            download_body_bound_artifact(&descriptor, &artifact, &source, &destination),
            Err(ArtifactDownloadRefusal::ContentMismatch)
        );
        assert!(!destination.exists());
        fs::write(&source, bytes).unwrap();
        fs::write(&destination, b"operator data").unwrap();
        assert_eq!(
            download_body_bound_artifact(&descriptor, &artifact, &source, &destination),
            Err(ArtifactDownloadRefusal::DestinationExists)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"operator data");
        fs::remove_dir_all(root).unwrap();
    }
}
