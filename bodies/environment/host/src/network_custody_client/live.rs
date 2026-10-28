//! Attended finite network ACK exchange. Files are evidence custody only;
//! neither this command nor a successful receipt requests physical erasure.
use super::*;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Offered {
    profile: String,
    snapshot: String,
    offer: String,
}
fn unhex<const N: usize>(text: &str) -> Result<[u8; N]> {
    if text.len() != N * 2 {
        return Err("network offer hex extent".into());
    }
    let mut out = [0; N];
    for (byte, pair) in out.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let digit = |b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err("network offer hex"),
        };
        *byte = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(out)
}
fn input(path: &Path, bound: usize) -> Result<Vec<u8>> {
    private(path, false)?;
    let mut out = Vec::new();
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?
        .take((bound + 1) as u64)
        .read_to_end(&mut out)?;
    if out.len() > bound {
        return Err("network custody input capacity".into());
    }
    Ok(out)
}
pub(crate) fn run(
    binding: &Path,
    continuity: &Path,
    peer: &str,
    store: &Path,
    offered: &Path,
    snapshot: &Path,
    producer_boot: &str,
) -> Result<()> {
    let binding_scope = crate::local_recorder::validated_binding(binding)?;
    let envelope: Offered = serde_json::from_slice(&input(offered, 1408)?)?;
    if envelope.profile != "network-retirement-offer@1" {
        return Err("network offer profile".into());
    }
    let offer = unhex::<240>(&envelope.offer)?;
    let boot: BootId = producer_boot.into();
    let scope = Scope {
        body: binding_scope.credential().body_id.clone(),
        root: *binding_scope.root(),
        producer_boot: boot,
        epoch: u64::from_le_bytes(offer[64..72].try_into().unwrap()),
        region: offer[32..64].try_into().unwrap(),
        predecessor: offer[144..176].try_into().unwrap(),
    };
    if unhex::<32>(&envelope.snapshot)? != offer_id(&scope.root, &offer) {
        return Err("network offer identifier".into());
    }
    let token =
        VerifiedDurableSnapshot::retain(store, &offer, &input(snapshot, CAPACITY)?, &scope)?;
    let ack = crate::body_join::sign_durable_network_custody(binding, continuity, &token)?;
    exchange(
        &token,
        binding_scope.credential(),
        &ack,
        |request| crate::body_join::http_binary(peer, "/network/custody/ack", request),
        || std::thread::sleep(std::time::Duration::from_secs(2)),
    )?;
    println!(
        "network snapshot retained; Root-signed committed custody retained; no erase requested"
    );
    Ok(())
}
pub(super) fn exchange(
    token: &VerifiedDurableSnapshot,
    credential: &MembershipCredential,
    ack: &[u8],
    mut post: impl FnMut(&[u8]) -> Result<(u16, Vec<u8>)>,
    mut pause: impl FnMut(),
) -> Result<()> {
    token.revalidate()?;
    if ack.len() != REQUEST
        || token
            .encode_fields(credential, ack[564..].try_into().unwrap())?
            .as_slice()
            != ack
    {
        return Err("network durable ACK scope".into());
    }
    let ack_path = token.directory.join("ack.bin");
    immutable(&ack_path, ack)?;
    File::open(&token.directory)?.sync_all()?;
    if reopen(&ack_path, REQUEST)? != ack {
        return Err("network durable ACK changed".into());
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    let mut last = None;
    // Let the absolute deadline govern 150 two-second polling intervals;
    // fast 202 replies must not exhaust attempts before Root's 300-second job.
    const MAX_ATTEMPTS: usize = 151;
    for attempt in 0..MAX_ATTEMPTS {
        if std::time::Instant::now() >= deadline {
            return Err("network custody absolute deadline".into());
        }
        // Reopen before EVERY retry; no new signature or identity is generated.
        token.revalidate()?;
        if reopen(&ack_path, REQUEST)? != ack {
            return Err("network retry ACK changed".into());
        }
        match post(ack) {
            Ok((200, response)) => {
                let receipt = token.verify_committed_receipt(credential, ack, &response)?;
                let id = digest(receipt.bytes())
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                let json = token.directory.join(format!("receipt-{id}.json"));
                let cursor = token.directory.join(format!("receipt-{id}.bin"));
                immutable(&json, &response)?;
                immutable(&cursor, receipt.bytes())?;
                File::open(&token.directory)?.sync_all()?;
                let reopened = reopen(&json, response.len())?;
                let checked = token.verify_committed_receipt(credential, ack, &reopened)?;
                if reopen(&cursor, 480)?.as_slice() != checked.bytes() {
                    return Err("network receipt durable cursor differs".into());
                }
                return Ok(());
            }
            Ok((202 | 503, _)) => last = Some("network custody still pending".into()),
            Ok((status, _)) => return Err(format!("network custody HTTP status {status}").into()),
            Err(error) => last = Some(error),
        }
        if attempt + 1 < MAX_ATTEMPTS {
            pause();
        }
    }
    Err(last.unwrap_or_else(|| "network custody retry bound".into()))
}
