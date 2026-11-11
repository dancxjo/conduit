use super::super::episode_retrieval_snapshot::EpisodeRetrievalSnapshotInput;
use super::*;
pub(super) fn field<'a>(v: View<'a>, key: &str) -> Result<View<'a>, R> {
    v.record_field(key)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
pub(super) fn leaf(v: View<'_>) -> Result<&[u8], R> {
    let n = v.value_node();
    if n.len() < 5
        || n[0] != 0
        || u32::from_le_bytes(n[1..5].try_into().unwrap()) as usize != n.len() - 5
    {
        return Err(R::Malformed);
    }
    Ok(&n[5..])
}
pub(super) fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > MAX - out.len() {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
pub(super) fn name(out: &mut Vec<u8>, s: &str) -> Result<(), R> {
    put(out, &(s.len() as u32).to_le_bytes())?;
    put(out, s.as_bytes())
}
pub(super) fn scalar(out: &mut Vec<u8>, b: &[u8]) -> Result<(), R> {
    put(out, &[0])?;
    put(out, &(b.len() as u32).to_le_bytes())?;
    put(out, b)
}
pub(super) fn record(out: &mut Vec<u8>, n: u32) -> Result<(), R> {
    put(out, &[2])?;
    put(out, &n.to_le_bytes())
}
pub(super) fn number(out: &mut Vec<u8>, n: u64) -> Result<(), R> {
    scalar(out, &n.to_le_bytes())
}
fn variant(out: &mut Vec<u8>, tag: &str) -> Result<(), R> {
    put(out, &[3])?;
    name(out, tag)
}
fn unit(out: &mut Vec<u8>, tag: &str) -> Result<(), R> {
    variant(out, tag)?;
    scalar(out, &[])
}
fn optional(out: &mut Vec<u8>, v: Option<u64>) -> Result<(), R> {
    match v {
        Some(n) => {
            variant(out, "some")?;
            number(out, n)
        }
        None => unit(out, "none"),
    }
}
pub(super) fn clock(
    out: &mut Vec<u8>,
    basis: conduit_ai::BorrowedTemporalClockBasis<'_>,
) -> Result<(), R> {
    match basis {
        conduit_ai::BorrowedTemporalClockBasis::UnixEpochMilliseconds => {
            unit(out, "unix_epoch_milliseconds")
        }
        conduit_ai::BorrowedTemporalClockBasis::MonotonicMilliseconds(id) => {
            variant(out, "monotonic_milliseconds")?;
            record(out, 1)?;
            name(out, "identity")?;
            scalar(out, id.as_bytes())
        }
    }
}
pub(super) fn hex_identity(digest: [u8; 32]) -> [u8; 64] {
    let mut out = [0; 64];
    let hex = b"0123456789abcdef";
    for (i, b) in digest.iter().enumerate() {
        out[2 * i] = hex[(b >> 4) as usize];
        out[2 * i + 1] = hex[(b & 15) as usize];
    }
    out
}
pub(super) fn candidate_identity(key: EpisodeRetrievalCandidateKey) -> ([u8; 64], usize) {
    let mut out = [0; 64];
    let mut n = 0;
    let (episode, start) = decimal(key.episode);
    let (item, item_start) = decimal(u64::from(key.item));
    for bytes in [
        b"episode/".as_slice(),
        &episode[start..],
        b"/evidence/",
        &item[item_start..],
    ] {
        out[n..n + bytes.len()].copy_from_slice(bytes);
        n += bytes.len();
    }
    (out, n)
}
fn decimal(mut n: u64) -> ([u8; 20], usize) {
    let mut out = [0; 20];
    let mut start = 20;
    loop {
        start -= 1;
        out[start] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    (out, start)
}
pub(super) fn candidate(
    out: &mut Vec<u8>,
    key: EpisodeRetrievalCandidateKey,
    observed: u64,
    uncertainty: u64,
    input: EpisodeRetrievalSnapshotInput<'_>,
) -> Result<(), R> {
    record(out, 6)?;
    name(out, "boundary")?;
    unit(out, "none")?;
    name(out, "identity")?;
    let (id, n) = candidate_identity(key);
    scalar(out, &id[..n])?;
    name(out, "provenance")?;
    record(out, 10)?;
    name(out, "clock_basis")?;
    clock(out, input.clock_basis)?;
    for field in [
        "event_at",
        "ingested_at",
        "observed_at",
        "recorded_at",
        "reference_at",
        "retrieved_at",
        "uncertainty_millis",
        "valid_from",
        "valid_until",
    ] {
        name(out, field)?;
        match field {
            "observed_at" => optional(out, Some(observed))?,
            "reference_at" => number(out, input.reference.ticks)?,
            "retrieved_at" => number(out, input.retrieved.ticks)?,
            "uncertainty_millis" => optional(out, Some(uncertainty))?,
            _ => optional(out, None)?,
        }
    }
    name(out, "source")?;
    unit(out, "observed")?;
    name(out, "transition")?;
    unit(out, "none")?;
    name(out, "validity")?;
    unit(out, "unknown_whether_current")
}
