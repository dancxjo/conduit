//! Bounded, deterministic package envelopes over distinct Conduit objects.
//!
//! The binary envelope is deliberately pathless. Decoding, validation, and
//! extraction never load, execute, fetch, or otherwise interpret object bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use conduit_core::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, EXECUTION_PLAN_SCHEMA_VERSION,
    IMPLEMENTATION_MANIFEST_SCHEMA_VERSION, Id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const PACKAGE_SCHEMA: &str = "conduit.package/v1";
pub const PACKAGE_SCHEMA_VERSION: u16 = 1;
pub const PACKAGE_MAGIC: &[u8; 8] = b"CNDPKG1\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageLimits {
    pub maximum_package_bytes: u64,
    pub maximum_manifest_bytes: u32,
    pub maximum_objects: u32,
    pub maximum_object_bytes: u64,
    pub maximum_extracted_bytes: u64,
}

impl Default for PackageLimits {
    fn default() -> Self {
        Self {
            maximum_package_bytes: 256 * 1024 * 1024,
            maximum_manifest_bytes: 1024 * 1024,
            maximum_objects: 4096,
            maximum_object_bytes: 128 * 1024 * 1024,
            maximum_extracted_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageObjectIdentity {
    pub kind: String,
    pub schema_version: u32,
    pub semantic_identity: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageProvenance {
    pub builder: String,
    pub source_digest: String,
    pub build_recipe_digest: String,
    pub reproducible: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageObject {
    pub digest: String,
    pub media_type: String,
    pub byte_size: u64,
    pub role: String,
    pub embedded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<PackageObjectIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub license_expressions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub license_objects: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sbom: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attestations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<PackageProvenance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retrieval_hints: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    pub schema: String,
    pub schema_version: u16,
    pub identity: String,
    pub objects: Vec<PackageObject>,
}

#[derive(Serialize)]
struct IdentityProjection<'a> {
    schema: &'a str,
    schema_version: u16,
    objects: &'a [PackageObject],
}

impl PackageManifest {
    #[must_use]
    pub fn new(objects: Vec<PackageObject>) -> Self {
        Self {
            schema: PACKAGE_SCHEMA.to_owned(),
            schema_version: PACKAGE_SCHEMA_VERSION,
            identity: String::new(),
            objects,
        }
    }

    pub fn seal(&mut self) -> Result<(), PackageError> {
        canonicalize_objects(&mut self.objects);
        self.identity = self.computed_identity()?;
        self.validate(PackageLimits::default())
    }

    pub fn computed_identity(&self) -> Result<String, PackageError> {
        let mut objects = self.objects.clone();
        canonicalize_objects(&mut objects);
        let projection = IdentityProjection {
            schema: &self.schema,
            schema_version: self.schema_version,
            objects: &objects,
        };
        let bytes = serde_json::to_vec(&projection)
            .map_err(|_| PackageError::new(PackageReason::MalformedManifest))?;
        Ok(format!("sha256:{}", hex(&Sha256::digest(bytes))))
    }

    pub fn validate(&self, limits: PackageLimits) -> Result<(), PackageError> {
        if self.schema != PACKAGE_SCHEMA || self.schema_version != PACKAGE_SCHEMA_VERSION {
            return Err(PackageError::new(PackageReason::UnsupportedVersion));
        }
        if self.objects.len() > limits.maximum_objects as usize {
            return Err(PackageError::new(PackageReason::LimitExceeded));
        }
        if self.identity != self.computed_identity()? {
            return Err(PackageError::new(PackageReason::IdentityMismatch));
        }
        let mut digests = BTreeSet::new();
        for object in &self.objects {
            validate_object(object, limits)?;
            if !digests.insert(object.digest.as_str()) {
                return Err(PackageError::new(PackageReason::MalformedManifest));
            }
        }
        for object in &self.objects {
            for digest in &object.license_objects {
                require_role(self, digest, &["license"])?;
            }
            if let Some(digest) = object.sbom.as_deref() {
                require_role(self, digest, &["sbom"])?;
            }
            for digest in &object.signatures {
                require_role(self, digest, &["signature"])?;
            }
            for digest in &object.attestations {
                require_role(self, digest, &["attestation"])?;
            }
            if let Some(provenance) = &object.provenance {
                validate_id(&provenance.builder)?;
                require_digest(self, &provenance.source_digest)?;
                require_digest(self, &provenance.build_recipe_digest)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedPackage {
    pub manifest: PackageManifest,
    pub embedded_blobs: BTreeMap<String, Vec<u8>>,
}

impl ValidatedPackage {
    #[must_use]
    pub fn embedded_bytes(&self) -> u64 {
        self.embedded_blobs
            .values()
            .map(|blob| blob.len() as u64)
            .sum()
    }

    pub fn extract_to(
        &self,
        output: &Path,
        limits: PackageLimits,
    ) -> Result<Vec<PathBuf>, PackageError> {
        self.manifest.validate(limits)?;
        if self.embedded_bytes() > limits.maximum_extracted_bytes {
            return Err(PackageError::new(PackageReason::LimitExceeded));
        }
        fs::create_dir_all(output).map_err(|_| PackageError::new(PackageReason::UnsafeTarget))?;
        let root = output
            .canonicalize()
            .map_err(|_| PackageError::new(PackageReason::UnsafeTarget))?;
        let blobs = root.join("blobs");
        let sha256 = blobs.join("sha256");
        create_plain_directory(&blobs)?;
        create_plain_directory(&sha256)?;

        let mut extracted = Vec::with_capacity(self.embedded_blobs.len());
        for (digest, bytes) in &self.embedded_blobs {
            let (_, digest_hex) = digest
                .split_once(':')
                .ok_or_else(|| PackageError::new(PackageReason::MalformedManifest))?;
            let path = sha256.join(digest_hex);
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|_| PackageError::new(PackageReason::UnsafeTarget))?;
            file.write_all(bytes)
                .map_err(|_| PackageError::new(PackageReason::UnsafeTarget))?;
            file.sync_all()
                .map_err(|_| PackageError::new(PackageReason::UnsafeTarget))?;
            extracted.push(path);
        }
        Ok(extracted)
    }
}

pub fn encode_package(
    manifest: &PackageManifest,
    blobs: &BTreeMap<String, Vec<u8>>,
    limits: PackageLimits,
) -> Result<Vec<u8>, PackageError> {
    manifest.validate(limits)?;
    validate_blob_set(manifest, blobs, limits)?;
    let manifest_bytes = serde_json::to_vec(manifest)
        .map_err(|_| PackageError::new(PackageReason::MalformedManifest))?;
    if manifest_bytes.len() > limits.maximum_manifest_bytes as usize {
        return Err(PackageError::new(PackageReason::LimitExceeded));
    }
    let mut total = PACKAGE_MAGIC.len() as u64;
    total = checked_add(total, 4)?;
    total = checked_add(total, manifest_bytes.len() as u64)?;
    total = checked_add(total, 4)?;
    for bytes in blobs.values() {
        total = checked_add(total, 32 + 8)?;
        total = checked_add(total, bytes.len() as u64)?;
    }
    if total > limits.maximum_package_bytes || total > usize::MAX as u64 {
        return Err(PackageError::new(PackageReason::LimitExceeded));
    }

    let manifest_length = u32::try_from(manifest_bytes.len())
        .map_err(|_| PackageError::new(PackageReason::LimitExceeded))?;
    let entry_count =
        u32::try_from(blobs.len()).map_err(|_| PackageError::new(PackageReason::LimitExceeded))?;
    let mut output = Vec::with_capacity(total as usize);
    output.extend_from_slice(PACKAGE_MAGIC);
    output.extend_from_slice(&manifest_length.to_be_bytes());
    output.extend_from_slice(&manifest_bytes);
    output.extend_from_slice(&entry_count.to_be_bytes());
    for (digest, bytes) in blobs {
        output.extend_from_slice(&parse_digest(digest)?);
        output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        output.extend_from_slice(bytes);
    }
    Ok(output)
}

pub fn decode_package(
    bytes: &[u8],
    limits: PackageLimits,
) -> Result<ValidatedPackage, PackageError> {
    if bytes.len() as u64 > limits.maximum_package_bytes {
        return Err(PackageError::new(PackageReason::LimitExceeded));
    }
    let mut cursor = 0usize;
    if take(bytes, &mut cursor, PACKAGE_MAGIC.len())? != PACKAGE_MAGIC {
        return Err(PackageError::new(PackageReason::MalformedEnvelope));
    }
    let manifest_length = read_u32(bytes, &mut cursor)?;
    if manifest_length > limits.maximum_manifest_bytes {
        return Err(PackageError::new(PackageReason::LimitExceeded));
    }
    let manifest_bytes = take(bytes, &mut cursor, manifest_length as usize)?;
    let manifest: PackageManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|_| PackageError::new(PackageReason::MalformedManifest))?;
    manifest.validate(limits)?;
    let entry_count = read_u32(bytes, &mut cursor)?;
    if entry_count > limits.maximum_objects {
        return Err(PackageError::new(PackageReason::LimitExceeded));
    }
    let mut blobs = BTreeMap::new();
    let mut aggregate = 0u64;
    for _ in 0..entry_count {
        let digest = format!("sha256:{}", hex(take(bytes, &mut cursor, 32)?));
        let byte_size = read_u64(bytes, &mut cursor)?;
        if byte_size == 0 || byte_size > limits.maximum_object_bytes {
            return Err(PackageError::new(PackageReason::LimitExceeded));
        }
        aggregate = checked_add(aggregate, byte_size)?;
        if aggregate > limits.maximum_extracted_bytes || byte_size > usize::MAX as u64 {
            return Err(PackageError::new(PackageReason::LimitExceeded));
        }
        let blob = take(bytes, &mut cursor, byte_size as usize)?.to_vec();
        if blobs.insert(digest, blob).is_some() {
            return Err(PackageError::new(PackageReason::MalformedManifest));
        }
    }
    if cursor != bytes.len() {
        return Err(PackageError::new(PackageReason::MalformedEnvelope));
    }
    validate_blob_set(&manifest, &blobs, limits)?;
    Ok(ValidatedPackage {
        manifest,
        embedded_blobs: blobs,
    })
}

fn validate_blob_set(
    manifest: &PackageManifest,
    blobs: &BTreeMap<String, Vec<u8>>,
    limits: PackageLimits,
) -> Result<(), PackageError> {
    let expected = manifest
        .objects
        .iter()
        .filter(|object| object.embedded)
        .map(|object| object.digest.as_str())
        .collect::<BTreeSet<_>>();
    if expected.len() != blobs.len()
        || blobs
            .keys()
            .any(|digest| !expected.contains(digest.as_str()))
    {
        return Err(PackageError::new(PackageReason::MissingBlob));
    }
    let mut aggregate = 0u64;
    for object in manifest.objects.iter().filter(|object| object.embedded) {
        let bytes = blobs
            .get(&object.digest)
            .ok_or_else(|| PackageError::new(PackageReason::MissingBlob))?;
        if bytes.len() as u64 != object.byte_size
            || format!("sha256:{}", hex(&Sha256::digest(bytes))) != object.digest
        {
            return Err(PackageError::new(PackageReason::BlobMismatch));
        }
        aggregate = checked_add(aggregate, bytes.len() as u64)?;
    }
    if aggregate > limits.maximum_extracted_bytes {
        return Err(PackageError::new(PackageReason::LimitExceeded));
    }
    Ok(())
}

fn validate_object(object: &PackageObject, limits: PackageLimits) -> Result<(), PackageError> {
    parse_digest(&object.digest)?;
    if object.byte_size == 0 || object.byte_size > limits.maximum_object_bytes {
        return Err(PackageError::new(PackageReason::LimitExceeded));
    }
    validate_id(&object.role)?;
    validate_media_type(object)?;
    if !object.embedded && object.retrieval_hints.is_empty() {
        return Err(PackageError::new(PackageReason::MalformedManifest));
    }
    for hint in &object.retrieval_hints {
        if hint.is_empty() || hint.len() > 2048 || hint.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(PackageError::new(PackageReason::MalformedManifest));
        }
    }
    for expression in &object.license_expressions {
        if expression.is_empty()
            || expression.len() > 256
            || expression.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(PackageError::new(PackageReason::MetadataMismatch));
        }
    }
    if let Some(identity) = &object.identity {
        validate_id(&identity.kind)?;
        parse_sha256(&identity.semantic_identity)?;
        match identity.kind.as_str() {
            "execution-plan" if identity.schema_version > EXECUTION_PLAN_SCHEMA_VERSION => {
                return Err(PackageError::new(PackageReason::UnsupportedVersion));
            }
            "implementation-manifest"
                if identity.schema_version != IMPLEMENTATION_MANIFEST_SCHEMA_VERSION =>
            {
                return Err(PackageError::new(PackageReason::UnsupportedVersion));
            }
            "artifact-manifest" if identity.schema_version != ARTIFACT_MANIFEST_SCHEMA_VERSION => {
                return Err(PackageError::new(PackageReason::UnsupportedVersion));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_media_type(object: &PackageObject) -> Result<(), PackageError> {
    if object.media_type.is_empty()
        || object.media_type.len() > 256
        || object
            .media_type
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        || !object
            .media_type
            .split_once('/')
            .is_some_and(|(top, rest)| !top.is_empty() && !rest.is_empty())
    {
        return Err(PackageError::new(PackageReason::MalformedManifest));
    }
    if object.media_type.starts_with("application/vnd.conduit.") {
        let Some((base, version)) = object.media_type.rsplit_once(";version=") else {
            return Err(PackageError::new(PackageReason::UnsupportedVersion));
        };
        let version = version
            .parse::<u32>()
            .map_err(|_| PackageError::new(PackageReason::UnsupportedVersion))?;
        let supported = match base {
            "application/vnd.conduit.execution-plan+json" => {
                (1..=EXECUTION_PLAN_SCHEMA_VERSION).contains(&version)
            }
            "application/vnd.conduit.implementation-manifest+json"
            | "application/vnd.conduit.artifact-manifest+json"
            | "application/vnd.conduit.module-lock+json"
            | "application/vnd.conduit.semantic-descriptor+json" => version == 1,
            _ => false,
        };
        if !supported {
            return Err(PackageError::new(PackageReason::UnsupportedVersion));
        }
    }
    Ok(())
}

fn canonicalize_objects(objects: &mut [PackageObject]) {
    for object in objects.iter_mut() {
        object.license_expressions.sort();
        object.license_objects.sort();
        object.signatures.sort();
        object.attestations.sort();
        object.retrieval_hints.sort();
    }
    objects.sort_by(|left, right| left.digest.cmp(&right.digest));
}

fn require_role(
    manifest: &PackageManifest,
    digest: &str,
    roles: &[&str],
) -> Result<(), PackageError> {
    let object = manifest
        .objects
        .iter()
        .find(|object| object.digest == digest)
        .ok_or_else(|| PackageError::new(PackageReason::MetadataMismatch))?;
    if roles.contains(&object.role.as_str()) {
        Ok(())
    } else {
        Err(PackageError::new(PackageReason::MetadataMismatch))
    }
}

fn require_digest(manifest: &PackageManifest, digest: &str) -> Result<(), PackageError> {
    parse_digest(digest)?;
    manifest
        .objects
        .iter()
        .any(|object| object.digest == digest)
        .then_some(())
        .ok_or_else(|| PackageError::new(PackageReason::MetadataMismatch))
}

fn validate_id(value: &str) -> Result<(), PackageError> {
    Id::new(value)
        .map(|_| ())
        .map_err(|_| PackageError::new(PackageReason::MalformedManifest))
}

fn parse_sha256(value: &str) -> Result<[u8; 32], PackageError> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or_else(|| PackageError::new(PackageReason::MalformedManifest))?;
    if hex.len() != 64 {
        return Err(PackageError::new(PackageReason::MalformedManifest));
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(bytes)
}

fn parse_digest(value: &str) -> Result<[u8; 32], PackageError> {
    parse_sha256(value)
}

fn nibble(byte: u8) -> Result<u8, PackageError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(PackageError::new(PackageReason::MalformedManifest)),
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], PackageError> {
    let end = cursor
        .checked_add(length)
        .ok_or_else(|| PackageError::new(PackageReason::LimitExceeded))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| PackageError::new(PackageReason::MalformedEnvelope))?;
    *cursor = end;
    Ok(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, PackageError> {
    let bytes: [u8; 4] = take(bytes, cursor, 4)?
        .try_into()
        .expect("slice length is exact");
    Ok(u32::from_be_bytes(bytes))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, PackageError> {
    let bytes: [u8; 8] = take(bytes, cursor, 8)?
        .try_into()
        .expect("slice length is exact");
    Ok(u64::from_be_bytes(bytes))
}

fn checked_add(left: u64, right: u64) -> Result<u64, PackageError> {
    left.checked_add(right)
        .ok_or_else(|| PackageError::new(PackageReason::LimitExceeded))
}

fn create_plain_directory(path: &Path) -> Result<(), PackageError> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| PackageError::new(PackageReason::UnsafeTarget))?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(PackageError::new(PackageReason::UnsafeTarget));
        }
    } else {
        fs::create_dir(path).map_err(|_| PackageError::new(PackageReason::UnsafeTarget))?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageReason {
    UnsupportedVersion,
    IdentityMismatch,
    MalformedManifest,
    MissingBlob,
    BlobMismatch,
    MetadataMismatch,
    LimitExceeded,
    MalformedEnvelope,
    UnsafeTarget,
}

impl PackageReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-PKG-001",
            Self::IdentityMismatch => "CND-PKG-002",
            Self::MalformedManifest => "CND-PKG-003",
            Self::MissingBlob => "CND-PKG-004",
            Self::BlobMismatch => "CND-PKG-005",
            Self::MetadataMismatch => "CND-PKG-006",
            Self::LimitExceeded => "CND-PKG-007",
            Self::MalformedEnvelope => "CND-PKG-008",
            Self::UnsafeTarget => "CND-PKG-009",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageError {
    pub reason: PackageReason,
}

impl PackageError {
    const fn new(reason: PackageReason) -> Self {
        Self { reason }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.reason.code()
    }
}

impl fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.reason {
            PackageReason::UnsupportedVersion => "unsupported package or Conduit media version",
            PackageReason::IdentityMismatch => "package manifest identity mismatch",
            PackageReason::MalformedManifest => "malformed or duplicate package object metadata",
            PackageReason::MissingBlob => "missing or unexpected embedded package blob",
            PackageReason::BlobMismatch => "package blob digest or size mismatch",
            PackageReason::MetadataMismatch => {
                "license, SBOM, signature, attestation, or provenance mismatch"
            }
            PackageReason::LimitExceeded => "package or extraction limit exceeded",
            PackageReason::MalformedEnvelope => "malformed, truncated, or trailing package bytes",
            PackageReason::UnsafeTarget => "unsafe or conflicting extraction target",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for PackageError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn digest(bytes: &[u8]) -> String {
        format!("sha256:{}", hex(&Sha256::digest(bytes)))
    }

    fn object(role: &str, bytes: &[u8], embedded: bool) -> PackageObject {
        PackageObject {
            digest: digest(bytes),
            media_type: "application/octet-stream".to_owned(),
            byte_size: bytes.len() as u64,
            role: role.to_owned(),
            embedded,
            identity: None,
            license_expressions: Vec::new(),
            license_objects: Vec::new(),
            sbom: None,
            signatures: Vec::new(),
            attestations: Vec::new(),
            provenance: None,
            retrieval_hints: if embedded {
                Vec::new()
            } else {
                vec!["https://example.invalid/content".to_owned()]
            },
        }
    }

    #[test]
    fn thick_and_thin_packages_round_trip_deterministically() {
        let linux = b"linux-native".to_vec();
        let wasm = b"wasm-component".to_vec();
        let pico = b"pico-firmware".to_vec();
        let mut manifest = PackageManifest::new(vec![
            object("embedded-firmware", &pico, true),
            object("linux-native", &linux, true),
            object("wasm-component", &wasm, false),
        ]);
        manifest.seal().unwrap();
        let blobs = BTreeMap::from([(digest(&linux), linux), (digest(&pico), pico)]);
        let first = encode_package(&manifest, &blobs, PackageLimits::default()).unwrap();
        let second = encode_package(&manifest, &blobs, PackageLimits::default()).unwrap();
        assert_eq!(first, second);
        let decoded = decode_package(&first, PackageLimits::default()).unwrap();
        assert_eq!(decoded.manifest, manifest);
        assert_eq!(decoded.embedded_blobs, blobs);
    }

    #[test]
    fn tamper_truncation_trailing_and_limits_fail_closed() {
        let bytes = b"payload".to_vec();
        let mut manifest = PackageManifest::new(vec![object("linux-native", &bytes, true)]);
        manifest.seal().unwrap();
        let blobs = BTreeMap::from([(digest(&bytes), bytes)]);
        let package = encode_package(&manifest, &blobs, PackageLimits::default()).unwrap();

        let mut tampered = package.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert_eq!(
            decode_package(&tampered, PackageLimits::default())
                .unwrap_err()
                .code(),
            "CND-PKG-005"
        );
        assert_eq!(
            decode_package(&package[..package.len() - 1], PackageLimits::default())
                .unwrap_err()
                .code(),
            "CND-PKG-008"
        );
        let mut trailing = package.clone();
        trailing.push(0);
        assert_eq!(
            decode_package(&trailing, PackageLimits::default())
                .unwrap_err()
                .code(),
            "CND-PKG-008"
        );
        let limits = PackageLimits {
            maximum_package_bytes: 4,
            ..PackageLimits::default()
        };
        assert_eq!(
            decode_package(&package, limits).unwrap_err().code(),
            "CND-PKG-007"
        );
    }

    #[test]
    fn metadata_roles_and_versions_are_enforced() {
        let bytes = b"plan";
        let mut plan = object("exact-plan", bytes, false);
        plan.media_type = "application/vnd.conduit.execution-plan+json;version=999".to_owned();
        plan.identity = Some(PackageObjectIdentity {
            kind: "execution-plan".to_owned(),
            schema_version: EXECUTION_PLAN_SCHEMA_VERSION + 1,
            semantic_identity: digest(b"semantic"),
        });
        let mut manifest = PackageManifest::new(vec![plan]);
        canonicalize_objects(&mut manifest.objects);
        manifest.identity = manifest.computed_identity().unwrap();
        assert_eq!(
            manifest
                .validate(PackageLimits::default())
                .unwrap_err()
                .code(),
            "CND-PKG-001"
        );

        let mut payload = object("linux-native", b"payload", false);
        payload.sbom = Some(digest(b"missing"));
        let mut manifest = PackageManifest::new(vec![payload]);
        canonicalize_objects(&mut manifest.objects);
        manifest.identity = manifest.computed_identity().unwrap();
        assert_eq!(
            manifest
                .validate(PackageLimits::default())
                .unwrap_err()
                .code(),
            "CND-PKG-006"
        );
    }

    #[test]
    fn extraction_uses_only_digest_paths_and_refuses_conflicts() {
        let bytes = b"#!/bin/sh\nexit 99\n".to_vec();
        let digest = digest(&bytes);
        let mut manifest = PackageManifest::new(vec![object("linux-native", &bytes, true)]);
        manifest.seal().unwrap();
        let package = encode_package(
            &manifest,
            &BTreeMap::from([(digest.clone(), bytes)]),
            PackageLimits::default(),
        )
        .unwrap();
        let decoded = decode_package(&package, PackageLimits::default()).unwrap();
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "conduit-package-extract-{}-{sequence}",
            std::process::id()
        ));
        let paths = decoded.extract_to(&root, PackageLimits::default()).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0].file_name().unwrap().to_string_lossy(),
            digest.strip_prefix("sha256:").unwrap()
        );
        assert_eq!(
            decoded
                .extract_to(&root, PackageLimits::default())
                .unwrap_err()
                .code(),
            "CND-PKG-009"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
