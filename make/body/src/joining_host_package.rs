//! Produce the existing stored native ZIP from exact release bytes and one
//! owner-issued invitation. This grants no admission and performs no transport.
//! The caller owns authorized source opening and private staging, discards all
//! partial output on error, and validates target-specific IMAGE semantics first.
use crate::{BuiltSpore, SporeBinding, SporeManifest, SPORE_MANIFEST_SCHEMA};
use conduit_body::PortableInvitation;
use conduit_host_make::{SporeOutputKind, VerifiedHostReleaseBytes};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};

const MAX_FILES: usize = 17;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const PROVISION: &str = "conduit-spore.json";
const EXECUTABLE: u32 = 0o100755;
const DATA: u32 = 0o100644;

/// Raw ZIP identity is deliberately distinct from the release bundle aggregate.
/// No secret, credential, invitation or admission receipt is retained here.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoiningHostPackageReceipt {
    pub artifact_bytes: u64,
    pub artifact_content_sha256: String,
    pub image_content_sha256: String,
    pub files: Vec<JoiningHostPackageFile>,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoiningHostPackageFile {
    pub path: String,
    pub bytes: u64,
    pub mode: u32,
}

#[derive(Serialize)]
struct Provision<'a> {
    schema: &'static str,
    spore: &'a SporeManifest,
    release_manifest: &'a conduit_host_make::HostReleaseManifest,
    invitation_provision: Invitation<'a>,
    portable_invitation: &'a PortableInvitation,
}
#[derive(Serialize)]
struct Invitation<'a> {
    invitation_id: &'a str,
    nonce: &'a [u8; 32],
    expires_at_millis: u64,
    secret: &'a [u8; 32],
    rendezvous: &'a Option<conduit_body::SpawnRendezvousDescriptor>,
    // Legacy candidate envelopes are independent; never derive or fabricate them.
    rendezvous_candidates: [u8; 0],
}
struct SecretBytes(Vec<u8>);
impl Write for SecretBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > 64 * 1024)
        {
            return Err(std::io::Error::other("native ZIP provision exceeds bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
struct Entry {
    path: String,
    bytes: u64,
    mode: u32,
    crc: u32,
    offset: u32,
}

/// `release` is the generic verifier's opaque evidence, never a digest-only
/// receipt. Each exact reviewed payload is opened, verified, rewound and then
/// verified again while writing. Source changes, short/long reads and cancelled
/// work yield no receipt. Output is private incomplete staging until success.
#[allow(clippy::too_many_arguments)]
pub fn write_joining_host_package<R: Read + Seek>(
    release: &VerifiedHostReleaseBytes,
    built: &BuiltSpore,
    invitation: &PortableInvitation,
    now_millis: u64,
    mut open: impl FnMut(&str) -> Result<R, String>,
    output: &mut impl Write,
    mut cancelled: impl FnMut() -> bool,
) -> Result<JoiningHostPackageReceipt, String> {
    check_binding(release, built, invitation, now_millis)?;
    let spore = &built.manifest;
    check_cancel(&mut cancelled)?;
    let mut provision = SecretBytes(Vec::new());
    serde_json::to_writer(
        &mut provision,
        &Provision {
            schema: "conduit.spore/native-package-provision@1",
            spore,
            release_manifest: release.manifest(),
            portable_invitation: invitation,
            invitation_provision: Invitation {
                invitation_id: invitation.claim.invitation_id.as_str(),
                nonce: &invitation.claim.nonce,
                expires_at_millis: invitation.claim.expires_at_millis,
                secret: &invitation.secret,
                rendezvous: &invitation.rendezvous,
                rendezvous_candidates: [],
            },
        },
    )
    .map_err(|_| "native ZIP provision encoding failed or exceeded bound")?;
    // Admit every header and directory byte before producing any output.
    let mut total = 22u64;
    for (path, bytes) in release
        .manifest()
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.bytes))
        .chain([(PROVISION, provision.0.len() as u64)])
    {
        if !safe_path(path) || path.len() > u16::MAX as usize {
            return Err("native ZIP requires bounded flat safe paths".into());
        }
        total = total
            .checked_add(76 + 2 * path.len() as u64)
            .and_then(|n| n.checked_add(bytes))
            .filter(|n| *n <= MAX_BYTES)
            .ok_or("native ZIP exceeds admitted byte bound")?;
    }
    let mut writer = PackageWriter {
        output,
        hash: Sha256::new(),
        written: 0,
    };
    let mut entries = Vec::with_capacity(release.manifest().files.len() + 1);
    for file in &release.manifest().files {
        check_cancel(&mut cancelled)?;
        let mut reader = open(&file.path)?;
        let crc = read_payload(
            &mut reader,
            file.bytes,
            &file.sha256,
            &mut cancelled,
            |_| Ok(()),
        )?;
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|_| "native ZIP payload rewind failed")?;
        let mode = match file.media_type.as_str() {
            "application/vnd.conduit.host+executable"
            | "application/vnd.microsoft.portable-executable" => EXECUTABLE,
            _ => DATA,
        };
        let entry = Entry {
            path: file.path.clone(),
            bytes: file.bytes,
            mode,
            crc,
            offset: writer.written as u32,
        };
        writer.local(&entry)?;
        let written_crc = read_payload(
            &mut reader,
            file.bytes,
            &file.sha256,
            &mut cancelled,
            |chunk| writer.write(chunk),
        )?;
        if written_crc != crc {
            return Err("native ZIP payload changed after verification".into());
        }
        entries.push(entry);
    }
    check_cancel(&mut cancelled)?;
    let entry = Entry {
        path: PROVISION.into(),
        bytes: provision.0.len() as u64,
        mode: DATA,
        crc: crc32(&provision.0),
        offset: writer.written as u32,
    };
    writer.local(&entry)?;
    for chunk in provision.0.chunks(8192) {
        check_cancel(&mut cancelled)?;
        writer.write(chunk)?;
    }
    entries.push(entry);
    let central_offset = writer.written as u32;
    for entry in &entries {
        check_cancel(&mut cancelled)?;
        writer.central(entry)?;
    }
    let mut end = [0u8; 22];
    u32_at(&mut end, 0, 0x0605_4b50);
    u16_at(&mut end, 8, entries.len() as u16);
    u16_at(&mut end, 10, entries.len() as u16);
    u32_at(&mut end, 12, writer.written as u32 - central_offset);
    u32_at(&mut end, 16, central_offset);
    writer.write(&end)?;
    check_cancel(&mut cancelled)?;
    if writer.written != total {
        return Err("native ZIP byte correspondence failed".into());
    }
    Ok(JoiningHostPackageReceipt {
        artifact_bytes: total,
        artifact_content_sha256: format!("sha256:{:x}", writer.hash.finalize()),
        image_content_sha256: release.image_content_digest().into(),
        files: entries
            .into_iter()
            .map(|e| JoiningHostPackageFile {
                path: e.path,
                bytes: e.bytes,
                mode: e.mode,
            })
            .collect(),
    })
}

