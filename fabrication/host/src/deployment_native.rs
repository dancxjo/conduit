//! Reviewed local install/start carrier for one native Body-bound package.

use crate::{
    BodyBoundArtifactIdentity, CarrierTerminal, DeploymentCarrierDescriptor, DeploymentCarrierKind,
    DeploymentCarrierRefusal, DeploymentCarrierRequest, DeploymentRealizationReceipt,
    DEPLOYMENT_RECEIPT_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub const NATIVE_INSTALL_START_IMPLEMENTATION: &str = "conduit/native-install-start@1";
const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeInstallOutcome {
    pub package_bytes_consumed: u64,
    pub byte_verification_completed: bool,
    pub start_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeInstallRefusal {
    Carrier(DeploymentCarrierRefusal),
    UnsupportedCarrier,
    SourceUnavailable,
    ExtentMismatch,
    ContentMismatch,
    PackageMalformed,
    BindingMismatch,
    ImageMismatch,
    InstallationFailed,
    Incomplete,
}

impl From<DeploymentCarrierRefusal> for NativeInstallRefusal {
    fn from(value: DeploymentCarrierRefusal) -> Self {
        Self::Carrier(value)
    }
}

pub trait NativePackageInstaller {
    fn install_and_start(
        &mut self,
        source: &Path,
        artifact: &BodyBoundArtifactIdentity,
    ) -> Result<NativeInstallOutcome, NativeInstallRefusal>;
}

pub fn install_start_body_bound_native_package(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    source: &Path,
    explicit_authority: bool,
    installer: &mut impl NativePackageInstaller,
) -> Result<DeploymentRealizationReceipt, NativeInstallRefusal> {
    if descriptor.kind != DeploymentCarrierKind::NativeInstallStart
        || descriptor.implementation_id != NATIVE_INSTALL_START_IMPLEMENTATION
    {
        return Err(NativeInstallRefusal::UnsupportedCarrier);
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
    verify_source(source, artifact)?;
    let outcome = installer.install_and_start(source, artifact)?;
    if outcome.package_bytes_consumed != artifact.artifact_bytes
        || !outcome.byte_verification_completed
        || !outcome.start_requested
    {
        return Err(NativeInstallRefusal::Incomplete);
    }
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
        explicit_authority_observed: true,
        boot_observed: false,
        join_observed: false,
        membership_admitted: false,
    };
    descriptor.validate_receipt(artifact, &receipt)?;
    Ok(receipt)
}

fn verify_source(
    source: &Path,
    artifact: &BodyBoundArtifactIdentity,
) -> Result<(), NativeInstallRefusal> {
    let mut file = File::open(source).map_err(|_| NativeInstallRefusal::SourceUnavailable)?;
    if file
        .metadata()
        .map_err(|_| NativeInstallRefusal::SourceUnavailable)?
        .len()
        != artifact.artifact_bytes
    {
        return Err(NativeInstallRefusal::ExtentMismatch);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| NativeInstallRefusal::SourceUnavailable)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if format!("sha256:{:x}", digest.finalize()) != artifact.artifact_content_sha256 {
        return Err(NativeInstallRefusal::ContentMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEPLOYMENT_CARRIER_SCHEMA;
    use std::{fs, path::PathBuf};

    struct Installer {
        outcome: Result<NativeInstallOutcome, NativeInstallRefusal>,
        called: bool,
    }

    impl NativePackageInstaller for Installer {
        fn install_and_start(
            &mut self,
            _source: &Path,
            _artifact: &BodyBoundArtifactIdentity,
        ) -> Result<NativeInstallOutcome, NativeInstallRefusal> {
            self.called = true;
            self.outcome
        }
    }

    fn fixture(bytes: &[u8]) -> (DeploymentCarrierDescriptor, BodyBoundArtifactIdentity) {
        let target_id = "std/x86_64/computer".to_owned();
        (
            DeploymentCarrierDescriptor {
                schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
                carrier_id: "conduit-carrier/native-install-start@1".into(),
                target_id: target_id.clone(),
                kind: DeploymentCarrierKind::NativeInstallStart,
                implementation_id: NATIVE_INSTALL_START_IMPLEMENTATION.into(),
                maximum_artifact_bytes: 4096,
                requires_explicit_authority: true,
                verifies_written_bytes: true,
            },
            BodyBoundArtifactIdentity {
                target_id,
                image_id: "image/native-reviewed".into(),
                image_content_sha256: format!("sha256:{:x}", Sha256::digest(b"image")),
                spore_id: "spore/body-one/native-host".into(),
                artifact_content_sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
                artifact_bytes: bytes.len() as u64,
            },
        )
    }

    fn source(name: &str, bytes: &[u8]) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "conduit-native-carrier-{name}-{}-{}",
            std::process::id(),
            bytes.len()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("body-bound.zip");
        fs::write(&source, bytes).unwrap();
        (root, source)
    }

    #[test]
    fn exact_native_package_requests_start_without_claiming_later_stages() {
        let bytes = b"exact native package";
        let (descriptor, artifact) = fixture(bytes);
        let (root, source) = source("exact", bytes);
        let mut installer = Installer {
            outcome: Ok(NativeInstallOutcome {
                package_bytes_consumed: bytes.len() as u64,
                byte_verification_completed: true,
                start_requested: true,
            }),
            called: false,
        };
        let receipt = install_start_body_bound_native_package(
            &descriptor,
            &artifact,
            &source,
            true,
            &mut installer,
        )
        .unwrap();
        assert!(installer.called);
        assert_eq!(receipt.terminal, CarrierTerminal::Completed);
        assert!(!receipt.boot_observed);
        assert!(!receipt.join_observed);
        assert!(!receipt.membership_admitted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn authority_and_content_refuse_before_installer_effects() {
        let bytes = b"exact native package";
        let (descriptor, artifact) = fixture(bytes);
        let (root, source) = source("refusal", bytes);
        let mut installer = Installer {
            outcome: Err(NativeInstallRefusal::InstallationFailed),
            called: false,
        };
        assert!(matches!(
            install_start_body_bound_native_package(
                &descriptor,
                &artifact,
                &source,
                false,
                &mut installer
            ),
            Err(NativeInstallRefusal::Carrier(
                DeploymentCarrierRefusal::AuthorityRequired
            ))
        ));
        assert!(!installer.called);
        fs::write(&source, b"EXACT native package").unwrap();
        assert_eq!(
            install_start_body_bound_native_package(
                &descriptor,
                &artifact,
                &source,
                true,
                &mut installer
            ),
            Err(NativeInstallRefusal::ContentMismatch)
        );
        assert!(!installer.called);
        fs::remove_dir_all(root).unwrap();
    }
}
