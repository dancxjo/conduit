use std::{fs, process::Command};

use serde::{Deserialize, Serialize};

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION},
    report::{git_head, sha256_file},
    ConduitosArch, ConduitosError,
};

const SCHEMA: &str = "conduit.conduitos.prepared-proof-image/v1";
const PROFILE_PATH: &str = "targets/conduitos/proof/profiles/conduitos-proof.profile.json";
const MAXIMUM_MANIFEST_BYTES: u64 = 4096;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct PreparedProofImage {
    schema: String,
    base_commit: String,
    architecture: String,
    artifact_role: String,
    profile_path: String,
    profile_sha256: String,
    limine_version: String,
    limine_archive_sha256: String,
    kernel_sha256: String,
    iso_sha256: String,
    build_record_sha256: String,
    image_record_sha256: String,
}

pub(super) fn prepare(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-prepared-image",
            "prepared proof image BUILD must retain exact immutable artifact bytes",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    require_clean_source(&paths)?;
    image::execute_proof(ConduitosArch::X86_64, opts)?;
    let record = record(&paths)?;
    let mut bytes = serde_json::to_vec_pretty(&record).map_err(|error| {
        ConduitosError::refusal("prepared-image-record-failed", error.to_string())
    })?;
    bytes.push(b'\n');
    fs::write(&paths.prepared_proof_image, bytes).map_err(|error| {
        ConduitosError::refusal("prepared-image-record-failed", error.to_string())
    })?;
    verify()?;
    if !opts.quiet && !opts.json {
        println!(
            "ConduitOS prepared proof image: {}",
            paths.prepared_proof_image.display()
        );
    }
    Ok(())
}

pub(super) fn ensure(prepared: bool, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if prepared {
        verify()
    } else {
        image::execute_proof(ConduitosArch::X86_64, opts).map(|_| ())
    }
}

fn verify() -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let metadata = fs::metadata(&paths.prepared_proof_image).map_err(|error| {
        ConduitosError::refusal("prepared-image-manifest-unavailable", error.to_string())
    })?;
    if metadata.len() > MAXIMUM_MANIFEST_BYTES {
        return Err(ConduitosError::refusal(
            "prepared-image-manifest-oversized",
            metadata.len().to_string(),
        ));
    }
    let bytes = fs::read(&paths.prepared_proof_image).map_err(|error| {
        ConduitosError::refusal("prepared-image-manifest-unavailable", error.to_string())
    })?;
    let actual: PreparedProofImage = serde_json::from_slice(&bytes).map_err(|error| {
        ConduitosError::refusal("prepared-image-manifest-invalid", error.to_string())
    })?;
    let expected = record(&paths)?;
    if actual != expected {
        return Err(ConduitosError::refusal(
            "prepared-image-identity-mismatch",
            "manifest, source, profile, build record, image record, kernel, or ISO identity changed",
        ));
    }
    Ok(())
}

fn record(paths: &Paths) -> Result<PreparedProofImage, ConduitosError> {
    Ok(PreparedProofImage {
        schema: SCHEMA.to_owned(),
        base_commit: git_head(&paths.root)?,
        architecture: ConduitosArch::X86_64.as_str().to_owned(),
        artifact_role: "architecture-proof-appliance".to_owned(),
        profile_path: PROFILE_PATH.to_owned(),
        profile_sha256: sha256_file(&paths.root.join(PROFILE_PATH))?,
        limine_version: LIMINE_VERSION.to_owned(),
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256.to_owned(),
        kernel_sha256: sha256_file(&paths.kernel)?,
        iso_sha256: sha256_file(&paths.iso)?,
        build_record_sha256: sha256_file(&paths.target.join("build.json"))?,
        image_record_sha256: sha256_file(&paths.target.join("image.json"))?,
    })
}

fn require_clean_source(paths: &Paths) -> Result<(), ConduitosError> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(&paths.root)
        .output()
        .map_err(|error| {
            ConduitosError::refusal("prepared-image-source-unavailable", error.to_string())
        })?;
    if !output.status.success() {
        return Err(ConduitosError::refusal(
            "prepared-image-source-unavailable",
            output.status.to_string(),
        ));
    }
    if !output.stdout.is_empty() {
        return Err(ConduitosError::refusal(
            "prepared-image-source-dirty",
            "prepared proof image requires an exact clean commit",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specimen() -> PreparedProofImage {
        PreparedProofImage {
            schema: SCHEMA.to_owned(),
            base_commit: "a".repeat(40),
            architecture: "x86_64".to_owned(),
            artifact_role: "architecture-proof-appliance".to_owned(),
            profile_path: PROFILE_PATH.to_owned(),
            profile_sha256: "b".repeat(64),
            limine_version: LIMINE_VERSION.to_owned(),
            limine_archive_sha256: LIMINE_ARCHIVE_SHA256.to_owned(),
            kernel_sha256: "c".repeat(64),
            iso_sha256: "d".repeat(64),
            build_record_sha256: "e".repeat(64),
            image_record_sha256: "f".repeat(64),
        }
    }

    #[test]
    fn every_identity_field_is_bound_by_exact_record_equality() {
        let expected = specimen();
        let mut changed: PreparedProofImage =
            serde_json::from_slice(&serde_json::to_vec(&expected).unwrap()).unwrap();
        assert_eq!(changed, expected);
        changed.iso_sha256 = "0".repeat(64);
        assert_ne!(changed, expected);
    }
}