fn check_binding(
    release: &VerifiedHostReleaseBytes,
    built: &BuiltSpore,
    invitation: &PortableInvitation,
    now: u64,
) -> Result<(), String> {
    if built.image_bytes.is_empty() || built.image_bytes.len() > 1024 * 1024 {
        return Err("native ZIP selected IMAGE manifest exceeds its finite bound".into());
    }
    let manifest = release.manifest();
    let spore = &built.manifest;
    let image = &built.image.manifest;
    conduit_host_make::verify_image_binding(&built.image, &built.image_bytes)
        .map_err(|_| "native ZIP selected IMAGE bytes differ from their binding")?;
    if manifest.files.is_empty()
        || manifest.files.len() >= MAX_FILES
        || manifest.files.iter().any(|f| f.path == PROVISION)
        || spore.schema != SPORE_MANIFEST_SCHEMA
        || spore.make.output != spore.output
        || !matches!(
            (&spore.output, manifest.output.as_str()),
            (SporeOutputKind::NativeBundle, "native-bundle")
                | (SporeOutputKind::BrowserBundle, "browser-bundle")
        )
        || spore.image_content_digest != release.image_content_digest()
        || spore.target != manifest.target_id
        // Construction/Birth Sign provenance belongs to the Spore; artifact
        // source provenance belongs to the underlying Make IMAGE.
        || spore.source_identity.is_empty()
        || image.source_identity != manifest.source_identity
        || spore.image_id != image.image_id
        || spore.profile_id != image.profile_id
        || spore.build_id != image.build_id
        || spore.host_configuration_id != image.source_configuration_id.as_deref().unwrap_or_default()
        || spore.target != image.target
        || spore.output != image.output
        || spore.make.make_package_id != image.make_package_id
        || spore.make.make_package_revision != image.make_package_revision
        || spore.make.toolchain_identity != image.toolchain_identity
        || spore.make.strategy != image.make_strategy
        || spore.make.builder_adapter != image.builder_adapter
        || spore.make.deployment_adapter != image.deployment_adapter
        || spore.make.post_build_actions != image.post_build_actions
        || spore.make.implementation_packages != image.implementation_packages
        || spore.make.make_package_id != manifest.make_package_id
        || spore.make.builder_adapter != manifest.builder_adapter
        || spore.make.deployment_adapter.as_deref() != Some(manifest.deployment_adapter.as_str())
        || spore.spore_id.is_empty()
        || spore.image_id.is_empty()
        || spore.body_id != invitation.claim.body_id.as_str()
        || !matches!(&spore.binding, SporeBinding::SelfJoining { invitation_id }
            if invitation_id == invitation.claim.invitation_id.as_str())
        || invitation.secret == [0; 32]
    {
        return Err("native ZIP lost exact release, Spore or invitation binding".into());
    }
    if invitation.rendezvous.as_ref().is_some_and(|descriptor| {
        descriptor
            .candidates
            .iter()
            .any(|candidate| candidate.expires_at_millis > invitation.claim.expires_at_millis)
    }) {
        return Err("native ZIP rendezvous exceeds invitation expiry".into());
    }
    invitation
        .validate(now)
        .map_err(|_| "native ZIP invitation provision invalid")?;
    Ok(())
}

