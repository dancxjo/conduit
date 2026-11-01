//! Actual BrowserBundle fabrication for a checked declarative Host.
use conduit_body::PortableInvitation;
use conduit_body_make::{
    check_body_description, seal_prebuilt_body_spore_with_content_digest, BuiltSpore,
    CheckedBodyDescription, SelectedPrebuiltContent, SporeJoinMode,
};
use conduit_host_make::validate_profile;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum BrowserFabricationRefusal {
    BodyAuthorityUnavailable,
    BodyAuthorityMismatch,
}
impl std::fmt::Display for BrowserFabricationRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BodyAuthorityUnavailable => f.write_str("BodyAuthorityUnavailable: browser fabrication needs the installed owning Host and its live authority"),
            Self::BodyAuthorityMismatch => f.write_str("BodyAuthorityMismatch: installed owner or invitation differs from the authored Body"),
        }
    }
}
impl std::error::Error for BrowserFabricationRefusal {}

// The installed owner status has the existing 2 MiB command envelope. Make
// needs only its Body identity: discard Host catalog fields during decoding.
const MAXIMUM_OWNER_STATUS_BYTES: u64 = 2 * 1024 * 1024;
const MAXIMUM_INVITATION_BYTES: u64 = 64 * 1024;
fn read_owner_response(reader: impl std::io::Read, action: &str) -> Result<Value> {
    use std::io::Read;
    let maximum = if action == "status" {
        MAXIMUM_OWNER_STATUS_BYTES
    } else {
        MAXIMUM_INVITATION_BYTES
    };
    let mut decoder =
        serde_json::Deserializer::from_reader(std::io::BufReader::new(reader.take(maximum)));
    let value = if action == "status" {
        #[derive(serde::Deserialize, serde::Serialize)]
        struct Biography {
            body_id: String,
        }
        #[derive(serde::Deserialize, serde::Serialize)]
        struct Status {
            biography: Biography,
        }
        let status = <Status as serde::Deserialize>::deserialize(&mut decoder)?;
        serde_json::to_value(status)?
    } else {
        <Value as serde::Deserialize>::deserialize(&mut decoder)?
    };
    decoder.end()?;
    Ok(value)
}
fn installed_product(state: &Path) -> Result<PathBuf> {
    use std::io::Read;
    #[derive(serde::Deserialize)]
    struct Installation {
        schema: String,
        product_executable: PathBuf,
    }
    let file = fs::File::open(state.join("installation.json"))
        .map_err(|_| BrowserFabricationRefusal::BodyAuthorityUnavailable)?;
    let mut decoder =
        serde_json::Deserializer::from_reader(std::io::BufReader::new(file.take(64 * 1024)));
    let installation = <Installation as serde::Deserialize>::deserialize(&mut decoder)?;
    decoder.end()?;
    if installation.schema != "conduit.install/durable-host@1"
        || !installation.product_executable.is_absolute()
    {
        return Err(BrowserFabricationRefusal::BodyAuthorityUnavailable.into());
    }
    Ok(installation.product_executable)
}
fn product_json(state: &Path, action: &str) -> Result<Value> {
    // The selected installation owns this Body; a different PATH CLI may have
    // an older biography decoder or a different product protocol.
    let mut command = Command::new(installed_product(state)?);
    command.args(["body", action, "--state-dir"]).arg(state);
    if action == "status" {
        command.arg("--json");
    }
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| BrowserFabricationRefusal::BodyAuthorityUnavailable)?;
    let response = read_owner_response(
        child
            .stdout
            .take()
            .ok_or(BrowserFabricationRefusal::BodyAuthorityUnavailable)?,
        action,
    );
    // Closing the finite reader precedes wait, including malformed/oversized
    // output: a producer cannot block while writing beyond the admitted read.
    let status = child.wait()?;
    if !status.success() {
        return Err(BrowserFabricationRefusal::BodyAuthorityUnavailable.into());
    }
    response
}

pub(super) fn require_owner(body: &CheckedBodyDescription) -> Result<PathBuf> {
    let state = std::env::var_os("CONDUIT_STATE_DIR")
        .filter(|s| !s.is_empty())
        .ok_or(BrowserFabricationRefusal::BodyAuthorityUnavailable)?;
    let state = PathBuf::from(state);
    let status = product_json(&state, "status")?;
    owner_matches(&status, &body.description().body.id)?;
    Ok(state)
}

