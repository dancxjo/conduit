//! Current preparation evidence is separate from immutable prior Play receipts.
use super::Owner;
impl Owner {
    pub(in super::super) fn with_recording_stage_truth(
        &self,
        mut truth: serde_json::Value,
    ) -> serde_json::Value {
        match self.source_recording_stage_metadata() {
            Ok(Some(metadata)) => truth["source_recording_stage"] = metadata,
            Ok(None) => {}
            Err(error) => truth["source_recording_stage_refusal"] = error.into(),
        }
        truth
    }
}
