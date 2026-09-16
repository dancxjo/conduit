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

#[path = "durable_host_invitation.rs"]
mod invitation;
pub(crate) use invitation::{accept_body_invitation, issue_body_invitation};

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
    body_state: Option<BodyBinding>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BodyBinding {
    body_id: String,
    biography_sha256: String,
    biography_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuntimeStatus {
    schema: String,
    host_id: String,
    boot_id: String,
    offer_generation: u64,
    process_id: u32,
    release_bundle_sha256: String,
    body_id: Option<String>,
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
        HostServiceCommand::OwnBody {
            evidence,
            state_dir,
        } => own_body(&evidence, &state_dir),
    }
}

fn own_body(evidence_path: &Path, state_dir: &Path) -> Result<(), String> {
    let evidence_bytes = bounded_read(evidence_path, 2 * 1024 * 1024)?;
    let evidence: conduit_body::BodyBiographyEvidence = serde_json::from_slice(&evidence_bytes)
        .map_err(|error| format!("Body biography evidence: {error}"))?;
    evidence
        .validate()
        .map_err(|error| format!("Body biography evidence refused: {error:?}"))?;
    let install_path = state_dir.join("installation.json");
    let mut installation = read_installation(&install_path)?;
    if let Some(current) = &installation.body_state {
        if current.body_id != evidence.body_id.as_str() {
            return Err(format!(
                "durable Host already owns Body {}; refusing replacement by {}",
                current.body_id,
                evidence.body_id.as_str()
            ));
        }
    }
    let body_dir = state_dir.join("body");
    fs::create_dir_all(&body_dir)
        .map_err(|error| format!("create Body state directory: {error}"))?;
    restrict_directory(&body_dir)?;
    let retained_path = body_dir.join("biography.json");
    write_bytes_atomic(&retained_path, &evidence_bytes)?;
    installation.body_state = Some(BodyBinding {
        body_id: evidence.body_id.as_str().into(),
        biography_sha256: digest(&evidence_bytes),
        biography_path: retained_path.display().to_string(),
    });
    write_json_atomic(&install_path, &installation)?;
    println!("durable Host now owns Body {}", evidence.body_id.as_str());
    Ok(())
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
    crate::durable_host_control::ensure_secret(state_dir)?;
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
    let (status, truth) = prepare_runtime(state_dir)?;
    println!(
        "durable Host {} boot {} is running",
        status.host_id, status.boot_id
    );
    let outcome = crate::durable_host_control::serve(state_dir, truth);
    let runtime = state_dir.join("runtime.json");
    match fs::remove_file(&runtime) {
        Ok(()) => outcome,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => outcome,
        Err(error) => Err(format!(
            "durable Host stopped but its runtime marker could not be retired: {error}"
        )),
    }
}

#[cfg(test)]
fn start_runtime(state_dir: &Path) -> Result<RuntimeStatus, String> {
    prepare_runtime(state_dir).map(|(status, _)| status)
}

fn prepare_runtime(
    state_dir: &Path,
) -> Result<(RuntimeStatus, crate::durable_host_control::DurableHostTruth), String> {
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
        body_id: installation
            .body_state
            .as_ref()
            .map(|binding| binding.body_id.clone()),
        release_bundle_sha256: installation.release_bundle_sha256,
    };
    write_json_atomic(&state_dir.join("runtime.json"), &status)?;
    let executable = Path::new(&installation.product_executable);
    let image_content_digest = digest(&bounded_read(executable, MAXIMUM_RELEASE_FILE_BYTES)?);
    let truth = crate::durable_host_control::DurableHostTruth {
        target_id: running_target_id()?.into(),
        image_content_digest,
        advertisement: host.advertisement().clone(),
    };
    Ok((status, truth))
}

fn running_target_id() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("std/x86_64/computer"),
        ("windows", "x86_64") => Ok("std/x86_64/windows-computer"),
        ("macos", "aarch64") => Ok("std/aarch64/macos-computer"),
        (os, architecture) => Err(format!(
            "no reviewed durable Host profile is installed for {os}/{architecture}"
        )),
    }
}

fn status(state_dir: &Path, json: bool) -> Result<(), String> {
    let installation = read_installation(&state_dir.join("installation.json"))?;
    if let Some(status) = observe_current_runtime(state_dir, &installation)? {
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
    } else if json {
        println!(
            "{}",
            serde_json::json!({
                "schema": "conduit.install/durable-host-status@1",
                "host_id": installation.host_id,
                "presence": "installed-offline",
                "release_bundle_sha256": installation.release_bundle_sha256,
                "body_id": installation.body_state.as_ref().map(|binding| binding.body_id.as_str()),
            })
        );
    } else {
        println!(
            "Host {} is installed but not observed running; release {}",
            installation.host_id, installation.release_bundle_sha256
        );
    }
    Ok(())
}