fn check_cancel(cancelled: &mut impl FnMut() -> bool) -> Result<(), String> {
    if cancelled() {
        Err("native ZIP production cancelled".into())
    } else {
        Ok(())
    }
}
fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 256
        && !matches!(path, "." | "..")
        && !path
            .bytes()
            .any(|b| b <= b' ' || b == 127 || b"/\\%:?#".contains(&b))
}
fn read_payload(
    reader: &mut impl Read,
    length: u64,
    expected: &str,
    cancelled: &mut impl FnMut() -> bool,
    mut consume: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<u32, String> {
    let mut buffer = [0u8; 8192];
    let mut hash = Sha256::new();
    let mut crc = !0u32;
    let mut count = 0u64;
    loop {
        check_cancel(cancelled)?;
        let capacity = (length - count).min(buffer.len() as u64).max(1) as usize;
        let n = reader
            .read(&mut buffer[..capacity])
            .map_err(|_| "native ZIP payload read failed")?;
        if n == 0 {
            break;
        }
        if n as u64 > length - count {
            return Err("native ZIP payload exceeds reviewed length".into());
        }
        count += n as u64;
        hash.update(&buffer[..n]);
        crc = crc_update(crc, &buffer[..n]);
        consume(&buffer[..n])?;
    }
    if count != length || format!("sha256:{:x}", hash.finalize()) != expected {
        return Err("native ZIP payload differs from verified release".into());
    }
    Ok(!crc)
}
struct PackageWriter<'a, W> {
    output: &'a mut W,
    hash: Sha256,
    written: u64,
}
impl<W: Write> PackageWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.output
            .write_all(bytes)
            .map_err(|_| "native ZIP output write failed")?;
        self.hash.update(bytes);
        self.written += bytes.len() as u64;
        Ok(())
    }
    fn local(&mut self, e: &Entry) -> Result<(), String> {
        let mut header = [0u8; 30];
        u32_at(&mut header, 0, 0x0403_4b50);
        u16_at(&mut header, 4, 20);
        u16_at(&mut header, 6, 0x0800);
        u16_at(&mut header, 12, 0x0021);
        u32_at(&mut header, 14, e.crc);
        u32_at(&mut header, 18, e.bytes as u32);
        u32_at(&mut header, 22, e.bytes as u32);
        u16_at(&mut header, 26, e.path.len() as u16);
        self.write(&header)?;
        self.write(e.path.as_bytes())
    }
    fn central(&mut self, e: &Entry) -> Result<(), String> {
        let mut header = [0u8; 46];
        u32_at(&mut header, 0, 0x0201_4b50);
        u16_at(&mut header, 4, 0x0314);
        u16_at(&mut header, 6, 20);
        u16_at(&mut header, 8, 0x0800);
        u16_at(&mut header, 14, 0x0021);
        u32_at(&mut header, 16, e.crc);
        u32_at(&mut header, 20, e.bytes as u32);
        u32_at(&mut header, 24, e.bytes as u32);
        u16_at(&mut header, 28, e.path.len() as u16);
        u32_at(&mut header, 38, e.mode << 16);
        u32_at(&mut header, 42, e.offset);
        self.write(&header)?;
        self.write(e.path.as_bytes())
    }
}
fn u16_at(bytes: &mut [u8], offset: usize, n: u16) {
    bytes[offset..offset + 2].copy_from_slice(&n.to_le_bytes());
}
fn u32_at(bytes: &mut [u8], offset: usize, n: u32) {
    bytes[offset..offset + 4].copy_from_slice(&n.to_le_bytes());
}
fn crc32(bytes: &[u8]) -> u32 {
    !crc_update(!0, bytes)
}
fn crc_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    crc
}
