//! Native adapter for the bounded uncompressed Body-bound ZIP emitted by Crèche.

use conduit_host_fabrication::{
    BodyBoundArtifactIdentity, NativeInstallOutcome, NativeInstallRefusal, NativePackageInstaller,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const MAXIMUM_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const MAXIMUM_FILES: usize = 17;
const PROVISION_PATH: &str = "conduit-spore.json";
const LOCAL_FILE: u32 = 0x0403_4b50;
const CENTRAL_FILE: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

pub(crate) struct LocalNativeInstaller {
    state_dir: PathBuf,
}

impl LocalNativeInstaller {
    pub(crate) fn new(state_dir: PathBuf) -> Self {
        Self { state_dir }
    }
}

impl NativePackageInstaller for LocalNativeInstaller {
    fn install_and_start(
        &mut self,
        source: &Path,
        artifact: &BodyBoundArtifactIdentity,
    ) -> Result<NativeInstallOutcome, NativeInstallRefusal> {
        let prepared = prepare(source, artifact, &self.state_dir)?;
        let result = crate::durable_host::install_and_activate(&prepared.manifest, &self.state_dir);
        let cleanup = fs::remove_dir_all(&prepared.root);
        result.map_err(|_| NativeInstallRefusal::InstallationFailed)?;
        cleanup.map_err(|_| NativeInstallRefusal::InstallationFailed)?;
        Ok(NativeInstallOutcome {
            package_bytes_consumed: artifact.artifact_bytes,
            byte_verification_completed: true,
            start_requested: true,
        })
    }
}

#[derive(Debug)]
struct PreparedPackage {
    root: PathBuf,
    manifest: PathBuf,
}

#[derive(Deserialize)]
struct Provision {
    schema: String,
    spore: NativeSpore,
    invitation_provision: InvitationProvision,
}

#[derive(Deserialize)]
struct NativeSpore {
    schema: String,
    spore_id: String,
    body_id: String,
    binding: SporeBinding,
    image_id: String,
    image_content_digest: String,
    target: String,
    output: String,
    fabrication: FabricationSelection,
    source_identity: String,
}

#[derive(Deserialize)]
struct SporeBinding {
    mode: String,
    invitation_id: String,
}

#[derive(Deserialize)]
struct FabricationSelection {
    fabrication_package_id: String,
    builder_adapter: String,
    deployment_adapter: Option<String>,
}

#[derive(Deserialize)]
struct InvitationProvision {
    invitation_id: String,
    nonce: Vec<u8>,
    secret: Vec<u8>,
    expires_at_millis: u64,
}

#[derive(Serialize)]
struct ReleaseManifest<'a> {
    schema: &'static str,
    target_id: &'a str,
    fabrication_package_id: &'a str,
    output: &'static str,
    builder_adapter: &'a str,
    deployment_adapter: &'a str,
    source_identity: &'a str,
    bundle_sha256: &'a str,
    files: Vec<ReleaseFile>,
}

#[derive(Serialize)]
struct ReleaseFile {
    path: String,
    bytes: u64,
    sha256: String,
}

struct ZipEntry {
    name: String,
    bytes: Vec<u8>,
}

