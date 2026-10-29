//! Read-only pinned-Root Face feed. Publication preserves the complete sealed
//! Face; the local lease reports transport availability, never sensor freshness.
use crate::Result;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::Duration,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Offer {
    profile: String,
    producer_boot: String,
    body_id: conduit_body::BodyId,
    offer: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chunk {
    profile: String,
    digest: String,
    offset: u32,
    bytes: String,
}
struct Verified {
    digest: [u8; 32],
    revision: u64,
    bytes: u32,
    lifetime: u64,
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2)
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Face export hex".into());
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| Ok(u8::from_str_radix(std::str::from_utf8(p)?, 16)?))
        .collect()
}
fn verify(
    root: &[u8; 32],
    body: &conduit_body::BodyId,
    nonce: &[u8; 32],
    bytes: &[u8],
) -> Result<Verified> {
    let reply: Offer = serde_json::from_slice(bytes)?;
    let offer = unhex(&reply.offer)?;
    if reply.profile != "conduit.face/export@1"
        || reply.body_id != *body
        || reply.producer_boot.is_empty()
        || reply.producer_boot.len() > 256
        || offer.len() != 220
    {
        return Err("Face export scope/extent".into());
    }
    let mut hash = Sha256::new();
    hash.update(b"conduit.face/export@1\0");
    hash.update(root);
    hash.update(&offer[..156]);
    crate::checked(
        crate::checked(VerifyingKey::from_bytes(root))?.verify_strict(
            &hash.finalize(),
            &Signature::from_bytes(offer[156..].try_into()?),
        ),
    )?;
    if offer[..32] != *nonce
        || offer[32..64] != Sha256::digest(body.as_str().as_bytes())[..]
        || offer[64..96] != Sha256::digest(reply.producer_boot.as_bytes())[..]
    {
        return Err("Face export signed scope".into());
    }
    let revision = u64::from_le_bytes(offer[128..136].try_into()?);
    let length = u32::from_le_bytes(offer[136..140].try_into()?);
    let issued = u64::from_le_bytes(offer[140..148].try_into()?);
    let until = u64::from_le_bytes(offer[148..156].try_into()?);
    let lifetime = until
        .checked_sub(issued)
        .filter(|n| (1..=5000).contains(n))
        .ok_or("Face export lifetime")?;
    if !(1..=8192).contains(&length) {
        return Err("Face export document capacity".into());
    }
    Ok(Verified {
        digest: offer[96..128].try_into()?,
        revision,
        bytes: length,
        lifetime,
    })
}
fn boot_ms() -> Result<u64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: valid writable timespec; observes the actual local suspend-aware clock.
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut time) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    (time.tv_sec as u64)
        .checked_mul(1000)
        .and_then(|n| n.checked_add(time.tv_nsec as u64 / 1_000_000))
        .ok_or_else(|| "Face feed clock".into())
}
fn get(peer: &str, path: &str, started: u64) -> Result<Vec<u8>> {
    get_until(
        || crate::body_join::http(peer, "GET", path, &[]),
        boot_ms,
        || std::thread::sleep(Duration::from_millis(50)),
        started,
    )
}
fn get_until(
    mut request: impl FnMut() -> Result<(u16, Vec<u8>)>,
    mut now: impl FnMut() -> Result<u64>,
    mut pause: impl FnMut(),
    started: u64,
) -> Result<Vec<u8>> {
    for _ in 0..24 {
        if now()?.saturating_sub(started) >= 5000 {
            return Err("Face export absolute deadline".into());
        }
        match request() {
            Ok((200, bytes)) if bytes.len() <= 1408 => return Ok(bytes),
            Ok((202 | 503, _)) => pause(),
            Ok((status, _)) => return Err(format!("Face export HTTP {status}").into()),
            Err(error) if retryable_transport(error.as_ref()) => pause(),
            Err(error) => return Err(error),
        }
    }
    Err("Face export polling bound".into())
}
fn retryable_transport(error: &(dyn std::error::Error + 'static)) -> bool {
    use std::io::ErrorKind;
    error.downcast_ref::<std::io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::TimedOut
                | ErrorKind::WouldBlock
                | ErrorKind::Interrupted
                | ErrorKind::UnexpectedEof
        )
    })
}

