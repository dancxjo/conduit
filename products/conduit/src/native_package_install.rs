//! Native adapter for the bounded uncompressed body-bound ZIP emitted by Crèche.

use conduit_body_make::read_stored_joining_zip as read_stored_zip;
#[cfg(test)]
use conduit_body_make::JoiningZipEntry as ZipEntry;
use conduit_host_make::{
    BodyBoundArtifactIdentity, NativeInstallOutcome, NativeInstallRefusal, NativePackageInstaller,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const MAXIMUM_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const MAXIMUM_FILES: usize = 17;
const PROVISION_PATH: &str = "conduit-spore.json";
#[cfg(test)]
const LOCAL_FILE: u32 = 0x0403_4b50;
#[cfg(test)]
const CENTRAL_FILE: u32 = 0x0201_4b50;
#[cfg(test)]
const END: u32 = 0x0605_4b50;

pub(crate) struct LocalNativeInstaller {
    state_dir: PathBuf,
    installed_host: Option<crate::durable_host::InstalledHostIdentity>,
    #[cfg(test)]
    suppress_service_manager: bool,
}

impl LocalNativeInstaller {
    pub(crate) fn new(state_dir: PathBuf) -> Self {
        Self {
            state_dir,
            installed_host: None,
            #[cfg(test)]
            suppress_service_manager: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_without_service_manager(state_dir: PathBuf) -> Self {
        Self {
            state_dir,
            installed_host: None,
            suppress_service_manager: true,
        }
    }

    pub(crate) fn installed_host(&self) -> Option<&crate::durable_host::InstalledHostIdentity> {
        self.installed_host.as_ref()
    }
}

impl NativePackageInstaller for LocalNativeInstaller {
    fn install_and_start(
        &mut self,
        source: &Path,
        artifact: &BodyBoundArtifactIdentity,
    ) -> Result<NativeInstallOutcome, NativeInstallRefusal> {
        let prepared = prepare(source, artifact, &self.state_dir)?;
        #[cfg(test)]
        let result = if self.suppress_service_manager {
            crate::durable_host::install_without_start(&prepared.manifest, &self.state_dir)
        } else {
            crate::durable_host::install_and_activate(&prepared.manifest, &self.state_dir)
        };
        #[cfg(not(test))]
        let result = crate::durable_host::install_and_activate(&prepared.manifest, &self.state_dir);
        let cleanup = fs::remove_dir_all(&prepared.root);
        self.installed_host = Some(result.map_err(|_| NativeInstallRefusal::InstallationFailed)?);
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
    make: MakeSelection,
    source_identity: String,
}

#[derive(Deserialize)]
struct SporeBinding {
    mode: String,
    invitation_id: String,
}

#[derive(Deserialize)]
struct MakeSelection {
    make_package_id: String,
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
    make_package_id: &'a str,
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
            .make
            .deployment_adapter
            .as_deref()
            .ok_or(NativeInstallRefusal::BindingMismatch)?;
        let manifest = ReleaseManifest {
            schema: "conduit.release/host-bundle@1",
            target_id: &provision.spore.target,
            make_package_id: &provision.spore.make.make_package_id,
            output: "native-bundle",
            builder_adapter: &provision.spore.make.builder_adapter,
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
            "native package provision lost its exact body, spore, or IMAGE identity".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
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
pub(crate) mod tests;
