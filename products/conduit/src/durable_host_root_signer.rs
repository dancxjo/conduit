//! Explicit installed authoritative receipt identity; never a Host key or control token.
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};
const PRIVATE: &str = "body/root-signer-private.json";
const SELECTED: &str = "body/root-signer.json";
const MAXIMUM: u64 = 1024;
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    schema: String,
    pub(crate) body_id: String,
    pub(crate) owner_host_id: String,
    pub(crate) public_key: [u8; 32],
    private_record_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Private {
    schema: String,
    body_id: String,
    owner_host_id: String,
    public_key: [u8; 32],
    seed: [u8; 32],
}
impl Drop for Private {
    fn drop(&mut self) {
        self.seed.fill(0);
    }
}
fn validate_scope(body: &str, host: &str) -> Result<(), String> {
    if body.is_empty() || host.is_empty() || body.len() > 256 || host.len() > 256 {
        return Err("invalid installed Root signer scope".into());
    }
    Ok(())
}
fn read_private(root: &Path, body: &str, host: &str) -> Result<(Binding, SigningKey), String> {
    validate_scope(body, host)?;
    let path = root.join(PRIVATE);
    let metadata =
        fs::symlink_metadata(&path).map_err(|e| format!("Root signer unavailable: {e}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Root signer is not a private regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("Root signer permissions expose private material".into());
        }
    }
    let mut bytes = crate::durable_host::bounded_read(&path, MAXIMUM)?;
    let digest = crate::durable_host::digest(&bytes);
    let parsed = serde_json::from_slice::<Private>(&bytes);
    bytes.fill(0);
    let private = parsed.map_err(|_| "invalid private Root signer record")?;
    let key = SigningKey::from_bytes(&private.seed);
    if private.schema != "conduit.body/installed-root-signer-private@1"
        || private.body_id != body
        || private.owner_host_id != host
        || private.seed == [0; 32]
        || private.public_key != key.verifying_key().to_bytes()
    {
        return Err("private Root signer differs from installed Body/owner".into());
    }
    Ok((
        Binding {
            schema: "conduit.body/installed-root-signer@1".into(),
            body_id: body.into(),
            owner_host_id: host.into(),
            public_key: private.public_key,
            private_record_sha256: digest,
        },
        key,
    ))
}
pub(crate) fn validate(
    root: &Path,
    binding: &Binding,
    body: &str,
    host: &str,
) -> Result<(), String> {
    let (actual, _) = read_private(root, body, host)?;
    if &actual != binding {
        return Err("selected Root signer binding differs from retained private custody".into());
    }
    Ok(())
}
pub(crate) fn selected(root: &Path, body: &str, host: &str) -> Result<Option<Binding>, String> {
    let path = root.join(SELECTED);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
            return Err("invalid selected Root signer file".into())
        }
        Ok(_) => {}
    }
    let binding: Binding =
        serde_json::from_slice(&crate::durable_host::bounded_read(&path, MAXIMUM)?)
            .map_err(|_| "invalid selected Root signer binding")?;
    validate(root, &binding, body, host)?;
    Ok(Some(binding))
}
/// Public provenance only; selected custody is verified without exporting its
/// private-record digest, seed, or private path. Never establish implicitly.
pub(crate) fn public_authority(
    root: &Path,
    body: &str,
    host: &str,
) -> Result<conduit_body::BodyRootAuthority, String> {
    let binding = selected(root, body, host)?
        .ok_or("browser Spore preparation requires the selected Body Root signer")?;
    let authority = conduit_body::BodyRootAuthority {
        schema: "conduit.body/root-authority@1".into(),
        body_id: binding.body_id.into(),
        owner_host_id: binding.owner_host_id.into(),
        public_key: binding.public_key,
    };
    authority
        .validate()
        .map_err(|e| format!("invalid selected Root authority: {e:?}"))?;
    Ok(authority)
}

/// Prepare private custody only. Caller must publish selection through the owner journal.
/// An interrupted unselected private file is reused only after exact scope validation.
pub(crate) fn prepare(root: &Path, body: &str, host: &str) -> Result<Binding, String> {
    if let Some(binding) = selected(root, body, host)? {
        return Ok(binding);
    }
    validate_scope(body, host)?;
    let directory = root.join("body");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    crate::durable_host::restrict_directory(&directory)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(root.join(PRIVATE)) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return read_private(root, body, host).map(|v| v.0)
        }
        Err(e) => return Err(e.to_string()),
        Ok(mut file) => {
            let mut seed = [0; 32];
            if let Err(e) = getrandom::fill(&mut seed) {
                seed.fill(0);
                return Err(e.to_string());
            }
            if seed == [0; 32] {
                return Err("Root signer entropy refusal".into());
            }
            let key = SigningKey::from_bytes(&seed);
            let private = Private {
                schema: "conduit.body/installed-root-signer-private@1".into(),
                body_id: body.into(),
                owner_host_id: host.into(),
                public_key: key.verifying_key().to_bytes(),
                seed,
            };
            seed.fill(0);
            let mut bytes =
                serde_json::to_vec(&private).map_err(|_| "encode private Root signer")?;
            if bytes.len() as u64 > MAXIMUM {
                bytes.fill(0);
                return Err("private Root signer record exceeds finite bound".into());
            }
            let result = file.write_all(&bytes).and_then(|()| file.sync_all());
            bytes.fill(0);
            result.map_err(|e| e.to_string())?;
            fs::File::open(directory)
                .and_then(|d| d.sync_all())
                .map_err(|e| e.to_string())?;
        }
    }
    read_private(root, body, host).map(|v| v.0)
}
pub(crate) fn commit(root: &Path, binding: &Binding, body: &str, host: &str) -> Result<(), String> {
    validate(root, binding, body, host)?;
    if let Some(previous) = selected(root, body, host)? {
        if previous != *binding {
            return Err("Root signer replacement refused".into());
        }
    }
    crate::durable_host::write_json_atomic(&root.join(SELECTED), binding)
}
#[cfg(all(test, unix))]
#[path = "durable_host_root_signer/tests.rs"]
mod tests;

#[path = "durable_host_root_signer/receipt.rs"]
pub(crate) mod receipt;
