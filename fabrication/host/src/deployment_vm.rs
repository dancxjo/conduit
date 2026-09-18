//! Exact VM launch carrier for a verified Body-bound ConduitOS artifact.

use crate::{
    BodyBoundArtifactIdentity, CarrierTerminal, DeploymentCarrierDescriptor, DeploymentCarrierKind,
    DeploymentCarrierRefusal, DeploymentCarrierRequest, DeploymentRealizationReceipt,
    DEPLOYMENT_RECEIPT_SCHEMA, MAXIMUM_CARRIER_TEXT_BYTES,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const VERIFY_BUFFER_BYTES: usize = 64 * 1024;
pub const CONDUITOS_X86_64_QEMU_PROFILE: &str = "qemu-x86_64-q35-single-cpu-512m-cdrom";
pub const CONDUITOS_X86_64_QEMU_IMPLEMENTATION: &str = "conduit-helper/qemu-x86_64@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualMachineLaunchRefusal {
    Carrier(DeploymentCarrierRefusal),
    UnsupportedCarrier,
    InvalidProfile,
    ArtifactUnavailable,
    ExtentMismatch,
    ContentMismatch,
    LauncherUnavailable,
}

impl From<DeploymentCarrierRefusal> for VirtualMachineLaunchRefusal {
    fn from(value: DeploymentCarrierRefusal) -> Self {
        Self::Carrier(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualMachineLaunchReceipt {
    pub realization: DeploymentRealizationReceipt,
    pub launch_profile: String,
    pub launcher_process_id: u32,
}

pub trait VirtualMachineLauncher {
    fn launch(
        &mut self,
        exact_artifact: &Path,
        launch_profile: &str,
    ) -> Result<u32, VirtualMachineLaunchRefusal>;
}

/// Reviewed x86_64 QEMU realization. Spawning QEMU proves only carrier launch;
/// serial or guest evidence must separately establish Boot and later stages.
pub struct QemuX86_64Launcher {
    executable: PathBuf,
}

impl QemuX86_64Launcher {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
}

impl VirtualMachineLauncher for QemuX86_64Launcher {
    fn launch(
        &mut self,
        exact_artifact: &Path,
        launch_profile: &str,
    ) -> Result<u32, VirtualMachineLaunchRefusal> {
        if launch_profile != CONDUITOS_X86_64_QEMU_PROFILE {
            return Err(VirtualMachineLaunchRefusal::InvalidProfile);
        }
        Command::new(&self.executable)
            .args([
                "-machine", "q35", "-cpu", "max", "-smp", "1", "-m", "512M", "-boot", "d", "-cdrom",
            ])
            .arg(exact_artifact)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|child| child.id())
            .map_err(|_| VirtualMachineLaunchRefusal::LauncherUnavailable)
    }
}

pub fn launch_body_bound_virtual_machine(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    artifact_path: &Path,
    launch_profile: &str,
    explicit_authority: bool,
    launcher: &mut dyn VirtualMachineLauncher,
) -> Result<VirtualMachineLaunchReceipt, VirtualMachineLaunchRefusal> {
    if descriptor.kind != DeploymentCarrierKind::VirtualMachineLaunch
        || descriptor.implementation_id != CONDUITOS_X86_64_QEMU_IMPLEMENTATION
    {
        return Err(VirtualMachineLaunchRefusal::UnsupportedCarrier);
    }
    if launch_profile.is_empty() || launch_profile.len() > MAXIMUM_CARRIER_TEXT_BYTES {
        return Err(VirtualMachineLaunchRefusal::InvalidProfile);
    }
    descriptor.validate_request(&DeploymentCarrierRequest {
        carrier_id: &descriptor.carrier_id,
        target_id: &descriptor.target_id,
        artifact,
        explicit_authority,
        destructive_destination: None,
        confirmed_destination: None,
        destination_is_removable: None,
    })?;
    verify_artifact(artifact_path, artifact)?;
    let launcher_process_id = launcher.launch(artifact_path, launch_profile)?;
    if launcher_process_id == 0 {
        return Err(VirtualMachineLaunchRefusal::LauncherUnavailable);
    }
    let realization = DeploymentRealizationReceipt {
        schema: DEPLOYMENT_RECEIPT_SCHEMA.into(),
        carrier_id: descriptor.carrier_id.clone(),
        implementation_id: descriptor.implementation_id.clone(),
        target_id: descriptor.target_id.clone(),
        spore_id: artifact.spore_id.clone(),
        artifact_content_sha256: artifact.artifact_content_sha256.clone(),
        terminal: CarrierTerminal::Completed,
        bytes_realized: artifact.artifact_bytes,
        byte_verification_completed: true,
        explicit_authority_observed: true,
        boot_observed: false,
        join_observed: false,
        membership_admitted: false,
    };
    descriptor.validate_receipt(artifact, &realization)?;
    Ok(VirtualMachineLaunchReceipt {
        realization,
        launch_profile: launch_profile.into(),
        launcher_process_id,
    })
}

fn verify_artifact(
    path: &Path,
    artifact: &BodyBoundArtifactIdentity,
) -> Result<(), VirtualMachineLaunchRefusal> {
    let metadata =
        fs::metadata(path).map_err(|_| VirtualMachineLaunchRefusal::ArtifactUnavailable)?;
    if !metadata.is_file() {
        return Err(VirtualMachineLaunchRefusal::ArtifactUnavailable);
    }
    if metadata.len() != artifact.artifact_bytes {
        return Err(VirtualMachineLaunchRefusal::ExtentMismatch);
    }
    let mut file =
        File::open(path).map_err(|_| VirtualMachineLaunchRefusal::ArtifactUnavailable)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; VERIFY_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| VirtualMachineLaunchRefusal::ArtifactUnavailable)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if format!("sha256:{:x}", digest.finalize()) != artifact.artifact_content_sha256 {
        return Err(VirtualMachineLaunchRefusal::ContentMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEPLOYMENT_CARRIER_SCHEMA;

    struct CapturingLauncher {
        calls: usize,
        pid: u32,
    }

    impl VirtualMachineLauncher for CapturingLauncher {
        fn launch(
            &mut self,
            exact_artifact: &Path,
            launch_profile: &str,
        ) -> Result<u32, VirtualMachineLaunchRefusal> {
            assert!(exact_artifact.ends_with("body-bound.iso"));
            assert_eq!(launch_profile, CONDUITOS_X86_64_QEMU_PROFILE);
            self.calls += 1;
            Ok(self.pid)
        }
    }

    fn fixture(bytes: &[u8]) -> (DeploymentCarrierDescriptor, BodyBoundArtifactIdentity) {
        let target_id = "conduitos/x86_64/pc".to_owned();
        (
            DeploymentCarrierDescriptor {
                schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
                carrier_id: "conduit-carrier/qemu-x86_64@1".into(),
                target_id: target_id.clone(),
                kind: DeploymentCarrierKind::VirtualMachineLaunch,
                implementation_id: "conduit-helper/qemu-x86_64@1".into(),
                maximum_artifact_bytes: 4096,
                requires_explicit_authority: true,
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

    #[test]
    fn exact_artifact_launch_retains_carrier_evidence_without_boot_claims() {
        let bytes = b"exact Body-bound ConduitOS ISO";
        let (descriptor, artifact) = fixture(bytes);
        let root = std::env::temp_dir().join(format!("conduit-vm-launch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let image = root.join("body-bound.iso");
        fs::write(&image, bytes).unwrap();
        let mut launcher = CapturingLauncher { calls: 0, pid: 42 };

        let receipt = launch_body_bound_virtual_machine(
            &descriptor,
            &artifact,
            &image,
            CONDUITOS_X86_64_QEMU_PROFILE,
            true,
            &mut launcher,
        )
        .unwrap();

        assert_eq!(launcher.calls, 1);
        assert_eq!(receipt.launcher_process_id, 42);
        assert_eq!(receipt.realization.terminal, CarrierTerminal::Completed);
        assert!(!receipt.realization.boot_observed);
        assert!(!receipt.realization.join_observed);
        assert!(!receipt.realization.membership_admitted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn authority_or_content_mismatch_refuses_before_launch() {
        let bytes = b"expected exact ISO";
        let (descriptor, artifact) = fixture(bytes);
        let root = std::env::temp_dir().join(format!("conduit-vm-refusal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let image = root.join("body-bound.iso");
        fs::write(&image, b"tampered exact ISO").unwrap();
        let mut launcher = CapturingLauncher { calls: 0, pid: 42 };

        let mut relabeled = descriptor.clone();
        relabeled.implementation_id = "unreviewed/qemu-wrapper@1".into();
        assert_eq!(
            launch_body_bound_virtual_machine(
                &relabeled,
                &artifact,
                &image,
                CONDUITOS_X86_64_QEMU_PROFILE,
                true,
                &mut launcher,
            ),
            Err(VirtualMachineLaunchRefusal::UnsupportedCarrier)
        );

        assert_eq!(
            launch_body_bound_virtual_machine(
                &descriptor,
                &artifact,
                &image,
                CONDUITOS_X86_64_QEMU_PROFILE,
                false,
                &mut launcher,
            ),
            Err(VirtualMachineLaunchRefusal::Carrier(
                DeploymentCarrierRefusal::AuthorityRequired
            ))
        );
        assert_eq!(
            launch_body_bound_virtual_machine(
                &descriptor,
                &artifact,
                &image,
                CONDUITOS_X86_64_QEMU_PROFILE,
                true,
                &mut launcher,
            ),
            Err(VirtualMachineLaunchRefusal::ContentMismatch)
        );
        assert_eq!(launcher.calls, 0);
        fs::remove_dir_all(root).unwrap();
    }
}
