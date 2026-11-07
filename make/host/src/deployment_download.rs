//! Installed artifact-only carrier for an already-built body-bound artifact.

use crate::{
    BodyBoundArtifactIdentity, CarrierTerminal, DeploymentCarrierDescriptor, DeploymentCarrierKind,
    DeploymentCarrierRefusal, DeploymentCarrierRequest, DeploymentRealizationReceipt,
    DEPLOYMENT_RECEIPT_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
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
    Cancelled,
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
    validate(descriptor, artifact)?;
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
    let mut source = File::open(source).map_err(|_| ArtifactDownloadRefusal::SourceUnavailable)?;
    download_body_bound_artifact_from(descriptor, artifact, &mut source, destination, || false)
}

/// Export an already-owned artifact handle. No source pathname is reopened and
/// no package or private provisioning is reconstructed. The caller separately
/// owns destination-write authority. Reads, hashing and cancellation are bounded
/// by the exact raw artifact identity, independently of its IMAGE aggregate.
/// Source position returns to zero after copying. Cancellation is cooperative
/// between I/O operations; it does not interrupt a blocked OS write or sync.
pub fn download_body_bound_artifact_from<R: Read + Seek>(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    source: &mut R,
    destination: &Path,
    mut cancelled: impl FnMut() -> bool,
) -> Result<DeploymentRealizationReceipt, ArtifactDownloadRefusal> {
    validate(descriptor, artifact)?;
    if destination.exists() {
        return Err(ArtifactDownloadRefusal::DestinationExists);
    }
    if cancelled() {
        return Err(ArtifactDownloadRefusal::Cancelled);
    }
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| ArtifactDownloadRefusal::SourceUnavailable)?;
    let temporary = temporary_path(destination)?;
    let mut output = create_partial(&temporary)?;
    let result = copy_exact(source, &mut output, artifact, &mut cancelled);
    drop(output);
    let rewind = source
        .seek(SeekFrom::Start(0))
        .map_err(|_| ArtifactDownloadRefusal::SourceUnavailable);
    if let Err(error) = result.and(rewind.map(|_| ())) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if cancelled() {
        let _ = fs::remove_file(&temporary);
        return Err(ArtifactDownloadRefusal::Cancelled);
    }
    // Publish without replacing a concurrently created destination. Both names
    // are in the same directory. A filesystem lacking this atomic operation
    // refuses rather than weakening the no-overwrite contract.
    fs::hard_link(&temporary, destination).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            ArtifactDownloadRefusal::DestinationExists
        } else {
            ArtifactDownloadRefusal::DestinationUnavailable
        }
    })?;
    // Destination already owns the verified inode; failed cleanup must not turn
    // a completed publication into a false claim that no output exists.
    let _ = fs::remove_file(&temporary);
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

fn validate(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
) -> Result<(), ArtifactDownloadRefusal> {
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
    Ok(())
}

fn temporary_path(destination: &Path) -> Result<PathBuf, ArtifactDownloadRefusal> {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ArtifactDownloadRefusal::DestinationUnavailable)?;
    Ok(destination.with_file_name(format!(".{file_name}.conduit-partial")))
}

fn create_partial(temporary: &Path) -> Result<File, ArtifactDownloadRefusal> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(temporary)
        .map_err(|_| ArtifactDownloadRefusal::DestinationUnavailable)
}

fn copy_exact<R: Read>(
    input: &mut R,
    output: &mut File,
    artifact: &BodyBoundArtifactIdentity,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<(), ArtifactDownloadRefusal> {
    let mut digest = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        if cancelled() {
            return Err(ArtifactDownloadRefusal::Cancelled);
        }
        let capacity = (artifact.artifact_bytes - copied)
            .min(buffer.len() as u64)
            .max(1) as usize;
        let count = input
            .read(&mut buffer[..capacity])
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
    if cancelled() {
        return Err(ArtifactDownloadRefusal::Cancelled);
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

    mod handles;

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
