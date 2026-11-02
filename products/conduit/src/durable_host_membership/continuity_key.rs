//! Private custody of the original admitted invitation identity. Retaining this
//! key neither authenticates an owner receipt nor authorizes a return or a Line.
use super::PendingBodyJoin;
use conduit_body::{MembershipCredential, SpawnInvitationSecret};
use conduit_core::HostAdvertisement;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

const PATH: &str = "member-continuity-key.json";
const SCHEMA: &str = "conduit.body/member-continuity-key@1";
const MAX_BYTES: u64 = 4096;

// Deliberately no Debug, inspection, response serialization or secret accessor.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RetainedKey {
    schema: String,
    body_id: String,
    part_id: String,
    host_id: String,
    invitation_id: String,
    secret: [u8; 32],
}
impl Drop for RetainedKey {
    fn drop(&mut self) {
        self.secret.fill(0);
    }
}

pub(super) fn retain(
    state: &Path,
    pending: &PendingBodyJoin,
    credential: &MembershipCredential,
    advertisement: &HostAdvertisement,
) -> Result<(), String> {
    let request = &pending.request;
    request
        .validate()
        .map_err(|_| "invalid pending continuity request")?;
    pending
        .invitation
        .validate(0)
        .map_err(|_| "invalid pending continuity invitation")?;
    if pending.schema != "conduit.body/pending-join@1"
        || request.host_advertisement != *advertisement
        || request.body_id != credential.body_id
        || advertisement.host_id != credential.host_id
        || advertisement.boot_id != credential.boot_id
        || pending.invitation.claim.invitation_id != request.invitation_id
        || pending.invitation.claim.body_id != request.body_id
        || pending.invitation.claim.nonce != request.nonce
        || credential.part_id.as_str().is_empty()
    {
        return Err("pending continuity key lost its exact admitted scope".into());
    }
    let secret = SpawnInvitationSecret::from_csprng_bytes(pending.invitation.secret)
        .map_err(|_| "invalid pending continuity key")?;
    let signature = secret.sign(&pending.invitation.claim.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    ));
    if request.signature.as_slice() != signature {
        return Err("pending continuity key differs from signed admission request".into());
    }
    let record = RetainedKey {
        schema: SCHEMA.into(),
        body_id: credential.body_id.as_str().into(),
        part_id: credential.part_id.as_str().into(),
        host_id: credential.host_id.as_str().into(),
        invitation_id: request.invitation_id.as_str().into(),
        secret: pending.invitation.secret,
    };
    let body = state.join("body");
    require_directory(state)?;
    require_directory(&body)?;
    super::restrict_directory(&body)?;
    let path = body.join(PATH);
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            let existing = read(&path)?;
            if existing != record {
                return Err("retained continuity key has another scope or identity".into());
            }
            return sync_directory(&body);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("inspect private continuity key failed".into()),
    }
    let temporary = body.join("member-continuity-key.tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| "create private continuity staging file failed")?;
    let mut bytes =
        serde_json::to_vec(&record).map_err(|_| "encode private continuity key failed")?;
    if bytes.len() as u64 > MAX_BYTES {
        bytes.fill(0);
        let _ = fs::remove_file(&temporary);
        return Err("private continuity key exceeds finite bound".into());
    }
    let result = file.write_all(&bytes).and_then(|()| file.sync_all());
    bytes.fill(0);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("persist private continuity key failed".into());
    }
    // Atomic no-clobber publication: an existing file OR symlink refuses. A
    // failed membership commit leaves the pending request and this key intact.
    if fs::hard_link(&temporary, &path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("commit private continuity key without overwrite failed".into());
    }
    fs::remove_file(&temporary).map_err(|_| "retire private continuity staging file failed")?;
    sync_directory(&body)
}

fn require_directory(path: &Path) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "private continuity directory is absent")?;
    if !metadata.file_type().is_dir() {
        return Err("private continuity directory is not an actual directory".into());
    }
    Ok(())
}
fn read(path: &Path) -> Result<RetainedKey, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "inspect private continuity record failed")?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_BYTES {
        return Err("private continuity record is not a bounded regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err("private continuity record permissions are not 0600".into());
        }
    }
    let file = fs::File::open(path).map_err(|_| "open private continuity record failed")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file
            .metadata()
            .map_err(|_| "inspect opened continuity record failed")?;
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
            return Err("private continuity record changed during read".into());
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "read private continuity record failed")?;
    let result = if bytes.len() as u64 <= MAX_BYTES {
        serde_json::from_slice(&bytes).map_err(|_| "invalid private continuity record".to_string())
    } else {
        Err("private continuity record exceeds finite bound".into())
    };
    bytes.fill(0);
    result
}
fn sync_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        fs::File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(|_| "sync private continuity directory failed")?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests;
