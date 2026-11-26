//! Legacy public paths for the shared DHT11 recording codec and recovery.
//! Storage effects and authoritative publication observation remain local.
pub use conduit_dht11_observation::recording::{
    ChunkAppend, PreparedRecordingChunk, PreparedRecordingHistory, PreparedRecordingReader,
    PreparedRecordingSnapshot, RecordedSample, RecordingContext, RecordingDisposition,
    RecordingDispositionForm, RecordingHistoryRefusal, RecordingReadRefusal, RecordingRefusal,
    RecordingSnapshotRefusal, RecoveredHistory, MAXIMUM_CHUNK_BYTES, MAXIMUM_CHUNK_SAMPLES,
    SAMPLE_BYTES,
};
use conduit_dht11_observation::ObservationContract;
mod host_call;
pub use host_call::{RecordedRange, RecordingCommitObserver, RecordingSnapshotBack};
