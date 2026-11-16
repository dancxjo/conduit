//! One explicit read of an admitted record Fore. No scan, replay or durable ACK.
use super::AuthorizedForeProbe;
use conduit_resource_snapshot::read_result::{decode, ReadResult, READ_RESULT_BYTES};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadSelection {
    pub(super) member_host: String,
    pub(super) index: u32,
}

/// Fixed canonical staging lives under the original caller envelope. The
/// original guard outlives bytes; this is not an additional memory grant.
pub(super) struct PendingRead {
    selection: ReadSelection,
    attempted: bool,
    bytes: [u8; READ_RESULT_BYTES],
    length: usize,
    _memory: conduit_std_host::body_execution::ImageMemoryRetention,
}
impl PendingRead {
    pub(super) fn new(
        selection: ReadSelection,
        memory: conduit_std_host::body_execution::ImageMemoryRetention,
    ) -> Self {
        Self {
            selection,
            attempted: false,
            bytes: [0; READ_RESULT_BYTES],
            length: 0,
            _memory: memory,
        }
    }
    pub(super) fn attempted(&self) -> bool {
        self.attempted
    }
    pub(super) fn member_host(&self) -> &str {
        &self.selection.member_host
    }
    pub(super) fn exchange(&mut self, probe: &mut AuthorizedForeProbe<'_>) -> Result<(), String> {
        if self.attempted {
            return Err("record read already attempted".into());
        }
        // Set before validation/IO: any refusal or unknown outcome is final.
        self.attempted = true;
        if !probe.binding().record_read_abi {
            return Err("original Fore is not the exact bounded record-read ABI".into());
        }
        let input = conduit_core::encode_count(u64::from(self.selection.index));
        let mut scratch = [0; conduit_body::MAX_PICO_ADMISSION_FRAME_BYTES];
        let mut refusal = None;
        let result = probe.exchange(
            &input,
            &mut scratch,
            std::time::Duration::from_millis(1000),
            |bytes| {
                if let Err(error) = self.accept(bytes) {
                    refusal = Some(error);
                    return Err(());
                }
                // Full validated bytes are owned before transport delivery ACK.
                // This grants no recording commitment, media erase or history scope.
                Ok(())
            },
        );
        if let Some(error) = refusal {
            return Err(error);
        }
        result
            .map(|_| ())
            .map_err(|error| format!("record Fore exchange: {error:?}"))
    }
    pub(super) fn retain_diagnostic_copy(&self, state: &std::path::Path) -> Result<(), String> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if self.length == 0 {
            return Ok(());
        }
        // Fixed owner-private evidence copy. Its presence is never a commit
        // receipt or permission to discard the producer's original record.
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(state.join("source-execution/record-read-result.bin"))
            .map_err(|error| format!("retain diagnostic record outcome: {error}"))?;
        file.write_all(&self.bytes[..self.length])
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("retain diagnostic record outcome: {error}"))
    }
    fn accept(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.length != 0 {
            return Err("record outcome staging is full".into());
        }
        correlate(self.selection.index, bytes)?;
        self.bytes[..bytes.len()].copy_from_slice(bytes);
        self.length = bytes.len();
        Ok(())
    }
}

fn correlate(requested: u32, bytes: &[u8]) -> Result<(), String> {
    let value = decode(bytes).map_err(|error| format!("record outcome schema: {error:?}"))?;
    match value {
        ReadResult::Recorded { index, .. } | ReadResult::Gap { index, .. }
            if index != requested =>
        {
            Err("record outcome belongs to another physical index".into())
        }
        ReadResult::End(end) if end > requested => {
            Err("record outcome ends before its captured prefix".into())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_index_and_prefix_are_checked_without_observation_sequence_guessing() {
        use conduit_resource_snapshot::read_result::Encoder;
        let mut encoder = Encoder::new().unwrap();
        let bytes = encoder
            .encode(ReadResult::Recorded {
                index: 3,
                captured_end: 9,
                record: &[1, 2],
            })
            .unwrap();
        assert!(correlate(3, bytes).is_ok());
        assert!(correlate(4, bytes).is_err());
        let bytes = encoder.encode(ReadResult::End(9)).unwrap();
        assert!(correlate(8, bytes).is_err());
        assert!(correlate(9, bytes).is_ok());
        assert!(correlate(10, bytes).is_ok());
    }
    #[test]
    fn canonical_non_record_outcome_is_preserved() {
        assert!(correlate(0, conduit_resource_snapshot::read_result::cancelled()).is_ok());
        assert!(correlate(5, b"old thermometer reading").is_err());
        assert!(correlate(0, &[0; READ_RESULT_BYTES + 1]).is_err());
    }
}
