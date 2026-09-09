//! Attended physical evidence sealing for Mabel's two IA-32 BIOS boots.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use clap::Args as ClapArgs;
use serde::{Deserialize, Serialize};

use crate::{cli::GlobalOpts, workspace::workspace_root};

use super::{
    profile::{LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file},
    ConduitosError,
};

const SPECIMEN: &str = "mabel-copperbutton";
const MAX_PHOTO_BYTES: u64 = 16 * 1024 * 1024;
const IMAGE_PACKAGER: &str = "pinned-limine-hybrid-iso";
const BOOTLOADER: &str = "Limine";
const FIRMWARE_ENVIRONMENT: &str = "x86-bios";
const BIOS_BOOT_ENTRY: &str = "boot/limine/limine-bios-cd.bin";

#[derive(ClapArgs, Debug, Clone)]
pub(super) struct Args {
    /// Flash receipt produced by the guarded IA-32 whole-device write.
    #[arg(long)]
    flash_record: PathBuf,
    /// Photograph retaining the first complete VGA boot receipt.
    #[arg(long)]
    first_photo: PathBuf,
    /// Exact first HostId transcribed from the retained screen.
    #[arg(long)]
    first_host_id: String,
    /// Exact first BootId transcribed from the retained screen.
    #[arg(long)]
    first_boot_id: String,
    /// Photograph retaining the second complete VGA boot receipt.
    #[arg(long)]
    second_photo: PathBuf,
    /// Exact second HostId transcribed from the retained screen.
    #[arg(long)]
    second_host_id: String,
    /// Exact second BootId transcribed from the retained screen.
    #[arg(long)]
    second_boot_id: String,
    /// Repeat the exact image digest from the guarded flash receipt.
    #[arg(long)]
    confirm_image_sha256: String,
    /// Repeat `mabel-copperbutton` to bind the attended specimen.
    #[arg(long)]
    confirm_specimen: String,
    /// Attest that both photographs visibly contain every exact receipt field.
    #[arg(long)]
    attest_exact_screens: bool,
    /// New receipt path; existing physical evidence is never overwritten.
    #[arg(
        long,
        default_value = "target/conduitos/ia32/ia32-mabel-physical-proof.json"
    )]
    output: PathBuf,
}

#[derive(Deserialize)]
struct FlashRecord {
    schema: String,
    base_commit: String,
    architecture: String,
    host: String,
    profile_id: String,
    build_id: String,
    image_id: String,
    image_sha256: String,
    image_bytes: u64,
    image_packager: String,
    bootloader: String,
    bootloader_version: String,
    bootloader_archive_sha256: String,
    firmware_environment: String,
    firmware_boot_entry: String,
    carrier: String,
    device: String,
    removable: bool,
    device_bytes: u64,
    write_completed: bool,
    byte_verification_completed: bool,
    physical_boot_claimed: bool,
}

#[derive(Serialize)]
struct PhotoEvidence {
    path: String,
    media_type: &'static str,
    sha256: String,
    bytes: u64,
}

#[derive(Serialize)]
struct BootEvidence {
    ordinal: u8,
    host_id: String,
    boot_id: String,
    photo: PhotoEvidence,
    exact_screen_fields_human_attested: bool,
}

#[derive(Serialize)]
struct PhysicalProof {
    schema: &'static str,
    proof_class: &'static str,
    recorder_commit: String,
    image_source_commit: String,
    specimen: &'static str,
    specimen_class: &'static str,
    architecture: &'static str,
    firmware_environment: &'static str,
    image_packager: String,
    bootloader: String,
    bootloader_version: String,
    bootloader_archive_sha256: String,
    firmware_boot_entry: String,
    profile_id: String,
    build_id: String,
    image_id: String,
    image_sha256: String,
    image_bytes: u64,
    carrier: String,
    flash_device: String,
    flash_device_bytes: u64,
    removable_media_byte_verified: bool,
    first: BootEvidence,
    second: BootEvidence,
    stable_artifact_identity: bool,
    fresh_host_id: bool,
    fresh_boot_id: bool,
    physical_machine_booted: bool,
    internal_disk_written: bool,
    automated_screen_interpretation_claimed: bool,
}