fn fetch(
    root: &[u8; 32],
    body: &conduit_body::BodyId,
    peer: &str,
) -> Result<(conduit_face::Face, u64)> {
    let mut nonce = [0; 32];
    File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    let started = boot_ms()?;
    let offer = get(
        peer,
        &format!("/face/export/snapshot?nonce={}", hex(&nonce)),
        started,
    )?;
    let verified = verify(root, body, &nonce, &offer)?;
    let mut document = Vec::with_capacity(verified.bytes as usize);
    for offset in (0..verified.bytes).step_by(512) {
        let path = format!(
            "/face/export/chunk?digest={}&revision={}&bytes={}&offset={offset}",
            hex(&verified.digest),
            verified.revision,
            verified.bytes
        );
        let chunk: Chunk = serde_json::from_slice(&get(peer, &path, started)?)?;
        let bytes = unhex(&chunk.bytes)?;
        if chunk.profile != "conduit.face/chunk@1"
            || chunk.digest != hex(&verified.digest)
            || chunk.offset != offset
            || bytes.len() != 512.min((verified.bytes - offset) as usize)
        {
            return Err("Face export chunk scope".into());
        }
        document.extend_from_slice(&bytes);
    }
    if Sha256::digest(&document)[..] != verified.digest {
        return Err("Face export document digest".into());
    }
    let face: conduit_face::Face = serde_json::from_slice(&document)?;
    crate::checked(face.validate())?;
    if face.revision != verified.revision
        || face.basis.body_id.as_ref() != Some(body)
        || face.basis.wake_id.is_none()
        || face.basis.source_document_id.is_none()
        || face.basis.checked_plot_id.is_none()
        || face.basis.expanded_plot_id.is_none()
        || face.basis.plan_id.is_none()
        || face.basis.active_play_id.is_none()
    {
        return Err("Face export execution basis".into());
    }
    // Include time spent fetching/signing/chunks and any local suspend. Never
    // restart the producer's semantic freshness window on remote arrival.
    let deadline = started
        .checked_add(verified.lifetime)
        .ok_or("Face export deadline")?;
    if boot_ms()?.saturating_add(1) >= deadline {
        return Err("Face export expired in transit".into());
    }
    Ok((face, deadline))
}
fn private(path: &Path, directory: bool) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    // SAFETY: observes effective process identity without changing it.
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || if directory { !m.is_dir() } else { !m.is_file() }
    {
        return Err("Face feed requires private owned files/directory".into());
    }
    Ok(())
}
fn publish(path: &Path, face: &conduit_face::Face, deadline: u64) -> Result<()> {
    let parent = path.parent().ok_or("Face feed parent")?;
    private(parent, true)?;
    match std::fs::symlink_metadata(path) {
        Ok(_) => private(path, false)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    if boot_ms()? >= deadline {
        return Err("Face feed expired before publication".into());
    }
    #[derive(Serialize)]
    struct Envelope<'a> {
        schema: &'static str,
        boot_id: String,
        valid_until_boot_ms: u64,
        face: &'a conduit_face::Face,
    }
    let envelope = Envelope {
        schema: "conduit.local-face/leased@1",
        boot_id: std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?
            .trim()
            .into(),
        valid_until_boot_ms: deadline,
        face,
    };
    let bytes = serde_json::to_vec(&envelope)?;
    if bytes.len() > 16384 {
        return Err("Face feed envelope capacity".into());
    }
    let mut random = [0; 8];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let temp = parent.join(format!(".face-{}.tmp", hex(&random)));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
fn refresh_delay(started: u64, finished: u64) -> Duration {
    Duration::from_millis(1000u64.saturating_sub(finished.saturating_sub(started)))
}
pub(crate) fn run(binding: &Path, peer: &str, output: &Path, once: bool) -> Result<()> {
    let binding = crate::local_recorder::validated_binding(binding)?;
    let parent = output.parent().ok_or("Face feed parent")?;
    crate::observed_history::create_durable_directory(parent)?;
    private(parent, true)?;
    loop {
        let started = boot_ms()?;
        let result = fetch(binding.root(), &binding.credential().body_id, peer)
            .and_then(|(face, remaining)| publish(output, &face, remaining));
        if once {
            return result;
        }
        if let Err(error) = result {
            eprintln!("Face feed unavailable: {error}");
        }
        std::thread::sleep(refresh_delay(started, boot_ms()?));
    }
}
#[cfg(test)]
#[path = "face_export_client_tests.rs"]
mod tests;
