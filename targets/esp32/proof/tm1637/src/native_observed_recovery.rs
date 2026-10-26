//! Read-only reopening of a bounded observation outbox. The media scope is
//! recovered data, not authenticated identity or permission to sign it.
use conduit_environment_runtime::{
    display_evidence_journal::{Journal, Storage},
    observed_record::{self, ObservedRecord},
};
pub fn reopen<'a, S: Storage>(
    storage: S,
    basis: &'a [u8],
    work: &mut [u8],
) -> Result<(Journal<'a, S>, [u8; 96], u64), &'static str> {
    reopen_profile(storage, basis, work, false)
}
pub fn reopen_profile<'a, S: Storage>(
    mut storage: S,
    basis: &'a [u8],
    work: &mut [u8],
    signed: bool,
) -> Result<(Journal<'a, S>, [u8; 96], u64), &'static str> {
    let mut scope = [0; 96];
    storage
        .read(4, &mut scope)
        .map_err(|_| "observed-history-read")?;
    let mut journal = Journal::recover(storage, &scope, basis, 960, work)
        .map_err(|_| "observed-history-recover")?;
    let mut cursor = journal.replay_cursor();
    let mut bytes = [0; observed_record::BYTES + 64];
    let length = if signed {
        bytes.len()
    } else {
        observed_record::BYTES
    };
    let mut sequence = 0;
    let mut clock = 0;
    while let Some(n) = journal
        .replay(&mut cursor, &mut bytes[..length], work)
        .map_err(|_| "observed-history-read")?
    {
        if n != length {
            return Err("observed-history-record");
        }
        let record = ObservedRecord::decode(&bytes[..observed_record::BYTES])
            .map_err(|_| "observed-history-record")?;
        if record.sequence != sequence + 1 || record.observed_at_boot_ms < clock {
            return Err("observed-history-sequence");
        }
        sequence = record.sequence;
        clock = record.observed_at_boot_ms;
    }
    Ok((journal, scope, sequence))
}
