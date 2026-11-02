//! Private custody of the original admitted invitation identity. Retaining this
//! key neither authenticates an owner receipt nor authorizes a return or a Line.
use super::PendingBodyJoin;
use conduit_body::{MembershipCredential, SpawnInvitationSecret};
use conduit_core::HostAdvertisement;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
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

/// Sign only an already authenticated owner's bounded return challenge.
/// This proves retained member-key possession; it grants no admission or Line.
/// A detached member may return in the same Boot, as Core permits: freshness is
/// the owner's nonce and exact current advertisement, not a fabricated reboot.
pub(crate) fn sign_return_challenge(
    state: &Path,
    credential: &MembershipCredential,
    advertisement: &HostAdvertisement,
    challenge: &conduit_body::PartReturnChallenge,
    now_millis: u64,
) -> Result<conduit_body::PartReturnProof, String> {
    require_directory(state)?;
    let body = state.join("body");
    require_directory(&body)?;
    let record = read(&body.join(PATH))?;
    let identities = [
        credential.credential_id.as_str(),
        credential.body_id.as_str(),
        credential.part_id.as_str(),
        credential.host_id.as_str(),
        credential.boot_id.as_str(),
        challenge.admission_id.as_str(),
        challenge.body_id.as_str(),
        challenge.part_id.as_str(),
        challenge.host_id.as_str(),
        challenge.boot_id.as_str(),
        record.invitation_id.as_str(),
    ];
    if record.schema != SCHEMA
        || identities.iter().any(|id| id.is_empty() || id.len() > 256)
        || record.body_id != credential.body_id.as_str()
        || record.part_id != credential.part_id.as_str()
        || record.host_id != credential.host_id.as_str()
        || challenge.body_id != credential.body_id
        || challenge.part_id != credential.part_id
        || challenge.host_id != credential.host_id
        || advertisement.host_id != credential.host_id
        || challenge.boot_id != advertisement.boot_id
        || challenge.offer_generation != advertisement.offer_generation
        || advertisement.offer_generation.0 == 0
        || advertisement.protocol_version != conduit_core::PROTOCOL_VERSION
    {
        return Err("return challenge differs from retained member scope".into());
    }
    if challenge.nonce == [0; 32]
        || challenge.issued_at_millis > now_millis
        || now_millis >= challenge.expires_at_millis
        || challenge
            .expires_at_millis
            .saturating_sub(challenge.issued_at_millis)
            > conduit_body::MAX_ADMISSION_TTL_MILLIS
    {
        return Err("return challenge is not currently bounded".into());
    }
    let key = SpawnInvitationSecret::from_csprng_bytes(record.secret)
        .map_err(|_| "invalid retained continuity key")?;
    Ok(conduit_body::PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()),
    })
}

pub(super) fn validate_member_scope(
    state: &Path,
    credential: &MembershipCredential,
) -> Result<(), String> {
    require_directory(state)?;
    let body = state.join("body");
    require_directory(&body)?;
    let record = read(&body.join(PATH))?;
    if record.schema != SCHEMA
        || record.body_id != credential.body_id.as_str()
        || record.part_id != credential.part_id.as_str()
        || record.host_id != credential.host_id.as_str()
    {
        return Err("retained private key differs from returning member".into());
    }
    SpawnInvitationSecret::from_csprng_bytes(record.secret)
        .map_err(|_| "invalid retained continuity key".to_string())?;
    Ok(())
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
    // A crash may leave an earlier private staging file. A new bounded name
    // lets the exact pending proof retry without erasing that historical state.
    let identity = crate::durable_host::fresh_identity("member-continuity-stage", "private-key");
    let suffix = identity
        .rsplit('/')
        .next()
        .ok_or("continuity staging identity missing")?;
    let temporary = body.join(format!("member-continuity-key-{suffix}.tmp"));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&temporary)
        .map_err(|_| "create private continuity staging file failed")?;
    let mut staging = OwnedStaging {
        path: temporary,
        file,
    };
    let mut bytes =
        serde_json::to_vec(&record).map_err(|_| "encode private continuity key failed")?;
    if bytes.len() as u64 > MAX_BYTES {
        bytes.fill(0);
        return Err("private continuity key exceeds finite bound".into());
    }
    let result = staging
        .file
        .write_all(&bytes)
        .and_then(|()| staging.file.sync_all());
    bytes.fill(0);
    if result.is_err() {
        return Err("persist private continuity key failed".into());
    }
    // Atomic no-clobber publication: an existing file OR symlink refuses. A
    // failed membership commit leaves the pending request and this key intact.
    if fs::hard_link(&staging.path, &path).is_err() {
        return Err("commit private continuity key without overwrite failed".into());
    }
    drop(staging);

    sync_directory(&body)
}

/// Cleanup belongs only to this attempt's newly created inode. Crash leftovers
/// or a replaced pathname remain untouched; neither is evidence of final custody.
struct OwnedStaging {
    path: PathBuf,
    file: fs::File,
}
impl Drop for OwnedStaging {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let (Ok(owned), Ok(current)) =
                (self.file.metadata(), fs::symlink_metadata(&self.path))
            {
                if current.file_type().is_file()
                    && owned.dev() == current.dev()
                    && owned.ino() == current.ino()
                {
                    let _ = fs::remove_file(&self.path);
                }
            }
        }
        // Without a reviewed inode-identity check, preserve uncertain staging.
    }
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
    let read_result = file.take(MAX_BYTES + 1).read_to_end(&mut bytes);
    let result = if read_result.is_err() {
        Err("read private continuity record failed".into())
    } else if bytes.len() as u64 <= MAX_BYTES {
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
#[path = "continuity_key/tests.rs"]
mod tests;
