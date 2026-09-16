//! Consequential whole-device carrier for exact Body-bound boot media.

use crate::{
    BodyBoundArtifactIdentity, CarrierTerminal, DeploymentCarrierDescriptor, DeploymentCarrierKind,
    DeploymentCarrierRefusal, DeploymentCarrierRequest, DeploymentRealizationReceipt,
    DEPLOYMENT_RECEIPT_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::Path,
};

const COPY_BUFFER_BYTES: usize = 64 * 1024;
pub const LINUX_REMOVABLE_WRITER_IMPLEMENTATION: &str = "conduit-helper/linux-removable-writer@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemovableWriteRefusal {
    Carrier(DeploymentCarrierRefusal),
    UnsupportedCarrier,
    SourceUnavailable,
    ExtentMismatch,
    ContentMismatch,
    DestinationUnavailable,
    DestinationNotBlockDevice,
    DestinationNotRemovable,
    DestinationMounted,
    WriteFailed,
    ReadbackFailed,
}

impl From<DeploymentCarrierRefusal> for RemovableWriteRefusal {
    fn from(value: DeploymentCarrierRefusal) -> Self {
        Self::Carrier(value)
    }
}

pub trait RemovableDeviceInspector {
    fn confirm_safe_whole_device(&self, destination: &Path) -> Result<(), RemovableWriteRefusal>;
}

pub struct LinuxRemovableDeviceInspector;

