//! Durable custody of one exact Root-signed network retirement snapshot.
//! No transport or erase authority is provided by this module.
use crate::{checked, Result};
use conduit_body::{BodyId, MembershipCredential};
use conduit_core::BootId;
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
const CAPACITY: usize = 32768;
const OFFER: usize = 240;
const REQUEST: usize = 628;
#[derive(Clone)]
pub(crate) struct Scope {
    pub body: BodyId,
    pub root: [u8; 32],
    pub producer_boot: BootId,
    pub epoch: u64,
    pub region: [u8; 32],
    pub predecessor: [u8; 32],
}
pub(crate) struct VerifiedDurableSnapshot {
    scope: Scope,
    directory: PathBuf,
    offer: [u8; 32],
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn offer_id(root: &[u8; 32], offer: &[u8; OFFER]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"conduit.environment/network-retirement-offer@1\0");
    h.update(root);
    h.update(&offer[..176]);
    h.finalize().into()
}
fn verify_offer(scope: &Scope, offer: &[u8; OFFER]) -> Result<[u8; 32]> {
    if offer[..32] != digest(scope.body.as_str().as_bytes())
        || offer[32..64] != scope.region
        || offer[64..72] != scope.epoch.to_le_bytes()
        || scope.epoch == u64::MAX
        || offer[72..104] != digest(scope.producer_boot.as_str().as_bytes())
        || offer[104..108] != (CAPACITY as u32).to_le_bytes()
        || !(1..=8).contains(&offer[108])
        || offer[109..112] != [0; 3]
        || offer[144..176] != scope.predecessor
    {
        return Err("network custody snapshot scope/extent differs".into());
    }
    let id = offer_id(&scope.root, offer);
    checked(VerifyingKey::from_bytes(&scope.root))?
        .verify_strict(
            &id,
            &Signature::from_bytes(offer[176..].try_into().unwrap()),
        )
        .map_err(|_| "network custody Root signature")?;
    Ok(id)
}
fn verify(scope: &Scope, offer: &[u8; OFFER], snapshot: &[u8]) -> Result<[u8; 32]> {
    let id = verify_offer(scope, offer)?;
    if snapshot.len() != CAPACITY || offer[112..144] != digest(snapshot) {
        return Err("network custody snapshot extent/digest differs".into());
    }
    Ok(id)
}
fn private(path: &Path, directory: bool) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    // SAFETY: reads the effective identity without changing process state.
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || (directory && !m.is_dir())
        || (!directory && !m.is_file())
    {
        return Err("network custody requires private owned files/directories".into());
    }
    Ok(())
}
fn reopen(path: &Path, bound: usize) -> Result<Vec<u8>> {
    private(path, false)?;
    let mut bytes = Vec::new();
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?
        .take((bound + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() != bound {
        return Err("network custody durable extent".into());
    }
    Ok(bytes)
}
fn immutable(path: &Path, bytes: &[u8]) -> Result<()> {
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if reopen(path, bytes.len())? != bytes {
                return Err("network custody immutable file differs".into());
            }
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)?
                .sync_all()?;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
impl VerifiedDurableSnapshot {
    pub(crate) fn retain(
        parent: &Path,
        offer: &[u8; OFFER],
        snapshot: &[u8],
        scope: &Scope,
    ) -> Result<Self> {
        let id = verify(scope, offer, snapshot)?;
        private(parent, true)?;
        let name = id.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let directory = parent.join(format!("network-{name}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        private(&directory, true)?;
        immutable(&directory.join("offer.bin"), offer)?;
        immutable(&directory.join("snapshot.bin"), snapshot)?;
        File::open(&directory)?.sync_all()?;
        File::open(parent)?.sync_all()?;
        let token = Self {
            scope: scope.clone(),
            directory,
            offer: id,
        };
        token.revalidate()?;
        Ok(token)
    }
    fn revalidate(&self) -> Result<()> {
        private(&self.directory, true)?;
        let offer = reopen(&self.directory.join("offer.bin"), OFFER)?;
        let snapshot = reopen(&self.directory.join("snapshot.bin"), CAPACITY)?;
        if verify(&self.scope, offer.as_slice().try_into().unwrap(), &snapshot)? != self.offer {
            return Err("network custody retained offer differs".into());
        }
        Ok(())
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.scope.root
    }
    pub(crate) fn body_digest(&self) -> [u8; 32] {
        digest(self.scope.body.as_str().as_bytes())
    }
    fn member_scope(&self, credential: &MembershipCredential) -> Result<[u8; 32]> {
        self.encode_fields(credential, [0; 64])?;
        let mut h = Sha256::new();
        h.update(b"conduit.environment/network-retirement-member@1\0");
        for id in [
            credential.body_id.as_str(),
            credential.part_id.as_str(),
            credential.host_id.as_str(),
            credential.boot_id.as_str(),
            credential.credential_id.as_str(),
        ] {
            h.update((id.len() as u64).to_le_bytes());
            h.update(id.as_bytes());
        }
        h.update(credential.issued_at_millis.to_le_bytes());
        Ok(h.finalize().into())
    }
    pub(crate) fn signing_transcript(&self, credential: &MembershipCredential) -> Result<[u8; 32]> {
        self.revalidate()?;
        let scope = self.member_scope(credential)?;
        let mut h = Sha256::new();
        h.update(b"conduit.environment/network-retirement-custody@1\0");
        h.update(self.offer);
        h.update(scope);
        Ok(h.finalize().into())
    }
    fn encode_fields(
        &self,
        c: &MembershipCredential,
        signature: [u8; 64],
    ) -> Result<[u8; REQUEST]> {
        if c.body_id != self.scope.body {
            return Err("network custody credential Body differs".into());
        }
        let mut bytes = [0; REQUEST];
        bytes[..8].copy_from_slice(b"NCACK1\0\0");
        bytes[8..40].copy_from_slice(&self.offer);
        bytes[40..48].copy_from_slice(&c.issued_at_millis.to_le_bytes());
        for (i, id) in [
            c.part_id.as_str(),
            c.host_id.as_str(),
            c.boot_id.as_str(),
            c.credential_id.as_str(),
        ]
        .iter()
        .enumerate()
        {
            if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
                return Err("network custody identity extent".into());
            }
            bytes[48 + i] = id.len() as u8;
            bytes[52 + i * 128..52 + i * 128 + id.len()].copy_from_slice(id.as_bytes());
        }
        bytes[564..].copy_from_slice(&signature);
        Ok(bytes)
    }
    pub(crate) fn encode_ack(
        &self,
        c: &MembershipCredential,
        signature: [u8; 64],
    ) -> Result<Vec<u8>> {
        self.revalidate()?;
        Ok(self.encode_fields(c, signature)?.to_vec())
    }
}
use std::os::unix::fs::DirBuilderExt;
#[cfg(test)]
#[path = "network_custody_client/tests.rs"]
mod tests;

#[path = "network_custody_client/receipt.rs"]
mod receipt;

#[path = "network_custody_client/live.rs"]
pub(crate) mod live;

#[path = "network_custody_client/resume.rs"]
pub(crate) mod resume;
