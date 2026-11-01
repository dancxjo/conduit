//! Installed RP2040 UF2 mass-storage carrier for an exact body-bound spore.

use crate::{
    BodyBoundArtifactIdentity, CarrierTerminal, DeploymentCarrierDescriptor, DeploymentCarrierKind,
    DeploymentCarrierRefusal, DeploymentCarrierRequest, DeploymentRealizationReceipt,
    DEPLOYMENT_RECEIPT_SCHEMA,
};
use conduit_rp2040_flash_layout::{Uf2Refusal, Uf2Validator, UF2_BLOCK_BYTES};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek},
    path::Path,
};

const VERIFY_BUFFER_BYTES: usize = 64 * 1024;
const MAXIMUM_VOLUME_IDENTITY_BYTES: u64 = 4 * 1024;
const UF2_VOLUME_IDENTITY: &str = "INFO_UF2.TXT";
const INSTALLED_UF2_NAME: &str = "CONDUIT.UF2";
pub const RP2040_UF2_FLASH_IMPLEMENTATION: &str = "conduit-host-rp2040/flash-uf2@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Uf2FlashRefusal {
    Carrier(DeploymentCarrierRefusal),
    UnsupportedCarrier,
    ArtifactUnavailable,
    ExtentMismatch,
    ContentMismatch,
    InvalidImage(Uf2Refusal),
    VolumeUnavailable,
    WrongVolume,
    AmbiguousDestination,
    DestinationExists,
    WriteFailed,
}

impl From<DeploymentCarrierRefusal> for Uf2FlashRefusal {
    fn from(value: DeploymentCarrierRefusal) -> Self {
        Self::Carrier(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Uf2FlashReceipt {
    pub realization: DeploymentRealizationReceipt,
    pub volume_identity_sha256: String,
}

pub fn flash_body_bound_rp2040_uf2(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    source: &Path,
    bootsel_volume: &Path,
    confirmed_volume: &str,
    explicit_authority: bool,
) -> Result<Uf2FlashReceipt, Uf2FlashRefusal> {
    if descriptor.kind != DeploymentCarrierKind::MicrocontrollerFlash
        || descriptor.implementation_id != RP2040_UF2_FLASH_IMPLEMENTATION
        || descriptor.verifies_written_bytes
    {
        return Err(Uf2FlashRefusal::UnsupportedCarrier);
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
    let volume_name = bootsel_volume
        .to_str()
        .ok_or(Uf2FlashRefusal::VolumeUnavailable)?;
    if volume_name != confirmed_volume {
        return Err(Uf2FlashRefusal::AmbiguousDestination);
    }
    let volume_metadata =
        fs::metadata(bootsel_volume).map_err(|_| Uf2FlashRefusal::VolumeUnavailable)?;
    if !volume_metadata.is_dir() {
        return Err(Uf2FlashRefusal::VolumeUnavailable);
    }
    let identity_path = bootsel_volume.join(UF2_VOLUME_IDENTITY);
    let identity_metadata =
        fs::metadata(&identity_path).map_err(|_| Uf2FlashRefusal::WrongVolume)?;
    if !identity_metadata.is_file()
        || identity_metadata.len() == 0
        || identity_metadata.len() > MAXIMUM_VOLUME_IDENTITY_BYTES
    {
        return Err(Uf2FlashRefusal::WrongVolume);
    }
    let volume_identity = fs::read(&identity_path).map_err(|_| Uf2FlashRefusal::WrongVolume)?;
    let identity_text =
        core::str::from_utf8(&volume_identity).map_err(|_| Uf2FlashRefusal::WrongVolume)?;
    if !identity_text.lines().any(|line| {
        line.trim() == "Model: Raspberry Pi RP2" || line.trim().starts_with("Board-ID: RPI-RP2")
    }) {
        return Err(Uf2FlashRefusal::WrongVolume);
    }
    let mut input = verify_artifact(source, artifact)?;
    let destination = bootsel_volume.join(INSTALLED_UF2_NAME);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Uf2FlashRefusal::DestinationExists
            } else {
                Uf2FlashRefusal::WriteFailed
            }
        })?;
    let copied = std::io::copy(&mut (&mut input).take(artifact.artifact_bytes), &mut output)
        .map_err(|_| Uf2FlashRefusal::WriteFailed)?;
    output
        .sync_all()
        .map_err(|_| Uf2FlashRefusal::WriteFailed)?;
    if copied != artifact.artifact_bytes {
        return Err(Uf2FlashRefusal::ExtentMismatch);
    }
    let realization = DeploymentRealizationReceipt {
        schema: DEPLOYMENT_RECEIPT_SCHEMA.into(),
        carrier_id: descriptor.carrier_id.clone(),
        implementation_id: descriptor.implementation_id.clone(),
        target_id: descriptor.target_id.clone(),
        spore_id: artifact.spore_id.clone(),
        artifact_content_sha256: artifact.artifact_content_sha256.clone(),
        terminal: CarrierTerminal::Completed,
        bytes_realized: copied,
        byte_verification_completed: false,
        explicit_authority_observed: true,
        boot_observed: false,
        join_observed: false,
        membership_admitted: false,
    };
    descriptor.validate_receipt(artifact, &realization)?;
    Ok(Uf2FlashReceipt {
        realization,
        volume_identity_sha256: format!("sha256:{:x}", Sha256::digest(&volume_identity)),
    })
}

fn verify_artifact(
    source: &Path,
    artifact: &BodyBoundArtifactIdentity,
) -> Result<File, Uf2FlashRefusal> {
    let metadata = fs::metadata(source).map_err(|_| Uf2FlashRefusal::ArtifactUnavailable)?;
    if !metadata.is_file() || metadata.len() != artifact.artifact_bytes {
        return Err(Uf2FlashRefusal::ExtentMismatch);
    }
    let mut input = File::open(source).map_err(|_| Uf2FlashRefusal::ArtifactUnavailable)?;
    let mut geometry = Uf2Validator::new(metadata.len()).map_err(Uf2FlashRefusal::InvalidImage)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; VERIFY_BUFFER_BYTES];
    let mut remaining = metadata.len();
    while remaining > 0 {
        let count = remaining.min(VERIFY_BUFFER_BYTES as u64) as usize;
        input
            .read_exact(&mut buffer[..count])
            .map_err(|_| Uf2FlashRefusal::ArtifactUnavailable)?;
        for block in buffer[..count].as_chunks::<UF2_BLOCK_BYTES>().0 {
            geometry
                .push(block)
                .map_err(Uf2FlashRefusal::InvalidImage)?;
        }
        digest.update(&buffer[..count]);
        remaining -= count as u64;
    }
    geometry.finish().map_err(Uf2FlashRefusal::InvalidImage)?;
    if format!("sha256:{:x}", digest.finalize()) != artifact.artifact_content_sha256 {
        return Err(Uf2FlashRefusal::ContentMismatch);
    }
    // Retain the verified source handle rather than resolving the path again.
    input
        .rewind()
        .map_err(|_| Uf2FlashRefusal::ArtifactUnavailable)?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEPLOYMENT_CARRIER_SCHEMA;