fn observe_current_runtime(
    state_dir: &Path,
    installation: &Installation,
) -> Result<Option<RuntimeStatus>, String> {
    let runtime = state_dir.join("runtime.json");
    if !runtime.exists() {
        return Ok(None);
    }
    let status = {
        let bytes = bounded_read(&runtime, 64 * 1024)?;
        let status: RuntimeStatus = serde_json::from_slice(&bytes)
            .map_err(|error| format!("durable Host runtime status: {error}"))?;
        if status.schema != RUNTIME_SCHEMA || status.host_id != installation.host_id {
            return Err("durable Host runtime status is stale or belongs to another Host".into());
        }
        status
    };
    let truth = match crate::durable_host_control::current(state_dir) {
        Ok(truth) => truth,
        Err(_) => return Ok(None),
    };
    let advertisement = &truth.advertisement;
    if advertisement.host_id.as_str() != status.host_id
        || advertisement.boot_id.as_str() != status.boot_id
        || advertisement.offer_generation.0 != status.offer_generation
    {
        return Err("durable Host control truth disagrees with its runtime marker".into());
    }
    Ok(Some(status))
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
    if let Some(binding) = &value.body_state {
        if binding.body_id.is_empty()
            || !valid_digest(&binding.biography_sha256)
            || digest(&bounded_read(
                Path::new(&binding.biography_path),
                2 * 1024 * 1024,
            )?) != binding.biography_sha256
        {
            return Err("retained Body biography identity is invalid or stale".into());
        }
    }
    Ok(value)
}

