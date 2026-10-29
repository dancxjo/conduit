//! Three persisted export leases. Partials never authorize custody or ACKs.
use super::*;
use std::{
    os::fd::AsRawFd,
    time::{Duration, Instant},
};
const CHUNK: usize = 512;
const LEASES: usize = 3;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Chunk {
    profile: String,
    snapshot: String,
    offset: usize,
    bytes: String,
}
fn hex(id: &[u8]) -> String {
    id.iter().map(|v| format!("{v:02x}")).collect()
}
fn scope(body: &BodyId, root: [u8; 32], boot: &BootId, offer: &[u8; OFFER]) -> Scope {
    Scope {
        body: body.clone(),
        root,
        producer_boot: boot.clone(),
        epoch: u64::from_le_bytes(offer[64..72].try_into().unwrap()),
        region: offer[32..64].try_into().unwrap(),
        predecessor: offer[144..176].try_into().unwrap(),
    }
}
fn directory(path: &Path) -> Result<()> {
    match std::fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => File::open(path.parent().ok_or("network run parent")?)?.sync_all()?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.into()),
    }
    private(path, true)
}
fn durable(path: &Path, bytes: &[u8]) -> Result<()> {
    immutable(path, bytes)?;
    File::open(path.parent().ok_or("network file parent")?)?.sync_all()?;
    if reopen(path, bytes.len())? != bytes {
        return Err("network retained bytes differ".into());
    }
    Ok(())
}
/// GET is the only transport capability accepted here. No membership signing,
/// ACK, retirement, or erase operation is reachable through this entrance.
pub(super) fn retain_readonly(
    run: &Path,
    body: &BodyId,
    root: [u8; 32],
    boot: &BootId,
    mut get: impl FnMut(&str) -> Result<(u16, Vec<u8>)>,
    mut pause: impl FnMut(),
) -> Result<VerifiedDurableSnapshot> {
    if [body.as_str(), boot.as_str()]
        .iter()
        .any(|id| id.is_empty() || id.len() > 128 || id.chars().any(char::is_control))
    {
        return Err("network export identity extent".into());
    }
    private(run, true)?;
    let lock_path = run.join("export.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&lock_path)?;
    private(&lock_path, false)?;
    // SAFETY: a retained owned file descriptor; lock is released on File drop.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("network export already running".into());
    }
    let scope_bytes = serde_json::to_vec(
        &serde_json::json!({"body":body,"root":root,"boot":boot,"maximum_leases":LEASES}),
    )?;
    durable(&run.join("scope.json"), &scope_bytes)?;
    let offer_path = run.join("offer.bin");
    let mut bound = if offer_path.try_exists()? {
        let bytes = reopen(&offer_path, OFFER)?;
        let offer: [u8; OFFER] = bytes.try_into().unwrap();
        let scope = scope(body, root, boot, &offer);
        let id = verify_offer(&scope, &offer)?;
        let completed = run.join(format!("network-{}", hex(&id)));
        if completed.try_exists()? {
            private(&completed, true)?;
        }
        if completed.join("snapshot.bin").try_exists()? {
            // Existing completion must pass the same whole-file/signature gate.
            let data = reopen(&completed.join("snapshot.bin"), CAPACITY)?;
            return VerifiedDurableSnapshot::retain(run, &offer, &data, &scope);
        }
        Some(offer)
    } else {
        None
    };
    let mut started = 0;
    for lease in 0..LEASES {
        let path = run.join(format!("lease-{lease}.bin"));
        if path.try_exists()? {
            reopen(&path, 0)?;
            if started != lease {
                return Err("network lease sequence differs".into());
            }
            started += 1;
        }
    }
    if bound.is_some() && started == 0 {
        return Err("network export retained offer has no spent lease".into());
    }
    for lease in started..LEASES {
        // Persist and reopen the finite spend BEFORE any request. Restart cannot
        // replay a partially spent lease or obtain an unbounded retry allowance.
        durable(&run.join(format!("lease-{lease}.bin")), &[])?;
        let deadline = Instant::now() + Duration::from_secs(150);
        let Some(raw) = read(&mut get, &mut pause, "/network/custody/offer", deadline)? else {
            continue;
        };
        let envelope: live::Offered = serde_json::from_slice(&raw)?;
        if envelope.profile != "network-retirement-offer@1" {
            return Err("network offer profile".into());
        }
        let offer = live::unhex::<OFFER>(&envelope.offer)?;
        let scope = scope(body, root, boot, &offer);
        let id = verify_offer(&scope, &offer)?;
        if live::unhex::<32>(&envelope.snapshot)? != id {
            return Err("network offer id differs".into());
        }
        if bound.as_ref().is_some_and(|old| old != &offer) {
            return Err("network snapshot changed; partials retained unverified".into());
        }
        durable(&offer_path, &offer)?;
        bound = Some(offer);
        let chunks = run.join(format!("unverified-{}", hex(&id)));
        directory(&chunks)?;
        let mut snapshot = Vec::with_capacity(CAPACITY);
        let mut expired = false;
        for offset in (0..CAPACITY).step_by(CHUNK) {
            let path = chunks.join(format!("{offset:05}.bin"));
            if !path.try_exists()? {
                let route = format!(
                    "/network/custody/chunk?snapshot={}&offset={offset}",
                    hex(&id)
                );
                let Some(raw) = read(&mut get, &mut pause, &route, deadline)? else {
                    expired = true;
                    break;
                };
                let chunk: Chunk = serde_json::from_slice(&raw)?;
                if chunk.profile != "network-retirement-chunk@1"
                    || live::unhex::<32>(&chunk.snapshot)? != id
                    || chunk.offset != offset
                {
                    return Err("network chunk scope differs".into());
                }
                durable(&path, &live::unhex::<CHUNK>(&chunk.bytes)?)?;
            }
            snapshot.extend_from_slice(&reopen(&path, CHUNK)?);
        }
        if expired {
            continue;
        }
        // Only this existing full digest + durable reopen constructor mints the
        // opaque token. Retained partials cannot enter the ACK signer.
        return VerifiedDurableSnapshot::retain(run, &offer, &snapshot, &scope);
    }
    Err("network export lease bound exhausted; partials remain unverified".into())
}
fn read(
    get: &mut impl FnMut(&str) -> Result<(u16, Vec<u8>)>,
    pause: &mut impl FnMut(),
    path: &str,
    deadline: Instant,
) -> Result<Option<Vec<u8>>> {
    for _ in 0..376 {
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let response = get(path);
        if Instant::now() >= deadline {
            return Ok(None);
        }
        if response.as_ref().is_ok_and(|(_, bytes)| bytes.len() > 2048) {
            return Err("network export response extent".into());
        }
        match response {
            Ok((200, bytes)) => return Ok(Some(bytes)),
            Ok((409, _)) => return Ok(None),
            Ok((202 | 503, _)) | Err(_) => pause(),
            Ok((status, _)) => return Err(format!("network export HTTP status {status}").into()),
        }
    }
    Ok(None)
}
pub(crate) fn run(binding: &Path, peer: &str, run: &Path, producer_boot: &str) -> Result<()> {
    let binding = crate::local_recorder::validated_binding(binding)?;
    directory(run)?;
    let token = retain_readonly(
        run,
        &binding.credential().body_id,
        *binding.root(),
        &producer_boot.into(),
        |path| crate::body_join::http(peer, "GET", path, &[]),
        || std::thread::sleep(Duration::from_millis(400)),
    )?;
    token.revalidate()?;
    let offer = reopen(&token.directory.join("offer.bin"), OFFER)?;
    durable(
        &token.directory.join("offer.json"),
        &serde_json::to_vec(&serde_json::json!({
            "profile":"network-retirement-offer@1", "snapshot":hex(&token.offer), "offer":hex(&offer)
        }))?,
    )?;
    println!(
        "network snapshot durably retained at {}; no ACK or erase requested",
        token.directory.display()
    );
    Ok(())
}

#[cfg(test)]
#[path = "resume_tests.rs"]
mod tests;
