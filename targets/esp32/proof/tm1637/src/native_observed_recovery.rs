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

/// Find reusable erased backing after complete, empty committed headers only.
/// Every previous byte remains sealed; any record, torn header, or dirty tail
/// refuses this recovery path. This grants no authority over an old scope.
pub fn empty_header_prefix<S: Storage>(
    storage: &mut S,
    basis: &[u8],
    work: &mut [u8],
) -> Result<u32, &'static str> {
    let (prefix, erased) = retained_header_prefix(storage, basis, work)?;
    if !erased {
        return Err("observed-history-not-empty");
    }
    Ok(prefix)
}

/// Locate the final scope after sealed empty headers. A nonempty final journal
/// remains read-only under its original scope; only an erased tail is reusable.
pub fn retained_header_prefix<S: Storage>(
    storage: &mut S,
    basis: &[u8],
    work: &mut [u8],
) -> Result<(u32, bool), &'static str> {
    struct Suffix<'a, S> {
        storage: &'a mut S,
        start: u32,
    }
    impl<S: Storage> Storage for Suffix<'_, S> {
        fn capacity(&self) -> u32 {
            self.storage.capacity() - self.start
        }
        fn read(
            &mut self,
            at: u32,
            out: &mut [u8],
        ) -> Result<(), conduit_environment_runtime::display_evidence_journal::Refusal> {
            self.storage.read(self.start + at, out)
        }
        fn program(
            &mut self,
            _: u32,
            _: &[u8],
        ) -> Result<(), conduit_environment_runtime::display_evidence_journal::Refusal> {
            Err(conduit_environment_runtime::display_evidence_journal::Refusal::Media)
        }
    }
    let mut start = 0;
    loop {
        let mut bytes = [0; 64];
        let mut erased = true;
        for at in (start..storage.capacity()).step_by(64) {
            let n = (storage.capacity() - at).min(64) as usize;
            storage
                .read(at, &mut bytes[..n])
                .map_err(|_| "observed-history-read")?;
            if bytes[..n].iter().any(|byte| *byte != 255) {
                erased = false;
                break;
            }
        }
        if erased {
            return Ok((start, true));
        }
        let tail = Suffix { storage, start };
        let (journal, _, sequence) = reopen_profile(tail, basis, work, true)?;
        if journal.committed_count() != 0 || sequence != 0 {
            return Ok((start, false));
        }
        let used = journal.retained_extent();
        if used == 0 {
            return Err("observed-history-empty-header");
        }
        start = start.checked_add(used).ok_or("observed-history-capacity")?;
    }
}