fn node(action: &str, input: &Path, source: &Path, destination: &Path) -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new("node")
        .arg(root.join("targets/browser/tools/make-body-browser.mjs"))
        .arg(action)
        .arg(input)
        .arg(source)
        .arg(destination)
        .stdout(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err("BrowserArtifactRefused: canonical package producer failed".into());
    }
    Ok(())
}

/// Build inert reviewed bytes before minting the short-lived invitation.
pub(super) fn fabricate(
    body: &CheckedBodyDescription,
    spore: &BuiltSpore,
    output: &Path,
    state: &Path,
    distribution: &Path,
) -> Result<BuiltSpore> {
    let host = body
        .hosts()
        .iter()
        .find(|h| h.description.name == spore.manifest.host_entry_name)
        .ok_or("Unknown checked browser Host")?;
    if host.description.spore.join_mode != SporeJoinMode::SelfJoining {
        return Err("BrowserSporeBinding: browser Host must request self-joining admission".into());
    }
    require_artifact_name(&host.description.name)?;
    let root = output.join(&host.description.name);
    if root.exists() {
        return Err("BrowserArtifactExists: refusing to overwrite an earlier fabrication".into());
    }
    fs::create_dir_all(&root)?;
    let packages = conduit_workspace_make::package_set();
    let catalog = conduit_workspace_make::catalog();
    let profile_id = validate_profile(host.configuration.profile().clone(), &catalog)
        .map_err(|e| format!("BrowserProfileRefused: {e:?}"))?
        .profile_id()
        .as_str()
        .to_owned();
    let review = json!({"schema":"conduit.creche/checked-browser-configuration@1","target_id":"browser/wasm32/page","profile_id":profile_id,"configuration_id":host.configuration.configuration_id(),"selected_implementations":host.configuration.resolved_bases().iter().map(|(_,implementation)|implementation).collect::<Vec<_>>()});
    let review_path = root.join("checked-browser-configuration.json");
    fs::write(&review_path, serde_json::to_vec(&review)?)?;
    let configured = root.join("bundle");
    node("configure", &review_path, distribution, &configured)?;
    let release: Value =
        serde_json::from_slice(&fs::read(configured.join("browser-bundle-release.json"))?)?;
    let digest = release["bundle_sha256"]
        .as_str()
        .ok_or("Browser bundle lost digest")?;
    // Recheck the live owner immediately before asking it to mint authority.
    require_owner(body)?;
    let invitation_value = product_json(state, "invite")?;
    let mut invitation: PortableInvitation = serde_json::from_value(invitation_value)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    invitation
        .validate(now)
        .map_err(|e| format!("BodyAuthorityRefused: {e:?}"))?;
    if invitation.claim.body_id.as_str() != body.description().body.id {
        return Err(BrowserFabricationRefusal::BodyAuthorityMismatch.into());
    }
    // The author's invitation label requests binding; the owner chooses its real identity.
    let mut description = body.description().clone();
    description
        .hosts
        .iter_mut()
        .find(|h| h.name == host.description.name)
        .ok_or("Missing browser Host")?
        .spore
        .invitation = Some(invitation.claim.invitation_id.as_str().into());
    let configurations = body
        .hosts()
        .iter()
        .map(|h| {
            (
                h.description.configuration.clone(),
                h.configuration.configuration().clone(),
            )
        })
        .collect();
    let bound = check_body_description(description, &configurations, &catalog, &packages)
        .map_err(|e| format!("BrowserBindingRefused: {e:?}"))?;
    let image_bytes = serde_json::to_vec(&spore.image)?;
    let bound_spore = seal_prebuilt_body_spore_with_content_digest(
        &bound,
        &host.description.name,
        &spore.manifest.source_identity,
        &spore.image,
        SelectedPrebuiltContent {
            image_manifest_bytes: &image_bytes,
            image_content_digest: digest,
        },
        &catalog,
        &packages,
    )
    .map_err(|e| format!("BrowserSporeRefused: {e:?}"))?;
    let manifest_path = root.join("spore-manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&bound_spore.manifest)?,
    )?;
    let prepared = json!({"spore_manifest":bound_spore.manifest,"spore_id":bound_spore.manifest.spore_id,"body_id":bound_spore.manifest.body_id,"image_content_digest":digest,"invitation_id":invitation.claim.invitation_id,"invitation_nonce":invitation.claim.nonce,"invitation_secret":invitation.secret,"invitation_expires_at_millis":invitation.claim.expires_at_millis,"rendezvous":invitation.rendezvous});
    let private = root.join(".prepared-spore.json");
    write_private(&private, &serde_json::to_vec(&prepared)?)?;
    invitation.secret.fill(0);
    let request_path = root.join(".package-request.json");
    let artifact = root.join(format!("{}.zip", host.description.name));
    fs::write(
        &request_path,
        serde_json::to_vec(
            &json!({"manifestPath":manifest_path.canonicalize()?,"preparedPath":private.canonicalize()?,"releasePath":configured.join("browser-bundle-release.json").canonicalize()?,"outputPath":std::path::absolute(&artifact)?}),
        )?,
    )?;
    let result = node("package", &request_path, Path::new("."), Path::new("."));
    fs::remove_file(private)?;
    fs::remove_file(request_path)?;
    result?;
    Ok(bound_spore)
}
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn owner_matches(status: &Value, body_id: &str) -> Result<()> {
    if status.pointer("/biography/body_id").and_then(Value::as_str) != Some(body_id) {
        return Err(BrowserFabricationRefusal::BodyAuthorityMismatch.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn owner_status_discards_catalog_but_keeps_exact_body_and_finite_envelope() {
        let status = serde_json::json!({"host":{"capabilities":"x".repeat(100_000)},"biography":{"body_id":"body/one","other":"discard"}});
        let bytes = serde_json::to_vec(&status).unwrap();
        let parsed = super::read_owner_response(bytes.as_slice(), "status").unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({"biography":{"body_id":"body/one"}})
        );
        assert!(super::owner_matches(&parsed, "body/foreign").is_err());
        assert!(super::read_owner_response(b"{\"biography\":{}}".as_slice(), "status").is_err());
        let oversized = serde_json::to_vec(&serde_json::json!({"host":"x".repeat(super::MAXIMUM_OWNER_STATUS_BYTES as usize),"biography":{"body_id":"body/one"}})).unwrap();
        assert!(super::read_owner_response(oversized.as_slice(), "status").is_err());
        assert!(super::read_owner_response(bytes.as_slice(), "invite").is_err());
    }
    #[test]
    fn fabrication_uses_selected_installation_instead_of_path_product() {
        let directory = std::env::temp_dir().join(format!(
            "conduit-browser-installed-product-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("installation.json");
        std::fs::write(&path, br#"{"schema":"conduit.install/durable-host@1","product_executable":"/installed/exact/conduit"}"#).unwrap();
        assert_eq!(
            super::installed_product(&directory).unwrap(),
            std::path::PathBuf::from("/installed/exact/conduit")
        );
        for content in [
            r#"{"schema":"foreign","product_executable":"/installed/exact/conduit"}"#,
            r#"{"schema":"conduit.install/durable-host@1","product_executable":"relative/conduit"}"#,
            r#"{"schema":"conduit.install/durable-host@1"}"#,
        ] {
            std::fs::write(&path, content).unwrap();
            assert!(super::installed_product(&directory).is_err());
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn artifact_paths_cannot_escape_fabrication_output() {
        assert!(super::require_artifact_name("web-terminal-1").is_ok());
        for name in [
            "..",
            "../other",
            "/absolute",
            ".reviewed-browser-distribution",
            "windows\\other",
        ] {
            assert!(super::require_artifact_name(name).is_err());
        }
    }
    #[test]
    fn owner_scope_must_match_before_invitation_minting() {
        use serde_json::json;
        assert!(
            super::owner_matches(&json!({"biography":{"body_id":"body/one"}}), "body/one").is_ok()
        );
        assert!(
            super::owner_matches(&json!({"biography":{"body_id":"body/two"}}), "body/one").is_err()
        );
        assert!(super::owner_matches(&json!({"body_id":"body/one"}), "body/one").is_err());
        assert!(super::owner_matches(&json!(null), "body/one").is_err());
    }
}

pub(super) fn build_distribution(output: &Path) -> Result<PathBuf> {
    let distribution = output.join(".reviewed-browser-distribution");
    if distribution.exists() {
        return Err("BrowserReleaseExists: select a new fabrication output directory".into());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(std::env::current_exe()?)
        .args([
            "make",
            "host",
            "release",
            "--platform",
            "browser",
            "--output",
        ])
        .arg(&distribution)
        .current_dir(repository)
        .stdout(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err("BrowserReleaseUnavailable: reviewed runtime build failed".into());
    }
    Ok(distribution)
}

fn require_artifact_name(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
        || name.starts_with('.')
        || name.contains('\\')
    {
        return Err("BrowserArtifactPath: Host name must be one ordinary filename".into());
    }
    Ok(())
}
