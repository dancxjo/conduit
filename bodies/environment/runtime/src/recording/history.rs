//! Bounded ordered recovery of one exact producer's immutable chunks.
//! This workspace retains one chunk and an observed high-water mark, not a
//! manifest, storage acknowledgement, authentication claim or durable cursor.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub enum RecordingHistoryRefusal {
    ZeroChunks,
    Capacity,
    Preparation(RecordingRefusal),
    Content(RecordingReadRefusal),
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecoveredHistory {
    pub chunks: u16,
    pub samples: u32,
    /// The first accepted chunk proves no preceding history.
    pub first_sequence: Option<u64>,
    pub last_sequence: Option<u64>,
}

/// Root admits the complete maximum chunk count and expected producer context
/// before recovery. The browser's existing snapshot executor must independently
/// authorize and restore each exact selected generation. Incoming JSON cannot
/// select a producer or move this workspace to another Boot, Plan or Play.
pub struct PreparedRecordingHistory {
    reader: PreparedRecordingReader,
    maximum_chunks: u16,
    recovered: RecoveredHistory,
    previous: Option<(u64, u64)>,
}
impl PreparedRecordingHistory {
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        maximum_chunks: u16,
    ) -> Result<Self, RecordingHistoryRefusal> {
        if maximum_chunks == 0 {
            return Err(RecordingHistoryRefusal::ZeroChunks);
        }
        Ok(Self {
            reader: PreparedRecordingReader::prepare(context, contract)
                .map_err(RecordingHistoryRefusal::Preparation)?,
            maximum_chunks,
            recovered: RecoveredHistory::default(),
            previous: None,
        })
    }
    /// Accept the next chunk only after its complete content and boundary pass.
    /// Overlap/replay, reversed clocks, corruption, foreign context and capacity
    /// refusal preserve the prior loaded chunk and high-water mark. Gaps remain
    /// visible on samples; they never become fabricated observations.
    pub fn read_next(&mut self, json: &str) -> Result<(), RecordingHistoryRefusal> {
        self.read_from(json, self.recovered, self.previous)
    }
    /// Both workspaces are prepared together for the same context by the Back.
    /// Only its committed workspace supplies the next boundary.
    #[cfg(feature = "recording-kernel")]
    pub(super) fn stage_from(
        &mut self,
        committed: &Self,
        json: &str,
    ) -> Result<(), RecordingHistoryRefusal> {
        self.read_from(json, committed.recovered, committed.previous)
    }
    fn read_from(
        &mut self,
        json: &str,
        recovered: RecoveredHistory,
        previous: Option<(u64, u64)>,
    ) -> Result<(), RecordingHistoryRefusal> {
        if recovered.chunks == self.maximum_chunks {
            return Err(RecordingHistoryRefusal::Capacity);
        }
        self.reader
            .read_after(json, previous)
            .map_err(RecordingHistoryRefusal::Content)?;
        let first = self
            .reader
            .sample(0)
            .expect("validated nonempty chunk")
            .sequence;
        let last = self
            .reader
            .sample(self.reader.len() - 1)
            .expect("validated nonempty chunk");
        self.previous = Some((last.sequence, last.sampled_at_boot_ms));
        self.recovered = recovered;
        self.recovered.first_sequence.get_or_insert(first);
        self.recovered.last_sequence = Some(last.sequence);
        self.recovered.chunks += 1;
        // At most u16::MAX chunks of at most 64 samples fit comfortably in u32.
        self.recovered.samples += self.reader.len() as u32;
        Ok(())
    }
    pub fn recovered(&self) -> RecoveredHistory {
        self.recovered
    }
    pub fn loaded_samples(&self) -> usize {
        self.reader.len()
    }
    /// First-class observation quantities borrow the one retained workspace.
    /// Replaying this view does not advance recovery or imply publication.
    pub fn sample(&mut self, index: usize) -> Option<RecordedSample<'_>> {
        self.reader.sample(index)
    }
}
