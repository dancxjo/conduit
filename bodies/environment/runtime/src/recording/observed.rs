//! Preserve the shared observation record's exact bytes and event-time meaning.
//! Signature, original producer context and storage admission belong to callers.
use super::*;
use crate::observed_record::ObservedRecord;

impl PreparedRecordingChunk {
    pub fn append_observed(
        &mut self,
        record: &ObservedRecord,
    ) -> Result<ChunkAppend, RecordingRefusal> {
        if self.timestamp_basis != RecordingTimestampBasis::ProducerObservation {
            return Err(RecordingRefusal::ClockBasis);
        }
        self.append_entry(record.encode().map_err(RecordingRefusal::ObservedRecord)?)
    }
}