pub(super) fn execute(args: &Args, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if args.confirm_specimen != SPECIMEN {
        return Err(refusal(
            "physical-specimen-confirmation-mismatch",
            format!("--confirm-specimen must be the exact selected specimen {SPECIMEN}"),
        ));
    }
    if !args.attest_exact_screens {
        return Err(refusal(
            "physical-screen-attestation-missing",
            "--attest-exact-screens is required only after both retained photographs visibly show every exact VGA receipt field",
        ));
    }

    let flash = read_flash_record(&args.flash_record)?;
    validate_flash_record(&flash, &args.confirm_image_sha256)?;
    validate_identity("first HostId", &args.first_host_id)?;
    validate_identity("first BootId", &args.first_boot_id)?;
    validate_identity("second HostId", &args.second_host_id)?;
    validate_identity("second BootId", &args.second_boot_id)?;
    if args.first_host_id == args.second_host_id {
        return Err(refusal(
            "stale-physical-host-id",
            "the two attended boots reused one HostId",
        ));
    }
    if args.first_boot_id == args.second_boot_id {
        return Err(refusal(
            "stale-physical-boot-id",
            "the two attended boots reused one BootId",
        ));
    }

    let first_photo = photo(&args.first_photo)?;
    let second_photo = photo(&args.second_photo)?;
    if first_photo.path == second_photo.path || first_photo.sha256 == second_photo.sha256 {
        return Err(refusal(
            "duplicate-physical-screen-evidence",
            "the two boots require distinct retained photographs",
        ));
    }

    let root = workspace_root().map_err(|error| refusal("workspace-unavailable", error))?;
    let proof = PhysicalProof {
        schema: "conduit.conduitos/ia32-mabel-physical-proof@2",
        proof_class: "attended-physical-ia32-legacy-bios",
        recorder_commit: git_head(&root)?,
        image_source_commit: flash.base_commit,
        specimen: SPECIMEN,
        specimen_class: "hp-pavilion-dv1660se-core-duo-t2300",
        architecture: "ia32",
        firmware_environment: FIRMWARE_ENVIRONMENT,
        image_packager: flash.image_packager,
        bootloader: flash.bootloader,
        bootloader_version: flash.bootloader_version,
        bootloader_archive_sha256: flash.bootloader_archive_sha256,
        firmware_boot_entry: flash.firmware_boot_entry,
        profile_id: flash.profile_id,
        build_id: flash.build_id,
        image_id: flash.image_id,
        image_sha256: flash.image_sha256,
        image_bytes: flash.image_bytes,
        carrier: flash.carrier,
        flash_device: flash.device,
        flash_device_bytes: flash.device_bytes,
        removable_media_byte_verified: true,
        first: BootEvidence {
            ordinal: 1,
            host_id: args.first_host_id.clone(),
            boot_id: args.first_boot_id.clone(),
            photo: first_photo,
            exact_screen_fields_human_attested: true,
        },
        second: BootEvidence {
            ordinal: 2,
            host_id: args.second_host_id.clone(),
            boot_id: args.second_boot_id.clone(),
            photo: second_photo,
            exact_screen_fields_human_attested: true,
        },
        stable_artifact_identity: true,
        fresh_host_id: true,
        fresh_boot_id: true,
        physical_machine_booted: true,
        internal_disk_written: false,
        automated_screen_interpretation_claimed: false,
    };
    let encoded = serde_json::to_vec_pretty(&proof)
        .map_err(|error| refusal("physical-proof-encoding-failed", error))?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would seal two attended {} IA-32 legacy-BIOS boots to {}",
                SPECIMEN,
                args.output.display()
            );
        }
        return Ok(());
    }
    write_new(&args.output, &encoded)?;
    if opts.json {
        println!(
            "{}",
            String::from_utf8(encoded)
                .map_err(|error| refusal("physical-proof-encoding-failed", error))?
        );
    } else if !opts.quiet {
        println!("physical Mabel proof: {}", args.output.display());
    }
    Ok(())
}

fn read_flash_record(path: &Path) -> Result<FlashRecord, ConduitosError> {
    let bytes = fs::read(path).map_err(|error| refusal("flash-record-unavailable", error))?;
    serde_json::from_slice(&bytes).map_err(|error| refusal("flash-record-invalid", error))
}

fn validate_flash_record(record: &FlashRecord, confirmed: &str) -> Result<(), ConduitosError> {
    if record.schema != "conduit.conduitos.ia32-live-flash/v2"
        || record.architecture != "ia32"
        || record.host != "conduitos/ia32/pc"
        || record.image_packager != IMAGE_PACKAGER
        || record.bootloader != BOOTLOADER
        || record.bootloader_version != LIMINE_VERSION
        || record.bootloader_archive_sha256 != LIMINE_ARCHIVE_SHA256
        || record.firmware_environment != FIRMWARE_ENVIRONMENT
        || record.firmware_boot_entry != BIOS_BOOT_ENTRY
        || record.carrier != "removable-whole-device"
        || !record.removable
        || !record.write_completed
        || !record.byte_verification_completed
        || record.physical_boot_claimed
        || record.image_bytes == 0
        || record.device_bytes < record.image_bytes
    {
        return Err(refusal(
            "flash-record-invalid",
            "record is not one completed byte-verified IA-32 removable-media write",
        ));
    }
    validate_sha256("flash image SHA-256", &record.image_sha256)?;
    validate_sha256(
        "bootloader archive SHA-256",
        &record.bootloader_archive_sha256,
    )
    .map_err(|_| refusal("flash-record-invalid", "invalid bootloader archive SHA-256"))?;
    if record.image_sha256 != confirmed {
        return Err(refusal(
            "physical-image-confirmation-mismatch",
            "--confirm-image-sha256 differs from the guarded flash receipt",
        ));
    }
    validate_prefixed_digest("profile identity", &record.profile_id, "sha256:")?;
    validate_prefixed_digest("build identity", &record.build_id, "build:sha256:")?;
    validate_prefixed_digest("image identity", &record.image_id, "image:sha256:")?;
    validate_hex("image source commit", &record.base_commit, 40)?;
    Ok(())
}

