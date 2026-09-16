//! Installed durable Host ownership and its platform-service handoff.

use crate::cli::HostServiceCommand;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path},
    time::{SystemTime, UNIX_EPOCH},
};

const INSTALL_SCHEMA: &str = "conduit.install/durable-host@1";
const RUNTIME_SCHEMA: &str = "conduit.install/durable-host-runtime@1";
const RELEASE_SCHEMA: &str = "conduit.release/host-bundle@1";
const MAXIMUM_RELEASE_FILES: usize = 32;
const MAXIMUM_RELEASE_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct ReleaseManifest {
    schema: String,
    source_identity: String,
    bundle_sha256: String,
    files: Vec<ReleaseFile>,
}

#[derive(Debug, Deserialize)]
struct ReleaseFile {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Installation {
    schema: String,
    host_id: String,
    release_source_identity: String,
    release_bundle_sha256: String,
    product_executable: String,
    body_state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuntimeStatus {
    schema: String,
    host_id: String,
    boot_id: String,
    offer_generation: u64,
    process_id: u32,
    release_bundle_sha256: String,
}

pub(crate) fn dispatch(command: HostServiceCommand) -> Result<(), String> {
    match command {
        HostServiceCommand::Install {
            manifest,
            state_dir,
        } => install(&manifest, &state_dir).and_then(|installation| {
            activate_service(&state_dir)?;
            println!(
                "installed durable Host {} from {}",
                installation.host_id, installation.release_bundle_sha256
            );
            Ok(())
        }),
        HostServiceCommand::Run { state_dir } => run(&state_dir),
        HostServiceCommand::Status { state_dir, json } => status(&state_dir, json),
    }
}

fn install(manifest_path: &Path, state_dir: &Path) -> Result<Installation, String> {
    let manifest_bytes = bounded_read(manifest_path, 256 * 1024)?;
    let manifest: ReleaseManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("release manifest: {error}"))?;
    validate_manifest(&manifest)?;
    let bundle_dir = manifest_path
        .parent()
        .ok_or_else(|| "release manifest has no bundle directory".to_string())?;
    for file in &manifest.files {
        verify_release_file(bundle_dir, file)?;
    }

    fs::create_dir_all(state_dir).map_err(|error| format!("create state directory: {error}"))?;
    restrict_directory(state_dir)?;
    let install_path = state_dir.join("installation.json");
    let existing = if install_path.exists() {
        Some(read_installation(&install_path)?)
    } else {
        None
    };
    let executable = manifest
        .files
        .iter()
        .find(|file| file.path.starts_with("conduit-") && !file.path.contains("tour"))
        .ok_or_else(|| "release has no installed Conduit product executable".to_string())?;
    let bin_dir = state_dir.join("bin");
    fs::create_dir_all(&bin_dir).map_err(|error| format!("create installation bin: {error}"))?;
    for file in &manifest.files {
        let name = Path::new(&file.path)
            .file_name()
            .ok_or_else(|| "release path has no file name".to_string())?;
        fs::copy(bundle_dir.join(&file.path), bin_dir.join(name))
            .map_err(|error| format!("install {}: {error}", file.path))?;
    }
    let product_executable = bin_dir.join(
        Path::new(&executable.path)
            .file_name()
            .ok_or_else(|| "product executable name is invalid".to_string())?,
    );
    make_executable(&product_executable)?;
    let host_id = existing
        .as_ref()
        .map(|value| value.host_id.clone())
        .unwrap_or_else(|| fresh_identity("host/installed", &manifest.bundle_sha256));
    let installation = Installation {
        schema: INSTALL_SCHEMA.into(),
        host_id,
        release_source_identity: manifest.source_identity,
        release_bundle_sha256: manifest.bundle_sha256,
        product_executable: product_executable.display().to_string(),
        body_state: existing.and_then(|value| value.body_state),
    };
    write_json_atomic(&install_path, &installation)?;
    write_service_definition(state_dir, &installation)?;
    Ok(installation)
}

fn run(state_dir: &Path) -> Result<(), String> {
    let status = start_runtime(state_dir)?;
    println!(
        "durable Host {} boot {} is running",
        status.host_id, status.boot_id
    );
    loop {
        std::thread::park_timeout(std::time::Duration::from_secs(3_600));
    }
}

fn start_runtime(state_dir: &Path) -> Result<RuntimeStatus, String> {
    let installation = read_installation(&state_dir.join("installation.json"))?;
    let boot_id = fresh_identity("boot/installed", &installation.host_id);
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(installation.host_id.as_str()),
        boot_id: BootId::from(boot_id.as_str()),
        offer_generation: OfferGeneration(1),
    });
    let status = RuntimeStatus {
        schema: RUNTIME_SCHEMA.into(),
        host_id: host.advertisement().host_id.as_str().into(),
        boot_id: host.advertisement().boot_id.as_str().into(),
        offer_generation: host.advertisement().offer_generation.0,
        process_id: std::process::id(),
        release_bundle_sha256: installation.release_bundle_sha256,
    };
    write_json_atomic(&state_dir.join("runtime.json"), &status)?;
    Ok(status)
}