#[cfg(target_os = "linux")]
impl RemovableDeviceInspector for LinuxRemovableDeviceInspector {
    fn confirm_safe_whole_device(&self, destination: &Path) -> Result<(), RemovableWriteRefusal> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};

        let canonical = fs::canonicalize(destination)
            .map_err(|_| RemovableWriteRefusal::DestinationUnavailable)?;
        let metadata =
            fs::metadata(&canonical).map_err(|_| RemovableWriteRefusal::DestinationUnavailable)?;
        if !metadata.file_type().is_block_device() {
            return Err(RemovableWriteRefusal::DestinationNotBlockDevice);
        }
        let name = canonical
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(RemovableWriteRefusal::DestinationUnavailable)?;
        let removable = fs::read_to_string(format!("/sys/class/block/{name}/removable"))
            .map_err(|_| RemovableWriteRefusal::DestinationNotRemovable)?;
        if removable.trim() != "1" {
            return Err(RemovableWriteRefusal::DestinationNotRemovable);
        }
        let device = metadata.rdev();
        let major = ((device >> 8) & 0xfff) | ((device >> 32) & !0xfff);
        let minor = (device & 0xff) | ((device >> 12) & !0xff);
        let identity = format!("{major}:{minor}");
        let mountinfo = fs::read_to_string("/proc/self/mountinfo")
            .map_err(|_| RemovableWriteRefusal::DestinationUnavailable)?;
        if mountinfo
            .lines()
            .any(|line| line.split_ascii_whitespace().nth(2) == Some(identity.as_str()))
        {
            return Err(RemovableWriteRefusal::DestinationMounted);
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
impl RemovableDeviceInspector for LinuxRemovableDeviceInspector {
    fn confirm_safe_whole_device(&self, _destination: &Path) -> Result<(), RemovableWriteRefusal> {
        Err(RemovableWriteRefusal::DestinationUnavailable)
    }
}

pub fn write_body_bound_artifact_to_removable(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    source: &Path,
    destination: &Path,
    confirmed_destination: &str,
    explicit_authority: bool,
) -> Result<DeploymentRealizationReceipt, RemovableWriteRefusal> {
    write_with_control(
        descriptor,
        artifact,
        source,
        destination,
        confirmed_destination,
        explicit_authority,
        &LinuxRemovableDeviceInspector,
        || false,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_with_control(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    source: &Path,
    destination: &Path,
    confirmed_destination: &str,
    explicit_authority: bool,
    inspector: &dyn RemovableDeviceInspector,
    mut interrupted: impl FnMut() -> bool,
) -> Result<DeploymentRealizationReceipt, RemovableWriteRefusal> {
    if descriptor.kind != DeploymentCarrierKind::RemovableWholeDeviceWrite
        || descriptor.implementation_id != LINUX_REMOVABLE_WRITER_IMPLEMENTATION
        || !descriptor.verifies_written_bytes
    {
        return Err(RemovableWriteRefusal::UnsupportedCarrier);
    }
    let destination_name = destination
        .to_str()
        .ok_or(RemovableWriteRefusal::DestinationUnavailable)?;
    descriptor.validate_request(&DeploymentCarrierRequest {
        carrier_id: &descriptor.carrier_id,
        target_id: &descriptor.target_id,
        artifact,
        explicit_authority,
        destructive_destination: Some(destination_name),
        confirmed_destination: Some(confirmed_destination),
        destination_is_removable: Some(true),
    })?;
    let mut input = File::open(source).map_err(|_| RemovableWriteRefusal::SourceUnavailable)?;
    let source_metadata = input
        .metadata()
        .map_err(|_| RemovableWriteRefusal::SourceUnavailable)?;
    if !source_metadata.is_file() {
        return Err(RemovableWriteRefusal::SourceUnavailable);
    }
    if source_metadata.len() != artifact.artifact_bytes {
        return Err(RemovableWriteRefusal::ExtentMismatch);
    }
    verify_reader(
        &mut input,
        artifact,
        RemovableWriteRefusal::SourceUnavailable,
    )?;
    input
        .rewind()
        .map_err(|_| RemovableWriteRefusal::SourceUnavailable)?;
    inspector.confirm_safe_whole_device(destination)?;
    let mut output = OpenOptions::new()
        .write(true)
        .open(destination)
        .map_err(|_| RemovableWriteRefusal::DestinationUnavailable)?;
    let mut copied = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        if interrupted() {
            output
                .sync_all()
                .map_err(|_| RemovableWriteRefusal::WriteFailed)?;
            return receipt(
                descriptor,
                artifact,
                CarrierTerminal::Interrupted,
                copied,
                false,
            );
        }
        let count = input
            .read(&mut buffer)
            .map_err(|_| RemovableWriteRefusal::SourceUnavailable)?;
        if count == 0 {
            break;
        }
        copied = copied
            .checked_add(count as u64)
            .filter(|bytes| *bytes <= artifact.artifact_bytes)
            .ok_or(RemovableWriteRefusal::ExtentMismatch)?;
        output
            .write_all(&buffer[..count])
            .map_err(|_| RemovableWriteRefusal::WriteFailed)?;
    }
    output
        .sync_all()
        .map_err(|_| RemovableWriteRefusal::WriteFailed)?;
    if copied != artifact.artifact_bytes {
        return Err(RemovableWriteRefusal::ExtentMismatch);
    }
    let mut readback =
        File::open(destination).map_err(|_| RemovableWriteRefusal::ReadbackFailed)?;
    verify_reader(
        &mut readback,
        artifact,
        RemovableWriteRefusal::ReadbackFailed,
    )?;
    receipt(
        descriptor,
        artifact,
        CarrierTerminal::Completed,
        copied,
        true,
    )
}

fn receipt(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    terminal: CarrierTerminal,
    bytes_realized: u64,
    verified: bool,
) -> Result<DeploymentRealizationReceipt, RemovableWriteRefusal> {
    let receipt = DeploymentRealizationReceipt {
        schema: DEPLOYMENT_RECEIPT_SCHEMA.into(),
        carrier_id: descriptor.carrier_id.clone(),
        implementation_id: descriptor.implementation_id.clone(),
        target_id: descriptor.target_id.clone(),
        spore_id: artifact.spore_id.clone(),
        artifact_content_sha256: artifact.artifact_content_sha256.clone(),
        terminal,
        bytes_realized,
        byte_verification_completed: verified,
        explicit_authority_observed: true,
        boot_observed: false,
        join_observed: false,
        membership_admitted: false,
    };
    descriptor.validate_receipt(artifact, &receipt)?;
    Ok(receipt)
}

fn verify_reader(
    file: &mut File,
    artifact: &BodyBoundArtifactIdentity,
    unavailable: RemovableWriteRefusal,
) -> Result<(), RemovableWriteRefusal> {
    let mut digest = Sha256::new();
    let mut remaining = artifact.artifact_bytes;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    while remaining > 0 {
        let maximum = usize::try_from(remaining.min(COPY_BUFFER_BYTES as u64)).unwrap();
        let count = file.read(&mut buffer[..maximum]).map_err(|_| unavailable)?;
        if count == 0 {
            return Err(RemovableWriteRefusal::ExtentMismatch);
        }
        digest.update(&buffer[..count]);
        remaining -= count as u64;
    }
    if format!("sha256:{:x}", digest.finalize()) != artifact.artifact_content_sha256 {
        return Err(RemovableWriteRefusal::ContentMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEPLOYMENT_CARRIER_SCHEMA;

    struct TestInspector;

    impl RemovableDeviceInspector for TestInspector {
        fn confirm_safe_whole_device(
            &self,
            _destination: &Path,
        ) -> Result<(), RemovableWriteRefusal> {
            Ok(())
        }
    }

    fn fixture(bytes: &[u8]) -> (DeploymentCarrierDescriptor, BodyBoundArtifactIdentity) {
        let target_id = "conduitos/x86_64/pc".to_owned();
        (
            DeploymentCarrierDescriptor {
                schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
                carrier_id: "conduit-carrier/removable-whole-device@1".into(),
                target_id: target_id.clone(),
                kind: DeploymentCarrierKind::RemovableWholeDeviceWrite,
                implementation_id: "conduit-helper/linux-removable-writer@1".into(),
                maximum_artifact_bytes: 1024 * 1024,
                requires_explicit_authority: true,
                verifies_written_bytes: true,
            },
            BodyBoundArtifactIdentity {
                target_id,
                image_id: "image/reviewed".into(),
                image_content_sha256: format!("sha256:{:x}", Sha256::digest(b"generic")),
                spore_id: "spore/body-one/host-removable".into(),
                artifact_content_sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
                artifact_bytes: bytes.len() as u64,
            },
        )
    }

    #[test]
    fn exact_confirmed_media_is_written_and_read_back_without_boot_claims() {
        let bytes = b"exact Body-bound boot image";
        let (descriptor, artifact) = fixture(bytes);
        let root = std::env::temp_dir().join(format!("conduit-removable-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.img");
        let destination = root.join("device");
        fs::write(&source, bytes).unwrap();
        fs::write(&destination, vec![0_u8; bytes.len()]).unwrap();
        let confirmed = destination.to_str().unwrap();

        let result = write_with_control(
            &descriptor,
            &artifact,
            &source,
            &destination,
            confirmed,
            true,
            &TestInspector,
            || false,
        )
        .unwrap();

        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(result.terminal, CarrierTerminal::Completed);
        assert!(result.byte_verification_completed);
        assert!(!result.boot_observed && !result.join_observed && !result.membership_admitted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mismatch_and_interruption_never_produce_completed_evidence() {
        let bytes = vec![9_u8; COPY_BUFFER_BYTES + 17];
        let (descriptor, artifact) = fixture(&bytes);
        let root =
            std::env::temp_dir().join(format!("conduit-removable-stop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.img");
        let destination = root.join("device");
        fs::write(&source, &bytes).unwrap();
        fs::write(&destination, vec![0_u8; bytes.len()]).unwrap();
        assert!(matches!(
            write_with_control(
                &descriptor,
                &artifact,
                &source,
                &destination,
                "different-device",
                true,
                &TestInspector,
                || false,
            ),
            Err(RemovableWriteRefusal::Carrier(
                DeploymentCarrierRefusal::AmbiguousDestination
            ))
        ));
        let mut polls = 0;
        let interrupted = write_with_control(
            &descriptor,
            &artifact,
            &source,
            &destination,
            destination.to_str().unwrap(),
            true,
            &TestInspector,
            || {
                polls += 1;
                polls > 1
            },
        )
        .unwrap();
        assert_eq!(interrupted.terminal, CarrierTerminal::Interrupted);
        assert_eq!(interrupted.bytes_realized, COPY_BUFFER_BYTES as u64);
        assert!(!interrupted.byte_verification_completed);
        assert!(!interrupted.boot_observed);
        fs::remove_dir_all(root).unwrap();
    }
}
