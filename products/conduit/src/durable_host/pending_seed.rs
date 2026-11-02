//! Private original Seed invitation, retained before the installed Host can Boot.
//!
//! Installation is not membership authority. The explicit request entrance uses
//! the existing signed current-Host/current-Boot admission protocol; an owner's
//! separately authorized admission and local completion are still required.
use super::*;
use conduit_body::PortableInvitation;

const PATH: &str = "pending-seed.json";
const SCHEMA: &str = "conduit.install/pending-seed@1";

#[derive(Serialize, Deserialize)]
struct PendingSeed {
    schema: String,
    invitation: PortableInvitation,
}
impl Drop for PendingSeed {
    fn drop(&mut self) {
        self.invitation.secret.fill(0);
    }
}

fn read(root: &Path) -> Result<PendingSeed, String> {
    let mut bytes = bounded_read(&root.join(PATH), 64 * 1024)?;
    let decoded = serde_json::from_slice::<PendingSeed>(&bytes);
    bytes.fill(0);
    let pending = decoded.map_err(|_| "invalid private pending Seed provision")?;
    if pending.schema != SCHEMA {
        return Err("unsupported pending Seed provision".into());
    }
    // Expiry may prevent requesting admission, but must never turn a Seed into
    // an unrelated Body Birth. Preserve that distinction across delayed Boots.
    pending
        .invitation
        .validate(0)
        .map_err(|error| error.to_string())?;
    Ok(pending)
}

pub(crate) fn has_pending_seed(root: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(root.join(PATH)) {
        Ok(metadata) if metadata.file_type().is_file() => {
            read(root)?;
            Ok(true)
        }
        Ok(_) => Err("pending Seed provision is not a regular private file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("inspect pending Seed: {error}")),
    }
}

pub(crate) fn install_pending_seed(
    manifest: &Path,
    root: &Path,
    invitation: &PortableInvitation,
    start: bool,
) -> Result<InstalledHostIdentity, String> {
    invitation.validate(0).map_err(|error| error.to_string())?;
    // Never inherit a previously installed Host identity or Body credential.
    // A failed installation remains fenced, for explicit recovery, not Birth.
    fs::create_dir(root).map_err(|_| "Seed requires a fresh destination state directory")?;
    restrict_directory(root)?;
    let pending = PendingSeed {
        schema: SCHEMA.into(),
        invitation: invitation.clone(),
    };
    let mut bytes = conduit_body_make::encode_joining_host_evidence(&pending, 64 * 1024)?;
    let result = (|| {
        use std::io::Write;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(root.join(PATH))
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        #[cfg(unix)]
        fs::File::open(root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())?;
        let installed = install_without_start(manifest, root)?;
        if start {
            activate_service(root)?;
        }
        Ok(installed)
    })();
    bytes.fill(0);
    result
}

pub(crate) fn accept_pending_seed(root: &Path, authorize_join: bool) -> Result<(), String> {
    let request = prepare_pending_seed_request(root, authorize_join)?;
    println!(
        "{}",
        serde_json::to_string(&request).map_err(|error| error.to_string())?
    );
    Ok(())
}

pub(super) fn prepare_pending_seed_request(
    root: &Path,
    authorize_join: bool,
) -> Result<conduit_body::PortableSpawnAdmissionRequest, String> {
    if !authorize_join {
        return Err("accepting a Seed requires --authorize-join".into());
    }
    let pending = read(root)?;
    invitation::prepare_body_join(pending.invitation.clone(), root, authorize_join)
}

pub(super) fn validate_invitation(
    root: &Path,
    invitation: &PortableInvitation,
) -> Result<(), String> {
    if !has_pending_seed(root)? {
        return Ok(());
    }
    let pending = read(root)?;
    if pending.invitation.schema != invitation.schema
        || pending.invitation.claim != invitation.claim
        || pending.invitation.secret != invitation.secret
        || pending.invitation.rendezvous != invitation.rendezvous
    {
        return Err("request must use the installed Seed's exact original invitation".into());
    }
    Ok(())
}

pub(super) fn validate_completion(root: &Path, body_id: &str) -> Result<(), String> {
    if !has_pending_seed(root)? {
        return Ok(());
    }
    let pending = read(root)?;
    if pending.invitation.claim.body_id.as_str() != body_id {
        return Err("membership receipt differs from installed Seed Body".into());
    }
    Ok(())
}

pub(super) fn complete(root: &Path, body_id: &str) -> Result<(), String> {
    validate_completion(root, body_id)?;
    if has_pending_seed(root)? {
        fs::remove_file(root.join(PATH))
            .map_err(|error| format!("consume pending Seed: {error}"))?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "pending_seed/tests.rs"]
mod tests;