fn write_service_definition(state_dir: &Path, installation: &Installation) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let unit = linux_service_definition(state_dir, installation)?;
        fs::write(state_dir.join("conduit-host.service"), unit)
            .map_err(|error| format!("write systemd user-service definition: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    Err("this release has no reviewed durable service carrier for the current platform".into())
}

#[cfg(target_os = "linux")]
fn linux_service_definition(
    state_dir: &Path,
    installation: &Installation,
) -> Result<String, String> {
    let state_dir = fs::canonicalize(state_dir)
        .map_err(|error| format!("resolve durable Host state directory: {error}"))?;
    let executable = fs::canonicalize(&installation.product_executable)
        .map_err(|error| format!("resolve installed Conduit executable: {error}"))?;
    let executable = systemd_exec_argument(&executable)?;
    let state_dir = systemd_exec_argument(&state_dir)?;
    Ok(format!(
        "[Unit]\nDescription=Conduit durable Host\n\n[Service]\nExecStart={executable} host service run --state-dir {state_dir}\nRestart=on-failure\nRestartSec=1s\nUMask=0077\n\n[Install]\nWantedBy=default.target\n"
    ))
}

#[cfg(target_os = "linux")]
fn systemd_exec_argument(path: &Path) -> Result<String, String> {
    let value = path
        .to_str()
        .ok_or_else(|| "durable Host service path is not UTF-8".to_string())?;
    if value.contains(['\n', '\r', '\0']) {
        return Err("durable Host service path contains a forbidden control character".into());
    }
    // systemd expands percent specifiers even inside quotes. Doubling percent
    // and quoting shell-significant separators keeps the exact installed path.
    let escaped = value
        .replace('%', "%%")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    Ok(format!("\"{escaped}\""))
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

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
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

fn current_time_millis() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "system clock exceeds invitation representation".into())
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
    use super::invitation::{PortableInvitation, INVITATION_SCHEMA};
    use super::*;
    use conduit_body::{AdmissionManager, SpawnInvitationSecret};
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

    #[cfg(target_os = "linux")]
    #[test]
    fn systemd_definition_uses_absolute_escaped_paths_and_private_state() {
        let (manifest, original_state) = fixture();
        let state = original_state.with_file_name("state with 100% identity");
        let installation = install(&manifest, &state).unwrap();
        let unit = fs::read_to_string(state.join("conduit-host.service")).unwrap();
        let absolute_state = fs::canonicalize(&state).unwrap();
        let absolute_executable = fs::canonicalize(&installation.product_executable).unwrap();

        assert!(unit.contains(&format!(
            "ExecStart={} host service run --state-dir {}",
            systemd_exec_argument(&absolute_executable).unwrap(),
            systemd_exec_argument(&absolute_state).unwrap()
        )));
        assert!(unit.contains("Restart=on-failure\nRestartSec=1s\nUMask=0077"));
        assert!(unit.contains("100%% identity"));
        assert!(!unit.contains("--state-dir state with"));
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

    #[test]
    fn retained_runtime_marker_without_authenticated_owner_is_offline() {
        let (manifest, state) = fixture();
        let installation = install(&manifest, &state).unwrap();
        start_runtime(&state).unwrap();
        assert!(state.join("runtime.json").is_file());
        assert!(observe_current_runtime(&state, &installation)
            .unwrap()
            .is_none());
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }

    #[test]
    fn invalid_body_evidence_refuses_without_changing_durable_ownership() {
        let (manifest, state) = fixture();
        install(&manifest, &state).unwrap();
        let evidence = state.parent().unwrap().join("invalid-body.json");
        fs::write(
            &evidence,
            br#"{"schema":"conduit.body/biography-evidence@2"}"#,
        )
        .unwrap();
        assert!(own_body(&evidence, &state)
            .unwrap_err()
            .contains("Body biography evidence"));
        assert!(read_installation(&state.join("installation.json"))
            .unwrap()
            .body_state
            .is_none());
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }

    #[test]
    fn installed_body_issues_distinct_bounded_machine_readable_invitations() {
        let (manifest, state) = fixture();
        let mut installation = install(&manifest, &state).unwrap();
        let body_dir = state.join("body");
        fs::create_dir_all(&body_dir).unwrap();
        let body = conduit_body::Body::born(
            "source/invitation-test".into(),
            "checked/invitation-test".into(),
            1,
            conduit_core::SignId::from("sign/invitation-test/born"),
        )
        .unwrap();
        let biography = conduit_body::BodyBiographyEvidence::born(
            body.clone(),
            conduit_body::BodyMembership::new(body.body_id.clone()).unwrap(),
            "Invitation test".into(),
        )
        .unwrap();
        let biography_bytes = serde_json::to_vec_pretty(&biography).unwrap();
        let biography_path = body_dir.join("biography.json");
        fs::write(&biography_path, &biography_bytes).unwrap();
        installation.body_state = Some(BodyBinding {
            body_id: body.body_id.as_str().into(),
            biography_sha256: digest(&biography_bytes),
            biography_path: biography_path.display().to_string(),
        });
        write_json_atomic(&state.join("installation.json"), &installation).unwrap();

        issue_body_invitation(&state, 30).unwrap();
        let first: AdmissionManager = serde_json::from_slice(
            &bounded_read(&body_dir.join("admission.json"), 256 * 1024).unwrap(),
        )
        .unwrap();
        issue_body_invitation(&state, 30).unwrap();
        let second: AdmissionManager = serde_json::from_slice(
            &bounded_read(&body_dir.join("admission.json"), 256 * 1024).unwrap(),
        )
        .unwrap();
        assert_eq!(first.body_id, body.body_id);
        assert_eq!(second.body_id, first.body_id);
        assert_ne!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert!(!serde_json::to_vec(&second)
            .unwrap()
            .windows(32)
            .any(|window| window == [0_u8; 32]));
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }

    #[test]
    fn installed_host_accepts_exact_invitation_without_claiming_membership_or_play() {
        let (manifest, state) = fixture();
        let installation = install(&manifest, &state).unwrap();
        let runtime = start_runtime(&state).unwrap();
        let body_id = conduit_body::Body::born(
            "source/invited".into(),
            "checked/invited".into(),
            1,
            conduit_core::SignId::from("sign/invited/born"),
        )
        .unwrap()
        .body_id;
        let mut manager = AdmissionManager::new(body_id.clone()).unwrap();
        let secret_bytes = [7_u8; 32];
        let invitation = manager
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes(secret_bytes).unwrap(),
                [8_u8; 32],
                10,
                u64::MAX,
            )
            .unwrap_err();
        assert_eq!(invitation, conduit_body::AdmissionRefusal::InvalidExpiry);

        let now = current_time_millis().unwrap();
        let invitation = manager
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes(secret_bytes).unwrap(),
                [8_u8; 32],
                now,
                now + 30_000,
            )
            .unwrap();
        let path = state.parent().unwrap().join("invitation.json");
        fs::write(
            &path,
            serde_json::to_vec(&PortableInvitation {
                schema: INVITATION_SCHEMA.into(),
                claim: invitation.claim(),
                secret: secret_bytes,
            })
            .unwrap(),
        )
        .unwrap();

        accept_body_invitation(&path, &state, true).unwrap();

        let pending: serde_json::Value = serde_json::from_slice(
            &bounded_read(&state.join("body/pending-join.json"), 256 * 1024).unwrap(),
        )
        .unwrap();
        assert_eq!(pending["invitation"]["claim"]["body_id"], body_id.as_str());
        assert_eq!(
            pending["request"]["host_advertisement"]["host_id"],
            installation.host_id
        );
        assert_eq!(
            pending["request"]["host_advertisement"]["boot_id"],
            runtime.boot_id
        );
        assert_eq!(pending["request"]["membership_admitted"], false);
        assert_eq!(pending["request"]["plan_created"], false);
        assert_eq!(pending["request"]["play_created"], false);
        assert!(pending["request"]["host_advertisement"]["capabilities"]
            .as_array()
            .is_some_and(|offers| !offers.is_empty()));
        fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }
}
