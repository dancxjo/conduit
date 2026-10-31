//! Read-only pinned-Root Face feed. Publication preserves the complete sealed
//! Face; the local lease reports transport availability, never sensor freshness.
use crate::Result;
use conduit_environment_host::signed_face_export::{verify_snapshot, VerifiedSnapshot as Verified};
use serde::{Deserialize, Serialize};
#[cfg(test)]
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
struct Chunk {
    profile: String,
    digest: String,
    offset: u32,
    bytes: String,
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
    verify_snapshot(root, body, nonce, bytes)
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
        || std::thread::sleep(Duration::from_millis(RETRY_PAUSE_MS)),
        started,
    )
}
const FETCH_BUDGET_MS: u64 = 5000;
const RETRY_PAUSE_MS: u64 = 50;
// The attempt bound must allow the entire admitted time budget. A shorter
// count abandoned live requests before their deadline and left their finite
// response slot occupied while the next request used a different nonce.
const MAXIMUM_ATTEMPTS: u64 = FETCH_BUDGET_MS / RETRY_PAUSE_MS + 1;
fn get_until(
    mut request: impl FnMut() -> Result<(u16, Vec<u8>)>,
    mut now: impl FnMut() -> Result<u64>,
    mut pause: impl FnMut(),
    started: u64,
) -> Result<Vec<u8>> {
    for _ in 0..MAXIMUM_ATTEMPTS {
        if now()?.saturating_sub(started) >= FETCH_BUDGET_MS {
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
    let mut document = Vec::with_capacity(verified.bytes() as usize);
    for offset in (0..verified.bytes()).step_by(512) {
        let path = format!(
            "/face/export/chunk?digest={}&revision={}&bytes={}&offset={offset}",
            hex(verified.digest()),
            verified.revision(),
            verified.bytes()
        );
        let chunk: Chunk = serde_json::from_slice(&get(peer, &path, started)?)?;
        let bytes = unhex(&chunk.bytes)?;
        if chunk.profile != "conduit.face/chunk@1"
            || chunk.digest != hex(verified.digest())
            || chunk.offset != offset
            || bytes.len() != 512.min((verified.bytes() - offset) as usize)
        {
            return Err("Face export chunk scope".into());
        }
        document.extend_from_slice(&bytes);
    }
    let face = verified.decode_document(&document)?;
    // Include time spent fetching/signing/chunks and any local suspend. Never
    // restart the producer's semantic freshness window on remote arrival.
    let deadline = started
        .checked_add(verified.lifetime())
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
