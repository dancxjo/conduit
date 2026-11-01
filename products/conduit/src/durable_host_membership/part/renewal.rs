//! One bounded credential transaction. Installation is the commit point.
use super::*;
const FILE: &str = "native-part-renewal.json";
const SCHEMA: &str = "conduit.native/part-renewal@1";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Renewal {
    schema: String,
    original_sha256: String,
    current: MembershipCredential,
    current_path: String,
    prepared: Option<Prepared>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    previous: MembershipCredential,
    next: MembershipCredential,
    credential_path: String,
    credential_sha256: String,
}
type Lock = crate::durable_host::installation_lock::InstallationLock;
fn private_file(file: &fs::File) -> Result<(), String> {
    let m = file.metadata().map_err(|_| "native renewal metadata")?;
    if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o777 != 0o600 {
        return Err("native renewal requires private regular files".into());
    }
    Ok(())
}
fn original(state: &Path) -> Result<String, String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(state.join("body/native-part-private.json"))
        .map_err(|_| "native original custody unavailable")?;
    private_file(&file)?;
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "native original read")?;
    let result = if bytes.is_empty() || bytes.len() as u64 > LIMIT {
        Err("native original bound".into())
    } else {
        Ok(digest(&bytes))
    };
    bytes.fill(0);
    result
}
fn identity(a: &MembershipCredential, b: &MembershipCredential) -> bool {
    a.body_id == b.body_id && a.part_id == b.part_id && a.host_id == b.host_id
}
fn read(state: &Path, record: &Record) -> Result<Renewal, String> {
    super::verify_exact(
        state,
        &record.pending,
        &record.receipt,
        record.receipt.credential.host_id.as_str(),
    )?;
    let path = state.join("body").join(FILE);
    let anchor = original(state)?;
    if !path.try_exists().map_err(|_| "native renewal inspection")? {
        // A dangling symlink is still conflicting custody, never an absent record.
        if path.symlink_metadata().is_ok() {
            return Err("native renewal symlink refused".into());
        }
        return Ok(Renewal {
            schema: SCHEMA.into(),
            original_sha256: anchor,
            current: record.receipt.credential.clone(),
            current_path: state
                .join("body/membership-credential.json")
                .display()
                .to_string(),
            prepared: None,
        });
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "native renewal unavailable")?;
    private_file(&file)?;
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "native renewal read")?;
    if bytes.is_empty() || bytes.len() > 64 * 1024 {
        return Err("native renewal bound".into());
    }
    let value: Renewal = serde_json::from_slice(&bytes).map_err(|_| "native renewal encoding")?;
    if value.schema != SCHEMA
        || value.original_sha256 != anchor
        || !identity(&value.current, &record.receipt.credential)
    {
        return Err("native renewal provenance mismatch".into());
    }
    let current_path = if value.current == record.receipt.credential {
        state.join("body/membership-credential.json")
    } else {
        public_path(state, &value.current)
    };
    if value.current_path != current_path.to_string_lossy() {
        return Err("native renewal current reference mismatch".into());
    }
    if let Some(p) = &value.prepared {
        if p.previous != value.current
            || !identity(&p.next, &value.current)
            || p.next.credential_id == p.previous.credential_id
        {
            return Err("native renewal prepared identity mismatch".into());
        }
        let expected = public_path(state, &p.next);
        if p.credential_path != expected.to_string_lossy()
            || p.credential_sha256 != digest(&encoded(&p.next)?)
        {
            return Err("native renewal prepared reference mismatch".into());
        }
    }
    Ok(value)
}
fn encoded(c: &MembershipCredential) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(c).map_err(|_| "native credential encoding".into())
}
fn public_path(state: &Path, c: &MembershipCredential) -> std::path::PathBuf {
    // Digest-derived basename cannot contain caller-controlled path syntax.
    state.join("body").join(format!(
        "membership-{}.json",
        digest(c.credential_id.as_str().as_bytes())
    ))
}
fn write_private(state: &Path, value: &Renewal) -> Result<(), String> {
    let path = state.join("body").join(FILE);
    // Unique private staging avoids following a pre-existing helper .tmp file.
    let staging = state
        .join("body")
        .join(format!("renewal-{}", crate::birth_identity::fresh_uuid()?));
    let temporary = staging.with_extension("tmp");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| "native renewal private staging")?;
    let result = (|| {
        super::super::super::write_bytes_atomic(
            &staging,
            &serde_json::to_vec(value).map_err(|_| "native renewal encoding")?,
        )?;
        fs::rename(&staging, &path).map_err(|_| "native renewal publication")?;
        sync_parent(&path)
    })();
    let _ = fs::remove_file(staging);
    let _ = fs::remove_file(temporary);
    result
}
fn sync_parent(path: &Path) -> Result<(), String> {
    fs::File::open(path.parent().ok_or("native renewal parent")?)
        .and_then(|d| d.sync_all())
        .map_err(|_| "native renewal directory sync".into())
}
fn binding_matches(
    b: &super::super::JoinedBodyBinding,
    c: &MembershipCredential,
    path: Option<&str>,
) -> Result<bool, String> {
    Ok(b.body_id == c.body_id.as_str()
        && b.part_id == c.part_id.as_str()
        && b.credential_sha256 == digest(&encoded(c)?)
        && path.map_or(true, |p| b.credential_path == p))
}
/// Reconciliation never treats a prepared value as selected unless installation
/// already names its exact versioned public bytes and hash.
pub(super) fn reconcile(
    state: &Path,
    record: &Record,
    installation: &Installation,
    credential: &MembershipCredential,
) -> Result<MembershipCredential, String> {
    let _lock = Lock::prepare(state)?;
    let renewal = reconcile_locked(state, record, installation, credential)?;
    if let Some(prepared) = renewal.prepared {
        // This exact next value was durably prepared by a verified protected
        // Return before the crash. Resume its publication, not a new Return.
        transaction_locked(
            state,
            record,
            &prepared.previous,
            &prepared.next,
            |_| Ok(()),
        )?;
        return Ok(prepared.next);
    }
    Ok(renewal.current)
}

