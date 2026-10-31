//! Preparation-only correspondence between authenticated evidence and original
//! producer identity. This grants no execution, storage custody, clock or ACK.
use super::*;
use crate::local_recorder::ValidatedBinding;
use conduit_core::ActivePlayIdentity;
use conduit_environment_runtime::recording::{RecordingContext, RecordingTimestampBasis};
use std::os::unix::fs::MetadataExt;

const MAX_ENTRIES: usize = 960;
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Pins,
    Scope,
    ActivePlay,
    Prefix,
    Empty,
    Capacity,
}

#[derive(Debug)]
struct CapturedPrefix {
    count: u32,
    digest: [u8; 32],
    watermark: u32,
    records: Box<[u8]>,
}
#[derive(Debug)]
pub(crate) struct VerifiedProducerContext<'a> {
    active: &'a ActivePlayIdentity,
    prefix: CapturedPrefix,
}
impl<'a> VerifiedProducerContext<'a> {
    /// Capture a finite prefix from retained immutable batch files. Later
    /// directory additions are outside this preparation witness.
    /// Candidate identities are only preimages: they cannot replace the pinned
    /// membership key/Body/Host or the selected collector's signed scope.
    pub(crate) fn prepare(
        binding: &ValidatedBinding,
        selected: &Collector,
        candidate: &'a ActivePlayIdentity,
        directory: &Path,
    ) -> std::result::Result<Self, Refusal> {
        if selected.key.to_bytes() != *binding.root()
            || &candidate.host_id != binding.producer_host()
        {
            return Err(Refusal::Pins);
        }
        for (expected, value) in selected.scope.as_chunks::<32>().0.iter().zip([
            binding.credential().body_id.as_str(),
            candidate.boot_id.as_str(),
            candidate.active_play_id.as_str(),
        ]) {
            if expected[..] != Sha256::digest(value.as_bytes())[..] {
                return Err(Refusal::Scope);
            }
        }
        if !candidate.is_bound_to(
            &candidate.plan_id,
            binding.producer_host(),
            &candidate.boot_id,
            candidate.play_sequence,
        ) {
            return Err(Refusal::ActivePlay);
        }
        // Verify the captured selected prefix in one bounded read-only pass.
        // A later producer watermark or directory append does not drain it.
        let prefix = complete_prefix(selected, directory)?;
        if prefix.count == 0 {
            return Err(Refusal::Empty);
        }
        Ok(Self {
            active: candidate,
            prefix,
        })
    }
    pub(crate) fn context(&self) -> RecordingContext<'a> {
        RecordingContext {
            timestamp_basis: RecordingTimestampBasis::ProducerObservation,
            host: &self.active.host_id,
            boot: &self.active.boot_id,
            plan: &self.active.plan_id,
            play: &self.active.active_play_id,
        }
    }
    /// Exact signed records captured in the verification pass. A later append,
    /// replacement or deletion cannot substitute another input after checking.
    /// Root must include this finite retained storage in its preparation budget.
    pub(crate) fn records(&self) -> impl ExactSizeIterator<Item = ObservedRecord> + '_ {
        self.prefix
            .records
            .as_chunks::<{ observed_record::BYTES }>()
            .0
            .iter()
            .map(|bytes| ObservedRecord::decode(bytes).expect("captured canonical signed record"))
    }
    pub(crate) fn retained_record_bytes(&self) -> usize {
        self.prefix.records.len()
    }
    /// Signed producer watermark may exceed the retained selected prefix.
    pub(crate) fn producer_committed_watermark(&self) -> u32 {
        self.prefix.watermark
    }
    pub(crate) fn verified_prefix(&self) -> (u32, [u8; 32]) {
        (self.prefix.count, self.prefix.digest)
    }
}
fn private(metadata: &std::fs::Metadata, directory: bool) -> bool {
    // SAFETY: geteuid observes only the current process identity.
    metadata.uid() == unsafe { libc::geteuid() }
        && metadata.mode() & 0o077 == 0
        && if directory {
            metadata.is_dir()
        } else {
            metadata.is_file()
        }
}
fn complete_prefix(
    selected: &Collector,
    directory: &Path,
) -> std::result::Result<CapturedPrefix, Refusal> {
    let metadata = std::fs::symlink_metadata(directory).map_err(|_| Refusal::Prefix)?;
    if !private(&metadata, true) {
        return Err(Refusal::Prefix);
    }
    let mut paths = Vec::new();
    for (index, entry) in std::fs::read_dir(directory)
        .map_err(|_| Refusal::Prefix)?
        .enumerate()
    {
        if index == MAX_ENTRIES {
            return Err(Refusal::Capacity);
        }
        let path = entry.map_err(|_| Refusal::Prefix)?.path();
        if path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name.starts_with("observed-history-") && name.ends_with(".json")
        }) {
            paths.push(path);
        }
    }
    paths.sort();
    let mut replay = Collector {
        after: 0,
        observed_ms: 0,
        ..*selected
    };
    let mut watermark = 0;
    let mut digest = Sha256::new();
    let mut records = Vec::new();
    for path in paths {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&path)
            .map_err(|_| Refusal::Prefix)?;
        if !private(&file.metadata().map_err(|_| Refusal::Prefix)?, false) {
            return Err(Refusal::Prefix);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(2049)
            .read_to_end(&mut bytes)
            .map_err(|_| Refusal::Prefix)?;
        let (after, clock) = replay.validate(&bytes).map_err(|_| Refusal::Prefix)?;
        if after == replay.after
            || path.file_name().unwrap()
                != std::ffi::OsStr::new(&format!(
                    "observed-history-{:04}-{after:04}.json",
                    replay.after
                ))
        {
            return Err(Refusal::Prefix);
        }
        let batch: Batch = serde_json::from_slice(&bytes).map_err(|_| Refusal::Prefix)?;
        watermark = watermark.max(batch.committed);
        let decoded = hex(&batch.records).map_err(|_| Refusal::Prefix)?;
        if decoded.len() > 960 * observed_record::BYTES - records.len() {
            return Err(Refusal::Capacity);
        }
        digest.update(&decoded);
        records.extend_from_slice(&decoded);
        replay.after = after;
        replay.observed_ms = clock;
    }
    Ok(CapturedPrefix {
        count: replay.after,
        digest: digest.finalize().into(),
        watermark,
        records: records.into_boxed_slice(),
    })
}
#[cfg(test)]
#[path = "producer_context_tests.rs"]
mod tests;