    fn fixture(bytes: &[u8]) -> (DeploymentCarrierDescriptor, BodyBoundArtifactIdentity) {
        let target_id = "conduitos/thumbv6m/pico-w".to_owned();
        (
            DeploymentCarrierDescriptor {
                schema: DEPLOYMENT_CARRIER_SCHEMA.into(),
                carrier_id: "conduit-carrier/rp2040-uf2@1".into(),
                target_id: target_id.clone(),
                kind: DeploymentCarrierKind::MicrocontrollerFlash,
                implementation_id: RP2040_UF2_FLASH_IMPLEMENTATION.into(),
                maximum_artifact_bytes: 2 * 1024 * 1024,
                requires_explicit_authority: true,
                verifies_written_bytes: false,
            },
            BodyBoundArtifactIdentity {
                target_id,
                image_id: "image/rp2040-reviewed".into(),
                image_content_sha256: format!("sha256:{:x}", Sha256::digest(b"generic")),
                spore_id: "spore/body-one/pico".into(),
                artifact_content_sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
                artifact_bytes: bytes.len() as u64,
            },
        )
    }

    fn image(address: u32) -> [u8; 512] {
        let mut block = [0; 512];
        for (offset, value) in [
            (0, 0x0a324655u32),
            (4, 0x9e5d5157),
            (8, 0x2000),
            (12, address),
            (16, 256),
            (20, 0),
            (24, 1),
            (28, 0xe48bff56),
            (508, 0x0ab16f30),
        ] {
            block[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        block
    }

    fn root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("conduit-uf2-{name}-{}", std::process::id()))
    }