fn reconcile_locked(
    state: &Path,
    record: &Record,
    installation: &Installation,
    credential: &MembershipCredential,
) -> Result<Renewal, String> {
    let mut renewal = read(state, record)?;
    let binding = installation
        .joined_body_state
        .as_ref()
        .ok_or("native renewal has no installed Part")?;
    if *credential == renewal.current
        && binding_matches(binding, credential, Some(&renewal.current_path))?
    {
        return Ok(renewal);
    }
    if let Some(p) = &renewal.prepared {
        if *credential == p.next && binding_matches(binding, credential, Some(&p.credential_path))?
        {
            renewal.current = p.next.clone();
            renewal.current_path = p.credential_path.clone();
            renewal.prepared = None;
            write_private(state, &renewal)?;
            return Ok(renewal);
        }
    }
    Err("installed credential conflicts with private native renewal".into())
}
pub(super) fn commit(
    part: &RetainedPart,
    proof: &crate::host_rendezvous::VerifiedReturn,
) -> Result<(), String> {
    if proof.previous() != part.credential()
        || proof.advertisement() != part.advertisement()
        || !identity(proof.previous(), proof.credential())
        || proof.credential().boot_id != part.advertisement().boot_id
    {
        return Err("native Return evidence differs from retained Part".into());
    }
    if crate::durable_host_control::current(&part.state)?.advertisement != part.advertisement {
        return Err("native Return installed Boot changed".into());
    }
    transaction(
        &part.state,
        &part.record,
        proof.previous(),
        proof.credential(),
        |_| Ok(()),
    )
}
fn transaction(
    state: &Path,
    record: &Record,
    previous: &MembershipCredential,
    next: &MembershipCredential,
    boundary: impl FnMut(u8) -> Result<(), String>,
) -> Result<(), String> {
    let _lock = Lock::prepare(state)?;
    transaction_locked(state, record, previous, next, boundary)
}
fn transaction_locked(
    state: &Path,
    record: &Record,
    previous: &MembershipCredential,
    next: &MembershipCredential,
    mut boundary: impl FnMut(u8) -> Result<(), String>,
) -> Result<(), String> {
    let install_path = state.join("installation.json");
    let before = bounded_read(&install_path, 1024 * 1024)?;
    let mut installation = read_installation(&install_path)?;
    let binding = installation
        .joined_body_state
        .as_ref()
        .ok_or("native renewal missing installed Part")?;
    super::super::validate(binding, &installation)?;
    let public: MembershipCredential = serde_json::from_slice(&bounded_read(
        Path::new(&binding.credential_path),
        64 * 1024,
    )?)
    .map_err(|_| "native credential encoding")?;
    let mut renewal = reconcile_locked(state, record, &installation, &public)?;
    if renewal.current == *next {
        return Ok(());
    }
    if renewal.current != *previous {
        return Err("native renewal current credential changed".into());
    }
    let prepared = Prepared {
        previous: renewal.current.clone(),
        next: next.clone(),
        credential_path: public_path(state, next).display().to_string(),
        credential_sha256: digest(&encoded(next)?),
    };
    if let Some(existing) = &renewal.prepared {
        if serde_json::to_vec(existing).unwrap() != serde_json::to_vec(&prepared).unwrap() {
            return Err("conflicting prepared native Return".into());
        }
    }
    renewal.prepared = Some(prepared.clone());
    write_private(state, &renewal)?;
    boundary(1)?;
    let path = Path::new(&prepared.credential_path);
    let bytes = encoded(&prepared.next)?;
    if path.symlink_metadata().is_ok() {
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| "native public credential conflict")?;
        private_file(&file)?;
        if bounded_read(path, 64 * 1024)? != bytes {
            return Err("native public credential conflict".into());
        }
    } else {
        let staging = state.join("body").join(format!(
            "credential-{}",
            crate::birth_identity::fresh_uuid()?
        ));
        let temporary = staging.with_extension("tmp");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| "native credential staging")?;
        let result = (|| {
            super::super::super::write_bytes_atomic(&staging, &bytes)?;
            fs::hard_link(&staging, path).map_err(|_| "native credential exclusive publication")?;
            sync_parent(path)
        })();
        let _ = fs::remove_file(staging);
        let _ = fs::remove_file(temporary);
        result?;
    }
    boundary(2)?;
    // Compare the complete installation document, preserving unrelated fields.
    if bounded_read(&install_path, 1024 * 1024)? != before {
        return Err("installation changed during native renewal".into());
    }
    let selected = installation.joined_body_state.as_mut().unwrap();
    selected.credential_path = prepared.credential_path.clone();
    selected.credential_sha256 = prepared.credential_sha256.clone();
    let stage = state.join(format!(
        "installation-renewal-{}",
        crate::birth_identity::fresh_uuid()?
    ));
    let temporary = stage.with_extension("tmp");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| "native installation staging")?;
    let published = (|| {
        super::super::super::write_json_atomic(&stage, &installation)?;
        if bounded_read(&install_path, 1024 * 1024)? != before {
            return Err("installation changed before native renewal commit".into());
        }
        fs::rename(&stage, &install_path).map_err(|_| "native installation publication")?;
        sync_parent(&install_path)
    })();
    let _ = fs::remove_file(stage);
    let _ = fs::remove_file(temporary);
    published?;
    boundary(3)?;
    renewal.current_path = prepared.credential_path.clone();
    renewal.current = prepared.next;
    renewal.prepared = None;
    write_private(state, &renewal)?;
    boundary(4)
}

#[cfg(test)]
#[path = "renewal/tests.rs"]
mod tests;

pub(super) fn public_credential(path: &Path) -> Result<MembershipCredential, String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "native public credential unavailable")?;
    let m = file
        .metadata()
        .map_err(|_| "native public credential metadata")?;
    if !m.is_file() || m.uid() != unsafe { libc::geteuid() } || m.len() == 0 || m.len() > 64 * 1024
    {
        return Err("native public credential bound or owner".into());
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "native public credential read")?;
    serde_json::from_slice(&bytes).map_err(|_| "native public credential encoding".into())
}
