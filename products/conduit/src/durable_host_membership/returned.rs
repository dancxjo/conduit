//! Durable credential replacement after an independently authenticated return.
//! Caller owns the installed service lock and must validate the pinned owner's
//! exact challenge/response. Public credential coherence is not authentication.
use super::*;
use std::io::Write;

pub(crate) fn load_joined_credential(
    _state: &Path,
    installation: &Installation,
) -> Result<MembershipCredential, String> {
    let binding = installation
        .joined_body_state
        .as_ref()
        .ok_or("installed Host has no joined membership")?;
    validate(binding, installation)?;
    let bytes = bounded_read(Path::new(&binding.credential_path), 64 * 1024)?;
    if digest(&bytes) != binding.credential_sha256 {
        return Err("joined credential changed during read".into());
    }
    // Validate the exact bytes used here, not an earlier path observation.
    let credential: MembershipCredential =
        serde_json::from_slice(&bytes).map_err(|_| "invalid joined credential")?;
    if credential.body_id.as_str() != binding.body_id
        || credential.part_id.as_str() != binding.part_id
        || credential.host_id.as_str() != installation.host_id
    {
        return Err("joined credential scope differs".into());
    }
    Ok(credential)
}

pub(crate) fn retain_returned_membership(
    state: &Path,
    prior: &MembershipCredential,
    returned: &MembershipCredential,
    actual: &HostAdvertisement,
) -> Result<(), String> {
    let mut installation = read_installation(&state.join("installation.json"))?;
    if installation.body_state.is_some() || load_joined_credential(state, &installation)? != *prior
    {
        return Err("return completion lost its exact prior membership".into());
    }
    let ids = [
        returned.credential_id.as_str(),
        returned.body_id.as_str(),
        returned.part_id.as_str(),
        returned.host_id.as_str(),
        returned.boot_id.as_str(),
    ];
    if ids.iter().any(|id| id.is_empty() || id.len() > 256)
        || returned.body_id != prior.body_id
        || returned.part_id != prior.part_id
        || returned.host_id != prior.host_id
        || returned.host_id != actual.host_id
        || returned.boot_id != actual.boot_id
        || returned.host_id.as_str() != installation.host_id
        || returned.credential_id == prior.credential_id
        || actual.protocol_version != conduit_core::PROTOCOL_VERSION
        || actual.offer_generation.0 == 0
    {
        return Err("return completion differs from retained member or actual Host".into());
    }
    continuity_key::validate_member_scope(state, prior)?;
    let bytes =
        serde_json::to_vec_pretty(returned).map_err(|_| "encode returned credential failed")?;
    if bytes.len() > 64 * 1024 {
        return Err("returned credential exceeds finite bound".into());
    }
    let fingerprint = digest(&bytes);
    let directory = state.join("body");
    let path = directory.join(format!("membership-credential-{fingerprint}.json"));
    publish_immutable(&path, &bytes)?;
    installation.joined_body_state = Some(JoinedBodyBinding {
        body_id: returned.body_id.as_str().into(),
        part_id: returned.part_id.as_str().into(),
        credential_sha256: fingerprint,
        credential_path: path.display().to_string(),
    });
    // This is the sole commit point. Failure leaves the old binding/file valid;
    // a published orphan is harmless and supports an exact idempotent retry.
    write_json_atomic(&state.join("installation.json"), &installation)
}

fn publish_immutable(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.file_type().is_file() || bounded_read(path, 64 * 1024)? != bytes {
            return Err("immutable return credential already differs".into());
        }
        return sync_directory(path.parent().ok_or("credential directory absent")?);
    }
    let directory = path.parent().ok_or("credential directory absent")?;
    let identity = super::super::fresh_identity("return-credential", "stage");
    let suffix = identity
        .rsplit('/')
        .next()
        .ok_or("return staging identity absent")?;
    let staging = directory.join(format!("return-credential-{suffix}.tmp"));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&staging)
        .map_err(|_| "create return credential staging failed")?;
    let result = (|| {
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "sync return credential staging failed")?;
        fs::hard_link(&staging, path).map_err(|_| "publish immutable return credential failed")?;
        sync_directory(directory)
    })();
    // Remove only the inode created by this attempt, never a substituted path.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(owned), Ok(current)) = (file.metadata(), fs::symlink_metadata(&staging)) {
            if current.file_type().is_file()
                && owned.dev() == current.dev()
                && owned.ino() == current.ino()
            {
                let _ = fs::remove_file(&staging);
            }
        }
    }
    // Preserve uncertain staging on platforms without reviewed inode identity.
    result
}
fn sync_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| "sync returned credential directory failed")?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
