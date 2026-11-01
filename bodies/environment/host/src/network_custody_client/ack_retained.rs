//! Historical completion inspection has no signer, transport, or retry entrance.
use super::*;

// One finite Root cursor, with no allocation solely for the successful variant.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Retained {
    Absent,
    Interrupted,
    Complete(CommittedNetworkCustody),
}

/// Reopen the original immutable ACK and its signed Clean receipt, independent
/// of the caller's current incarnation. Never rewrite evidence or renew work.
pub(crate) fn reopen_completed(token: &VerifiedDurableSnapshot) -> Result<Retained> {
    token.revalidate()?;
    let ack_path = token.directory.join("ack.bin");
    let session = token.directory.join(SESSION);
    let completed = token.directory.join(COMPLETED);
    if !ack_path.try_exists()? && !session.try_exists()? && !completed.try_exists()? {
        return Ok(Retained::Absent);
    }
    let ack = reopen(&ack_path, REQUEST)?;
    let credential = original_credential(token, &ack)?;
    if !session.try_exists()? {
        if completed.try_exists()? {
            return Err("network ACK completion has no session".into());
        }
        // No window has been reserved and no POST was authorized. Preserve the
        // ordinary open path, which still requires this exact immutable ACK.
        return Ok(Retained::Absent);
    }
    let mut marker = [0; 88];
    marker[..8].copy_from_slice(b"NCACKR1\0");
    marker[8..40].copy_from_slice(&token.offer);
    marker[40..72].copy_from_slice(&digest(&ack));
    marker[72..80].copy_from_slice(&(ATTEMPTS as u64).to_le_bytes());
    marker[80..88].copy_from_slice(&LIFETIME.as_secs().to_le_bytes());
    if reopen(&session, marker.len())? != marker {
        return Err("network ACK session correlation differs".into());
    }
    if !completed.try_exists()? {
        return Ok(Retained::Interrupted);
    }
    let response = live::input(&completed, 1408)?;
    let receipt = token.verify_committed_receipt(&credential, &ack, &response)?;
    if receipt.bytes()[8] != 2 {
        return Err("network ACK completion is not Clean".into());
    }
    let id = digest(receipt.bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if reopen(
        &token.directory.join(format!("receipt-{id}.json")),
        response.len(),
    )? != response
        || reopen(&token.directory.join(format!("receipt-{id}.bin")), 480)?.as_slice()
            != receipt.bytes()
    {
        return Err("network historical completion files differ".into());
    }
    Ok(Retained::Complete(receipt))
}

/// Decode only identity data, then require exact canonical ACK re-encoding.
/// Authority comes later from both original member and pinned Root signatures.
fn original_credential(
    token: &VerifiedDurableSnapshot,
    ack: &[u8],
) -> Result<MembershipCredential> {
    let mut ids = [""; 4];
    for (index, id) in ids.iter_mut().enumerate() {
        let length = usize::from(ack[48 + index]);
        if !(1..=128).contains(&length) {
            return Err("network historical ACK identity extent".into());
        }
        *id = std::str::from_utf8(&ack[52 + index * 128..52 + index * 128 + length])?;
    }
    let credential = serde_json::from_value(serde_json::json!({
        "body_id":token.scope.body,"part_id":ids[0],"host_id":ids[1],
        "boot_id":ids[2],"credential_id":ids[3],
        "issued_at_millis":u64::from_le_bytes(ack[40..48].try_into().unwrap()),
    }))?;
    if token
        .encode_fields(&credential, ack[564..].try_into().unwrap())?
        .as_slice()
        != ack
    {
        return Err("network historical ACK encoding differs".into());
    }
    Ok(credential)
}

#[cfg(test)]
#[path = "ack_retained_tests.rs"]
mod tests;

/// Adopt an already obtained historical completion. No signer, transport,
/// clock or retry window is created; the original reserved session is required.
pub(crate) fn retain_completion(
    token: &VerifiedDurableSnapshot,
    response: &[u8],
) -> Result<CommittedNetworkCustody> {
    let lock_path = token.directory.join("incremental-ack.lock");
    private(&lock_path, false)?;
    let lock = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&lock_path)?;
    // SAFETY: this owned descriptor is retained through verification and fsync.
    // Recovery cannot race the original affine acknowledgment writer.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("network ACK owner still running".into());
    }
    let prior = reopen_completed(token)?;
    if matches!(prior, Retained::Absent) {
        return Err("network completion requires original reserved session".into());
    }
    let ack = reopen(&token.directory.join("ack.bin"), REQUEST)?;
    let credential = original_credential(token, &ack)?;
    let verified = token.verify_committed_receipt(&credential, &ack, response)?;
    if verified.bytes()[8] != 2 {
        return Err("network recovered completion is not Clean".into());
    }
    if let Retained::Complete(existing) = prior {
        if existing.bytes() != verified.bytes() {
            return Err("network recovered completion differs".into());
        }
        return Ok(existing);
    }
    token.retain_committed_receipt(&credential, &ack, response)?;
    immutable(&token.directory.join(COMPLETED), response)?;
    File::open(&token.directory)?.sync_all()?;
    match reopen_completed(token)? {
        Retained::Complete(receipt) if receipt.bytes() == verified.bytes() => Ok(receipt),
        _ => Err("network recovered completion did not reopen".into()),
    }
}

/// Filesystem-only recovery using independent admitted Body/Root pins and the
/// original producer Boot. Neither the response nor its filenames supply trust.
pub(crate) fn run(
    binding: &Path,
    producer_boot: &str,
    directory: &Path,
    response: &Path,
) -> Result<()> {
    let binding = crate::local_recorder::validated_binding(binding)?;
    private(directory, true)?;
    let offer = reopen(&directory.join("offer.bin"), OFFER)?;
    let offer: [u8; OFFER] = offer.try_into().map_err(|_| "network offer extent")?;
    let scope = Scope {
        body: binding.credential().body_id.clone(),
        root: *binding.root(),
        producer_boot: producer_boot.into(),
        epoch: u64::from_le_bytes(offer[64..72].try_into().unwrap()),
        region: offer[32..64].try_into().unwrap(),
        predecessor: offer[144..176].try_into().unwrap(),
    };
    let id = verify(
        &scope,
        &offer,
        &reopen(&directory.join("snapshot.bin"), CAPACITY)?,
    )?;
    let token = VerifiedDurableSnapshot {
        scope,
        directory: directory.into(),
        offer: id,
    };
    let response = live::input(response, 1408)?;
    let receipt = retain_completion(&token, &response)?;
    println!("original Root-signed Clean receipt verified and reopened ({} bytes); no transport or retry allowance created", receipt.bytes().len());
    Ok(())
}
