//! Internal maintenance: verify an actual captured cold offer and durably publish
//! only its recorder-signed ACK. This module has no carrier or producer signer.
//! Saved binding validation proves local provenance, not remote credential
//! freshness: only the producer decides current admission and ACK acceptance.
use crate::observed_custody_client::{wire, VerifiedDurableEpoch};
use crate::Result;
use std::{
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
const MAX_OFFER_BYTES: usize = 4096;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Offered<'a> {
    event: &'a str,
    slot: u8,
    #[serde(borrow)]
    offer: wire::Offer<'a>,
}
/// Cold exchange and offline preparation use exactly the same bounded strict
/// envelope, independent pin, Forms, scope, digest and durable-file verifier.
pub(super) fn verify_offered(
    binding_path: &Path,
    history: &Path,
    bytes: &[u8],
    expected_slot: Option<u8>,
) -> Result<(u8, VerifiedDurableEpoch)> {
    if bytes.len() > MAX_OFFER_BYTES {
        return Err("observed custody offer extent".into());
    }
    let offered: Offered<'_> = serde_json::from_slice(bytes)?;
    if offered.event != "observed-custody-offer"
        || offered.slot > 5
        || expected_slot.is_some_and(|slot| slot != offered.slot)
    {
        return Err("observed custody offer slot/event differs".into());
    }
    let binding = crate::local_recorder::validated_binding(binding_path)?;
    let scope = offered
        .offer
        .scope
        .0
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let directory = history.join(format!("epoch-{scope}"));
    let verified = VerifiedDurableEpoch::verify(
        &offered.offer,
        &binding.credential().body_id,
        binding.root(),
        &directory,
    )?;
    Ok((offered.slot, verified))
}
fn private(path: &Path, directory: bool) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    // SAFETY: observation of the process's effective local identity only.
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || if directory { !m.is_dir() } else { !m.is_file() }
    {
        return Err(
            "observed custody preparation requires current-owner private files/directories".into(),
        );
    }
    Ok(())
}
fn load_offer(path: &Path) -> Result<Vec<u8>> {
    private(path.parent().ok_or("private offer parent")?, true)?;
    private(path, false)?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let m = file.metadata()?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || m.len() > MAX_OFFER_BYTES as u64
    {
        return Err("observed custody private offer metadata/extent".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_OFFER_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_OFFER_BYTES {
        return Err("observed custody offer extent".into());
    }
    Ok(bytes)
}
fn save_new_ack(path: &Path, bytes: &[u8]) -> Result<()> {
    if bytes.len() > wire::MAX_REQUEST {
        return Err("observed custody ACK extent".into());
    }
    let parent = path.parent().ok_or("private ACK parent")?;
    private(parent, true)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        // Only this newly created inode is ours; an existing output never enters
        // this branch. Failed publication leaves no partial ACK to transfer.
        let _ = std::fs::remove_file(path);
        let _ = std::fs::File::open(parent).and_then(|d| d.sync_all());
    }
    result
}
pub(super) fn run(
    binding: &Path,
    continuity: &Path,
    history: &Path,
    offer: &Path,
    output: &Path,
) -> Result<()> {
    let bytes = load_offer(offer)?;
    let (_, verified) = verify_offered(binding, history, &bytes, None)?;
    // Reloads and validates the actual current saved binding, continuity secret,
    // invitation and Root. Producer still checks latest admission on acceptance.
    let ack = crate::body_join::sign_durable_custody(binding, continuity, &verified)?;
    save_new_ack(output, &ack)
}
pub(crate) fn from_args(args: &[String]) -> Result<()> {
    if args.len() != 5 {
        return Err("expected --prepare-observed-custody PRIVATE_BINDING CONTINUITY_PRIVATE HISTORY_DIRECTORY PRIVATE_OFFER_FILE PRIVATE_ACK_FILE".into());
    }
    run(
        Path::new(&args[0]),
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
        Path::new(&args[4]),
    )?;
    println!("prepared recorder custody ACK from authenticated offer and reopened durable history");
    Ok(())
}