fn prepare(
    source: &Path,
    artifact: &BodyBoundArtifactIdentity,
    state_dir: &Path,
) -> Result<PreparedPackage, NativeInstallRefusal> {
    let package = fs::read(source).map_err(|_| NativeInstallRefusal::SourceUnavailable)?;
    if package.is_empty()
        || package.len() > MAXIMUM_PACKAGE_BYTES
        || package.len() as u64 != artifact.artifact_bytes
        || digest(&package) != artifact.artifact_content_sha256
    {
        return Err(NativeInstallRefusal::ContentMismatch);
    }
    let mut entries =
        read_stored_zip(&package).map_err(|_| NativeInstallRefusal::PackageMalformed)?;
    let provision_index = entries
        .iter()
        .position(|entry| entry.name == PROVISION_PATH)
        .ok_or(NativeInstallRefusal::PackageMalformed)?;
    let mut provision: Provision = serde_json::from_slice(&entries.remove(provision_index).bytes)
        .map_err(|_| NativeInstallRefusal::PackageMalformed)?;
    validate_provision(&provision, artifact).map_err(|_| NativeInstallRefusal::BindingMismatch)?;
    provision.invitation_provision.secret.fill(0);
    if entries.is_empty() || entries.len() >= MAXIMUM_FILES {
        return Err(NativeInstallRefusal::PackageMalformed);
    }
    let files = entries
        .iter()
        .map(|entry| ReleaseFile {
            path: entry.name.clone(),
            bytes: entry.bytes.len() as u64,
            sha256: digest(&entry.bytes),
        })
        .collect::<Vec<_>>();
    if bundle_digest(&files) != artifact.image_content_sha256 {
        return Err(NativeInstallRefusal::ImageMismatch);
    }
    let parent = state_dir.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|_| NativeInstallRefusal::InstallationFailed)?;
    let root = parent.join(format!(
        ".conduit-native-{}-{}",
        std::process::id(),
        &artifact.artifact_content_sha256[7..23]
    ));
    fs::create_dir(&root).map_err(|_| NativeInstallRefusal::InstallationFailed)?;
    let result = (|| {
        for entry in entries {
            create_new(&root.join(&entry.name), &entry.bytes)
                .map_err(|_| NativeInstallRefusal::InstallationFailed)?;
        }
        let deployment = provision
            .spore
            .fabrication
            .deployment_adapter
            .as_deref()
            .ok_or(NativeInstallRefusal::BindingMismatch)?;
        let manifest = ReleaseManifest {
            schema: "conduit.release/host-bundle@1",
            target_id: &provision.spore.target,
            fabrication_package_id: &provision.spore.fabrication.fabrication_package_id,
            output: "native-bundle",
            builder_adapter: &provision.spore.fabrication.builder_adapter,
            deployment_adapter: deployment,
            source_identity: &provision.spore.source_identity,
            bundle_sha256: &artifact.image_content_sha256,
            files,
        };
        let manifest_path = root.join("body-bound-native-release.json");
        create_new(
            &manifest_path,
            &serde_json::to_vec_pretty(&manifest)
                .map_err(|_| NativeInstallRefusal::InstallationFailed)?,
        )
        .map_err(|_| NativeInstallRefusal::InstallationFailed)?;
        Ok(PreparedPackage {
            root: root.clone(),
            manifest: manifest_path,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&root);
    }
    result
}

fn validate_provision(
    provision: &Provision,
    artifact: &BodyBoundArtifactIdentity,
) -> Result<(), String> {
    let spore = &provision.spore;
    let invitation = &provision.invitation_provision;
    if provision.schema != "conduit.spore/native-package-provision@1"
        || spore.schema != "conduit.body/spore-manifest@2"
        || spore.spore_id != artifact.spore_id
        || spore.image_id != artifact.image_id
        || spore.image_content_digest != artifact.image_content_sha256
        || spore.target != artifact.target_id
        || spore.output != "native-bundle"
        || spore.body_id.is_empty()
        || spore.binding.mode != "self-joining"
        || spore.binding.invitation_id != invitation.invitation_id
        || invitation.nonce.len() != 32
        || invitation.nonce.iter().all(|byte| *byte == 0)
        || invitation.secret.len() != 32
        || invitation.secret.iter().all(|byte| *byte == 0)
        || invitation.expires_at_millis == 0
        || spore.source_identity.is_empty()
    {
        return Err(
            "native package provision lost its exact Body, spore, or IMAGE identity".into(),
        );
    }
    Ok(())
}

fn read_stored_zip(bytes: &[u8]) -> Result<Vec<ZipEntry>, String> {
    if bytes.len() < 22 || u32_at(bytes, bytes.len() - 22)? != END {
        return Err("native ZIP has no exact terminal directory".into());
    }
    let end = bytes.len() - 22;
    let count = usize::from(u16_at(bytes, end + 10)?);
    let central_bytes = usize_at_u32(bytes, end + 12)?;
    let central_offset = usize_at_u32(bytes, end + 16)?;
    if u16_at(bytes, end + 4)? != 0
        || u16_at(bytes, end + 6)? != 0
        || usize::from(u16_at(bytes, end + 8)?) != count
        || u16_at(bytes, end + 20)? != 0
        || !(2..=MAXIMUM_FILES).contains(&count)
        || central_offset.checked_add(central_bytes) != Some(end)
    {
        return Err("native ZIP terminal directory is malformed or unbounded".into());
    }
    let mut cursor = central_offset;
    let mut expected_local = 0_usize;
    let mut names = BTreeSet::new();
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(bytes, cursor)? != CENTRAL_FILE {
            return Err("native ZIP central entry is malformed".into());
        }
        let flags = u16_at(bytes, cursor + 8)?;
        let method = u16_at(bytes, cursor + 10)?;
        let expected_crc = u32_at(bytes, cursor + 16)?;
        let compressed = usize_at_u32(bytes, cursor + 20)?;
        let size = usize_at_u32(bytes, cursor + 24)?;
        let name_len = usize::from(u16_at(bytes, cursor + 28)?);
        let extra_len = usize::from(u16_at(bytes, cursor + 30)?);
        let comment_len = usize::from(u16_at(bytes, cursor + 32)?);
        let local_offset = usize_at_u32(bytes, cursor + 42)?;
        let name = name_at(bytes, cursor + 46, name_len)?;
        cursor = cursor
            .checked_add(46 + name_len + extra_len + comment_len)
            .ok_or("native ZIP central extent overflow")?;
        if flags != 0x0800
            || method != 0
            || compressed != size
            || local_offset != expected_local
            || !names.insert(name.clone())
            || u32_at(bytes, local_offset)? != LOCAL_FILE
            || u16_at(bytes, local_offset + 6)? != flags
            || u16_at(bytes, local_offset + 8)? != method
            || u32_at(bytes, local_offset + 14)? != expected_crc
            || usize_at_u32(bytes, local_offset + 18)? != size
            || usize_at_u32(bytes, local_offset + 22)? != size
        {
            return Err("native ZIP entry layout is inconsistent".into());
        }
        let local_name_len = usize::from(u16_at(bytes, local_offset + 26)?);
        let local_extra_len = usize::from(u16_at(bytes, local_offset + 28)?);
        if name_at(bytes, local_offset + 30, local_name_len)? != name {
            return Err("native ZIP local name differs from its central entry".into());
        }
        let data_offset = local_offset
            .checked_add(30 + local_name_len + local_extra_len)
            .ok_or("native ZIP local extent overflow")?;
        let data_end = data_offset
            .checked_add(size)
            .filter(|end| *end <= central_offset)
            .ok_or("native ZIP payload exceeds its local region")?;
        let payload = &bytes[data_offset..data_end];
        if crc32(payload) != expected_crc {
            return Err("native ZIP entry failed CRC-32".into());
        }
        entries.push(ZipEntry {
            name,
            bytes: payload.to_vec(),
        });
        expected_local = data_end;
    }
    if cursor != end || expected_local != central_offset {
        return Err("native ZIP has unaccounted bytes".into());
    }
    Ok(entries)
}

fn name_at(bytes: &[u8], offset: usize, length: usize) -> Result<String, String> {
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or("native ZIP entry name exceeds the package")?;
    let name = std::str::from_utf8(&bytes[offset..end])
        .map_err(|_| "native ZIP entry name is not UTF-8")?;
    if name.is_empty()
        || name.len() > 256
        || name.contains(['/', '\\'])
        || matches!(name, "." | "..")
    {
        return Err("native ZIP entry name is unsafe".into());
    }
    Ok(name.into())
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or("native ZIP field exceeds the package")?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or("native ZIP field exceeds the package")?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn usize_at_u32(bytes: &[u8], offset: usize) -> Result<usize, String> {
    usize::try_from(u32_at(bytes, offset)?).map_err(|_| "native ZIP extent is unsupported".into())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn create_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("create {}: {error}", path.display()))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn bundle_digest(files: &[ReleaseFile]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"conduit.release/host-bundle-content@1\0");
    for file in files {
        digest.update(file.path.as_bytes());
        digest.update(b"\0");
        digest.update(file.sha256.as_bytes());
        digest.update(b"\n");
    }
    format!("sha256:{:x}", digest.finalize())
}

#[cfg(test)]
#[path = "native_package_install_tests.rs"]
mod tests;
