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
#[path = "export.rs"]
mod export;
pub(crate) use export::{Export, Progress};
/// Attended driver of the same one-request engine used by incremental callers.
pub(super) fn retain_readonly(
    run: &Path,
    body: &BodyId,
    root: [u8; 32],
    boot: &BootId,
    mut get: impl FnMut(&str) -> Result<(u16, Vec<u8>)>,
    mut pause: impl FnMut(),
) -> Result<VerifiedDurableSnapshot> {
    let mut export = Export::open(run, body, root, boot)?;
    loop {
        let now = Instant::now();
        let deadline = export
            .lease_deadline()
            .filter(|deadline| *deadline > now)
            .unwrap_or(now + Duration::from_secs(150));
        match export.advance(deadline, |path, _| get(path))? {
            Progress::Complete(token) => return Ok(token),
            Progress::Pending => pause(),
        }
    }
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