    #[test]
    fn exact_spore_flashes_only_to_the_confirmed_rp2040_volume() {
        let bytes = &image(conduit_rp2040_flash_layout::FLASH_START);
        let (descriptor, artifact) = fixture(bytes);
        let root = root("exact");
        let volume = root.join("RPI-RP2");
        fs::create_dir_all(&volume).unwrap();
        fs::write(
            volume.join(UF2_VOLUME_IDENTITY),
            "Model: Raspberry Pi RP2\n",
        )
        .unwrap();
        let source = root.join("spore.uf2");
        fs::write(&source, bytes).unwrap();
        let confirmed = volume.to_str().unwrap();

        let receipt =
            flash_body_bound_rp2040_uf2(&descriptor, &artifact, &source, &volume, confirmed, true)
                .unwrap();
        assert_eq!(fs::read(volume.join(INSTALLED_UF2_NAME)).unwrap(), bytes);
        assert_eq!(receipt.realization.terminal, CarrierTerminal::Completed);
        assert!(!receipt.realization.boot_observed);
        assert!(!receipt.realization.join_observed);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wrong_volume_authority_and_relabeling_refuse_before_write() {
        let bytes = &image(conduit_rp2040_flash_layout::FLASH_START);
        let (descriptor, artifact) = fixture(bytes);
        let root = root("refusal");
        let volume = root.join("volume");
        fs::create_dir_all(&volume).unwrap();
        fs::write(
            volume.join(UF2_VOLUME_IDENTITY),
            "Model: unrelated device\n",
        )
        .unwrap();
        let source = root.join("spore.uf2");
        fs::write(&source, bytes).unwrap();
        let confirmed = volume.to_str().unwrap();

        assert_eq!(
            flash_body_bound_rp2040_uf2(&descriptor, &artifact, &source, &volume, confirmed, true,),
            Err(Uf2FlashRefusal::WrongVolume)
        );
        fs::write(
            volume.join(UF2_VOLUME_IDENTITY),
            "Model: Raspberry Pi RP2\n",
        )
        .unwrap();
        assert!(matches!(
            flash_body_bound_rp2040_uf2(&descriptor, &artifact, &source, &volume, confirmed, false,),
            Err(Uf2FlashRefusal::Carrier(
                DeploymentCarrierRefusal::AuthorityRequired
            ))
        ));
        let mut relabeled = descriptor;
        relabeled.implementation_id = "unreviewed/uf2-copy@1".into();
        assert_eq!(
            flash_body_bound_rp2040_uf2(&relabeled, &artifact, &source, &volume, confirmed, true,),
            Err(Uf2FlashRefusal::UnsupportedCarrier)
        );
        assert!(!volume.join(INSTALLED_UF2_NAME).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn matching_digest_cannot_admit_reserved_or_malformed_pages() {
        let root = root("geometry");
        let volume = root.join("RPI-RP2");
        fs::create_dir_all(&volume).unwrap();
        fs::write(
            volume.join(UF2_VOLUME_IDENTITY),
            "Model: Raspberry Pi RP2\n",
        )
        .unwrap();
        let source = root.join("spore.uf2");
        let confirmed = volume.to_str().unwrap();
        let mut malformed = image(conduit_rp2040_flash_layout::FLASH_START);
        malformed[0] = 0;
        // Cross the verifier's 64 KiB read buffer before presenting a forbidden
        // page, so geometry cannot accidentally be checked only in its prefix.
        let mut late_reserved = Vec::with_capacity(129 * UF2_BLOCK_BYTES);
        for index in 0..129u32 {
            let address = if index == 128 {
                conduit_rp2040_flash_layout::CREDENTIALS.address()
            } else {
                conduit_rp2040_flash_layout::FLASH_START + index * 256
            };
            let mut page = image(address);
            page[20..24].copy_from_slice(&index.to_le_bytes());
            page[24..28].copy_from_slice(&129u32.to_le_bytes());
            late_reserved.extend_from_slice(&page);
        }
        for bytes in [
            image(conduit_rp2040_flash_layout::CREDENTIALS.address()).to_vec(),
            image(conduit_rp2040_flash_layout::BOOTSTRAP.address() - 256).to_vec(),
            malformed.to_vec(),
            late_reserved,
        ] {
            let (descriptor, artifact) = fixture(&bytes);
            fs::write(&source, bytes).unwrap();
            assert!(matches!(
                flash_body_bound_rp2040_uf2(
                    &descriptor,
                    &artifact,
                    &source,
                    &volume,
                    confirmed,
                    true
                ),
                Err(Uf2FlashRefusal::InvalidImage(_))
            ));
            assert!(!volume.join(INSTALLED_UF2_NAME).exists());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