fn photo(path: &Path) -> Result<PhotoEvidence, ConduitosError> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| refusal("physical-screen-photo-unavailable", error))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| refusal("physical-screen-photo-unavailable", error))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_PHOTO_BYTES {
        return Err(refusal(
            "physical-screen-photo-invalid",
            format!(
                "{} must be a nonempty regular file of at most {MAX_PHOTO_BYTES} bytes",
                canonical.display()
            ),
        ));
    }
    let mut header = [0u8; 12];
    let mut file = fs::File::open(&canonical)
        .map_err(|error| refusal("physical-screen-photo-unavailable", error))?;
    let header_bytes = file
        .read(&mut header)
        .map_err(|error| refusal("physical-screen-photo-unavailable", error))?;
    let media_type = photo_media_type(&header[..header_bytes]).ok_or_else(|| {
        refusal(
            "physical-screen-photo-invalid",
            "retained screen evidence must be a JPEG, PNG, or WebP image",
        )
    })?;
    Ok(PhotoEvidence {
        path: canonical.display().to_string(),
        media_type,
        sha256: sha256_file(&canonical)?,
        bytes: metadata.len(),
    })
}

fn validate_identity(name: &str, value: &str) -> Result<(), ConduitosError> {
    validate_hex(name, value, 64)
}

fn validate_hex(name: &str, value: &str, length: usize) -> Result<(), ConduitosError> {
    if value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(refusal(
            "physical-screen-identity-invalid",
            format!("{name} must be exactly {length} lowercase hexadecimal characters"),
        ))
    }
}

fn validate_sha256(name: &str, value: &str) -> Result<(), ConduitosError> {
    validate_identity(name, value)
}

fn validate_prefixed_digest(name: &str, value: &str, prefix: &str) -> Result<(), ConduitosError> {
    let digest = value
        .strip_prefix(prefix)
        .ok_or_else(|| refusal("flash-record-invalid", format!("invalid {name}")))?;
    validate_sha256(name, digest)
        .map_err(|_| refusal("flash-record-invalid", format!("invalid {name}")))
}

fn photo_media_type(header: &[u8]) -> Option<&'static str> {
    if header.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if header.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if header.len() >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), ConduitosError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| refusal("physical-proof-write-failed", error))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| refusal("physical-proof-write-failed", error))?;
    file.write_all(bytes)
        .map_err(|error| refusal("physical-proof-write-failed", error))
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_lowercase_id_is_required() {
        assert!(validate_identity("id", &"a".repeat(64)).is_ok());
        assert!(validate_identity("id", &"A".repeat(64)).is_err());
        assert!(validate_identity("id", &"a".repeat(63)).is_err());
    }

    #[test]
    fn retained_evidence_requires_a_known_image_header() {
        assert_eq!(photo_media_type(b"\xff\xd8\xfffixture"), Some("image/jpeg"));
        assert_eq!(
            photo_media_type(b"\x89PNG\r\n\x1a\nfixture"),
            Some("image/png")
        );
        assert_eq!(photo_media_type(b"not a photo"), None);
    }

    #[test]
    fn completed_flash_record_still_cannot_claim_a_boot() {
        let mut record = FlashRecord {
            schema: "conduit.conduitos.ia32-live-flash/v2".into(),
            base_commit: "a".repeat(40),
            architecture: "ia32".into(),
            host: "conduitos/ia32/pc".into(),
            profile_id: format!("sha256:{}", "b".repeat(64)),
            build_id: format!("build:sha256:{}", "c".repeat(64)),
            image_id: format!("image:sha256:{}", "d".repeat(64)),
            image_sha256: "a".repeat(64),
            image_bytes: 1,
            image_packager: IMAGE_PACKAGER.into(),
            bootloader: BOOTLOADER.into(),
            bootloader_version: LIMINE_VERSION.into(),
            bootloader_archive_sha256: LIMINE_ARCHIVE_SHA256.into(),
            firmware_environment: FIRMWARE_ENVIRONMENT.into(),
            firmware_boot_entry: BIOS_BOOT_ENTRY.into(),
            carrier: "removable-whole-device".into(),
            device: "/dev/test".into(),
            removable: true,
            device_bytes: 2,
            write_completed: true,
            byte_verification_completed: true,
            physical_boot_claimed: true,
        };
        assert!(validate_flash_record(&record, &"a".repeat(64)).is_err());
        record.physical_boot_claimed = false;
        assert!(validate_flash_record(&record, &"a".repeat(64)).is_ok());
        record.firmware_boot_entry = "EFI/BOOT/BOOTIA32.EFI".into();
        assert!(validate_flash_record(&record, &"a".repeat(64)).is_err());
    }
}