fn status(state_dir: &Path, json: bool) -> Result<(), String> {
    let installation = read_installation(&state_dir.join("installation.json"))?;
    let runtime = state_dir.join("runtime.json");
    if runtime.exists() {
        let bytes = bounded_read(&runtime, 64 * 1024)?;
        let status: RuntimeStatus = serde_json::from_slice(&bytes)
            .map_err(|error| format!("durable Host runtime status: {error}"))?;
        if status.schema != RUNTIME_SCHEMA || status.host_id != installation.host_id {
            return Err("durable Host runtime status is stale or belongs to another Host".into());
        }
        if json {
            println!(
                "{}",
                serde_json::to_string(&status)
                    .map_err(|error| format!("encode durable Host status: {error}"))?
            );
        } else {
            println!(
                "Host {} boot {} pid {} release {}",
                status.host_id, status.boot_id, status.process_id, status.release_bundle_sha256
            );
        }
    } else {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "schema": "conduit.install/durable-host-status@1",
                    "host_id": installation.host_id,
                    "presence": "installed-offline",
                    "release_bundle_sha256": installation.release_bundle_sha256,
                })
            );
        } else {
            println!(
                "Host {} is installed but not observed running; release {}",
                installation.host_id, installation.release_bundle_sha256
            );
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &ReleaseManifest) -> Result<(), String> {
    if manifest.schema != RELEASE_SCHEMA
        || manifest.source_identity.is_empty()
        || !valid_digest(&manifest.bundle_sha256)
        || manifest.files.is_empty()
        || manifest.files.len() > MAXIMUM_RELEASE_FILES
    {
        return Err("release manifest identity or finite bounds are invalid".into());
    }
    if bundle_digest(&manifest.files) != manifest.bundle_sha256 {
        return Err("release manifest bundle identity does not match its exact file set".into());
    }
    Ok(())
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

fn verify_release_file(root: &Path, file: &ReleaseFile) -> Result<(), String> {
    let relative = Path::new(&file.path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || file.bytes == 0
        || file.bytes > MAXIMUM_RELEASE_FILE_BYTES
        || !valid_digest(&file.sha256)
    {
        return Err(format!("release file {} is not safely bounded", file.path));
    }
    let bytes = bounded_read(&root.join(relative), MAXIMUM_RELEASE_FILE_BYTES)?;
    if bytes.len() as u64 != file.bytes || digest(&bytes) != file.sha256 {
        return Err(format!(
            "release file {} failed exact verification",
            file.path
        ));
    }
    Ok(())
}

fn read_installation(path: &Path) -> Result<Installation, String> {
    let bytes = bounded_read(path, 64 * 1024)?;
    let value: Installation =
        serde_json::from_slice(&bytes).map_err(|error| format!("installation state: {error}"))?;
    if value.schema != INSTALL_SCHEMA
        || value.host_id.is_empty()
        || !valid_digest(&value.release_bundle_sha256)
    {
        return Err("installation state is invalid".into());
    }
    Ok(value)
}

fn write_service_definition(state_dir: &Path, installation: &Installation) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let unit = format!(
            "[Unit]\nDescription=Conduit durable Host\n\n[Service]\nExecStart={} host service run --state-dir {}\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n",
            installation.product_executable,
            state_dir.display()
        );
        fs::write(state_dir.join("conduit-host.service"), unit)
            .map_err(|error| format!("write systemd user-service definition: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    Err("this release has no reviewed durable service carrier for the current platform".into())
}

#[cfg(target_os = "linux")]
fn activate_service(state_dir: &Path) -> Result<(), String> {
    let unit = state_dir.join("conduit-host.service");
    let link = std::process::Command::new("systemctl")
        .args(["--user", "link"])
        .arg(&unit)
        .status()
        .map_err(|error| format!("link durable Host user service: {error}"))?;
    if !link.success() {
        return Err(format!(
            "installation is verified at {}, but linking durable startup failed with {link}",
            state_dir.display()
        ));
    }
    let start = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "conduit-host.service"])
        .status()
        .map_err(|error| format!("enable durable Host user service: {error}"))?;
    if !start.success() {
        return Err(format!(
            "installation is recoverable at {}, but durable startup failed with {start}",
            state_dir.display()
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn activate_service(_state_dir: &Path) -> Result<(), String> {
    Err("this release has no reviewed durable service activation for the current platform".into())
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| format!("commit {}: {error}", path.display()))
}

fn bounded_read(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("inspect {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(format!("{} violates its finite file bound", path.display()));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    Ok(bytes)
}

fn fresh_identity(prefix: &str, basis: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update(basis.as_bytes());
    hasher.update(now.as_nanos().to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    format!("{prefix}/{:x}", hasher.finalize())
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(unix)]
fn restrict_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("restrict installation directory: {error}"))
}

#[cfg(not(unix))]
fn restrict_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(fresh_identity("conduit-durable-host-test", "fixture"));
        let bundle = root.join("bundle");
        let state = root.join("state");
        fs::create_dir_all(&bundle).unwrap();
        let name = "conduit-linux-x86_64";
        let bytes = b"reviewed-product-executable";
        fs::write(bundle.join(name), bytes).unwrap();
        let files = vec![ReleaseFile {
            path: name.into(),
            bytes: bytes.len() as u64,
            sha256: digest(bytes),
        }];
        let manifest = serde_json::json!({
            "schema": RELEASE_SCHEMA,
            "target_id": "std/x86_64/computer",
            "fabrication_package_id": "hosted-native@1",
            "output": "native-bundle",
            "builder_adapter": "fixture/build@1",
            "deployment_adapter": "fixture/install@1",
            "source_identity": "commit:fixture",
            "bundle_sha256": bundle_digest(&files),
            "files": files.iter().map(|file| serde_json::json!({
                "path": file.path,
                "bytes": file.bytes,
                "sha256": file.sha256,
                "media_type": "application/vnd.conduit.host+executable"
            })).collect::<Vec<_>>()
        });
        fs::write(
            bundle.join("hosted-linux-x86_64.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        (bundle.join("hosted-linux-x86_64.json"), state)
    }

    #[test]
    fn reinstall_preserves_host_identity_and_records_exact_release() {
        let (manifest, state) = fixture();
        let first = install(&manifest, &state).unwrap();
        let second = install(&manifest, &state).unwrap();
        assert_eq!(first.host_id, second.host_id);
        assert_eq!(first.release_bundle_sha256, second.release_bundle_sha256);
        assert!(state.join("conduit-host.service").is_file());
        assert!(Path::new(&second.product_executable).is_file());
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }

    #[test]
    fn tampered_release_refuses_before_installation_state_exists() {
        let (manifest, state) = fixture();
        fs::write(
            manifest.parent().unwrap().join("conduit-linux-x86_64"),
            b"tampered",
        )
        .unwrap();
        assert!(install(&manifest, &state)
            .unwrap_err()
            .contains("failed exact verification"));
        assert!(!state.join("installation.json").exists());
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }

    #[test]
    fn real_service_restarts_keep_host_and_refresh_boot() {
        let (manifest, state) = fixture();
        let installation = install(&manifest, &state).unwrap();
        let first = start_runtime(&state).unwrap();
        let second = start_runtime(&state).unwrap();
        assert_eq!(first.host_id, installation.host_id);
        assert_eq!(second.host_id, installation.host_id);
        assert_ne!(first.boot_id, second.boot_id);
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }
}
