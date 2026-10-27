//! CPU prototype: durable remote biography custody, not wired to HTTP or signing.
use crate::{checked, Result};
use conduit_body::{BodyBiographyArchiveSegment, BodyId, MembershipCredential};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    os::fd::{AsRawFd, FromRawFd},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
};
#[path = "../../../../targets/esp32/proof/tm1637/src/native_biography_custody_wire.rs"]
pub(crate) mod wire;

pub(crate) struct VerifiedDurableBiography {
    root: [u8; 32],
    body: BodyId,
    offer: [u8; 32],
}
fn name(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
fn private(metadata: &std::fs::Metadata, directory: bool) -> Result<()> {
    // SAFETY: observes the process identity only.
    if metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err("biography custody requires private current-owner files/directories".into());
    }
    Ok(())
}
fn create_durable(directory: &Path) -> Result<()> {
    match std::fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err("biography directory symlink or type".into());
            }
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = directory
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    create_durable(parent)?;
    std::fs::DirBuilder::new().mode(0o700).create(directory)?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}
struct Directory(std::fs::File);
impl Directory {
    fn open(path: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(path)?;
        private(&file.metadata()?, true)?;
        Ok(Self(file))
    }
    fn file(&self, name: &str, create: bool) -> Result<std::fs::File> {
        if name.contains('/') || name == "." || name == ".." {
            return Err("biography file name".into());
        }
        let name = std::ffi::CString::new(name)?;
        let flags = libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if create {
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL
            } else {
                libc::O_RDONLY
            };
        // SAFETY: live owned directory descriptor and NUL-terminated filename. The
        // returned descriptor is adopted exactly once; failed descriptors never escape.
        let fd = unsafe { libc::openat(self.0.as_raw_fd(), name.as_ptr(), flags, 0o600) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: successful openat transfers a fresh descriptor to this File.
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        private(&file.metadata()?, false)?;
        Ok(file)
    }
    fn read(&self, name: &str) -> Result<Vec<u8>> {
        use std::io::Read;
        let mut file = self.file(name, false)?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(131073)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 131072 {
            return Err("biography file extent".into());
        }
        Ok(bytes)
    }
    fn immutable(&self, name: &str, bytes: &[u8]) -> Result<()> {
        match self.file(name, true) {
            Ok(mut file) => {
                file.write_all(bytes)?;
                file.flush()?;
                file.sync_all()?;
            }
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::AlreadyExists) => {}
            Err(error) => return Err(error),
        }
        if self.read(name)? != bytes {
            return Err("biography immutable file conflict".into());
        }
        self.file(name, false)?.sync_all()?;
        Ok(())
    }
    fn current(&self, path: &Path) -> Result<()> {
        let metadata = std::fs::symlink_metadata(path)?;
        private(&metadata, true)?;
        let held = self.0.metadata()?;
        if metadata.dev() != held.dev() || metadata.ino() != held.ino() {
            return Err("biography directory replaced".into());
        }
        Ok(())
    }
}
#[cfg(test)]
fn verify_files(directory: &Path, offer: &wire::Offer) -> Result<()> {
    verify_directory(&Directory::open(directory)?, offer)
}
fn verify_directory(directory: &Directory, offer: &wire::Offer) -> Result<()> {
    let mut ordinal = offer.end.count;
    let mut digest = offer.end.head.map(|v| v.0);
    while ordinal > 0 {
        let expected = digest.ok_or("biography missing predecessor")?;
        let segment: BodyBiographyArchiveSegment =
            serde_json::from_slice(&directory.read(&format!("{}.json", name(&expected)))?)?;
        checked(segment.validate())?;
        if segment.body_id != offer.body || segment.ordinal != ordinal || segment.digest != expected
        {
            return Err("biography retained chain".into());
        }
        if ordinal == offer.end.count || ordinal == offer.base.count {
            let mut summary = wire::Summary {
                count: ordinal - 1,
                head: segment.previous_digest.map(wire::Hex),
                last_sequence: 0,
                last_sign: None,
            };
            checked(summary.append(&offer.body, &segment))?;
            let required = if ordinal == offer.end.count {
                &offer.end
            } else {
                &offer.base
            };
            if &summary != required {
                return Err("biography retained summary".into());
            }
        }
        digest = segment.previous_digest;
        ordinal -= 1;
    }
    if digest.is_some() {
        return Err("biography chain has no origin".into());
    }
    Ok(())
}
impl VerifiedDurableBiography {
    /// Minted only after pinned Root authentication, exact stream validation,
    /// immutable file custody, fsync and independent complete-chain reopening.
    pub(crate) fn retain(
        offer: &wire::Offer,
        body: &BodyId,
        root: &[u8; 32],
        stream: &[u8],
        directory: &Path,
    ) -> Result<Self> {
        let key = checked(ed25519_dalek::VerifyingKey::from_bytes(root))?;
        let id = checked(offer.verify(body, &key))?;
        if stream.len() > 131072 || Sha256::digest(stream)[..] != offer.stream.0 {
            return Err("biography export extent or digest".into());
        }
        let segments: Vec<BodyBiographyArchiveSegment> = serde_json::from_slice(stream)?;
        let mut summary = offer.base.clone();
        for segment in &segments {
            checked(summary.append(body, segment))?;
        }
        if summary != offer.end {
            return Err("biography export end".into());
        }
        create_durable(directory)?;
        let custody = Directory::open(directory)?;
        // Partial failure leaves reusable immutable files, never an ACK token.
        for segment in &segments {
            custody.immutable(
                &format!("{}.json", name(&segment.digest)),
                &serde_json::to_vec(segment)?,
            )?;
        }
        custody.immutable(
            &format!("offer-{}.json", name(&id)),
            &serde_json::to_vec(offer)?,
        )?;
        custody.0.sync_all()?;
        verify_directory(&custody, offer)?;
        // Manifest itself must reopen and still authenticate after the fsync.
        let reopened: wire::Offer =
            serde_json::from_slice(&custody.read(&format!("offer-{}.json", name(&id)))?)?;
        if checked(reopened.verify(body, &key))? != id {
            return Err("biography manifest changed".into());
        }
        verify_directory(&custody, &reopened)?;
        custody.current(directory)?;
        Ok(Self {
            root: *root,
            body: body.clone(),
            offer: id,
        })
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn body(&self) -> &BodyId {
        &self.body
    }
    fn request(&self, credential: &MembershipCredential) -> Result<wire::Request> {
        if credential.body_id != self.body {
            return Err("biography credential Body differs".into());
        }
        Ok(wire::Request {
            schema: wire::ACK.into(),
            offer: wire::Hex(self.offer),
            part: credential.part_id.as_str().into(),
            host: credential.host_id.as_str().into(),
            boot: credential.boot_id.as_str().into(),
            signature: wire::Hex([0; 64]),
        })
    }
    pub(crate) fn signing_transcript(&self, credential: &MembershipCredential) -> Result<[u8; 32]> {
        checked(self.request(credential)?.transcript())
    }
    pub(crate) fn encode_ack(
        &self,
        credential: &MembershipCredential,
        signature: [u8; 64],
    ) -> Result<Vec<u8>> {
        let mut request = self.request(credential)?;
        checked(request.transcript())?;
        request.signature = wire::Hex(signature);
        let bytes = serde_json::to_vec(&request)?;
        if bytes.len() > 1024 {
            return Err("biography ACK extent".into());
        }
        Ok(bytes)
    }
}
#[cfg(test)]
#[path = "biography_custody_remote_tests.rs"]
mod tests;

/// Only a durable token can create this signing draft. No arbitrary digest API.
pub(crate) struct OwnerCustodyDraft {
    request: crate::native_body_join::owner::BiographyRequest,
}
impl VerifiedDurableBiography {
    pub(crate) fn owner_draft(
        &self,
        challenge: crate::native_body_join::owner::BiographyChallenge,
    ) -> Result<OwnerCustodyDraft> {
        if challenge.body != self.body.as_str()
            || challenge.root_key != self.root
            || challenge.offer != self.offer
        {
            return Err("biography owner challenge differs".into());
        }
        let request = crate::native_body_join::owner::BiographyRequest {
            schema: crate::native_body_join::owner::BIOGRAPHY_PROFILE.into(),
            challenge,
            signature: Vec::new(),
        };
        checked(request.transcript())?;
        Ok(OwnerCustodyDraft { request })
    }
}
impl OwnerCustodyDraft {
    pub(crate) fn signing_transcript(&self) -> Result<[u8; 32]> {
        checked(self.request.transcript())
    }
    pub(crate) fn encode(mut self, signature: [u8; 64]) -> Result<Vec<u8>> {
        self.request.signature = signature.to_vec();
        let bytes = serde_json::to_vec(&self.request)?;
        if bytes.len() > 1024 {
            return Err("biography owner ACK extent".into());
        }
        Ok(bytes)
    }
}
