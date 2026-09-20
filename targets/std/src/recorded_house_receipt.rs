#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RecordedHouseProofReceipt {
    /// Identity of the source document actually checked for this proof run.
    pub executed_source_document_id: String,
    /// Identity of the exact checked form actually planned for this proof run.
    pub executed_checked_form_id: String,
    /// Honest provenance for the semantic topology behind the identities above.
    pub semantic_topology_origin: String,
    /// Reviewed entry Form selected from the checked proof source.
    pub executed_form_name: String,
    /// Exact reviewed source document checked, planned, and executed by this run.
    pub executed_source: String,
    /// Checked-in sources whose exact concatenation produced the source identity.
    pub reviewed_source_paths: Vec<String>,
    pub plan_id: String,
    pub play_id: String,
    pub whisper_implementation_id: String,
    pub local_model_implementation_id: String,
    pub clip_sha256: String,
    pub recognized_text_sha256: Option<String>,
    pub recognized_text_bytes: u16,
    pub response_sha256: String,
    pub response_bytes: u32,
    pub local_model_invocations: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct MicrophoneHouseProofReceipt {
    pub house: RecordedHouseProofReceipt,
    pub microphone_implementation_id: String,
    pub microphone_executable_sha256: String,
    pub microphone_base_identity: String,
    pub microphone_card_id: String,
    pub microphone_device: u16,
    pub capture_milliseconds: u32,
    pub raw_pcm_sha256: String,
    pub raw_pcm_bytes: u32,
    pub microphone_diagnostic_bytes: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct SpokenMicrophoneHouseProofReceipt {
    pub microphone_house: MicrophoneHouseProofReceipt,
    pub speech_implementation_id: String,
    pub source_pcm_frames: u32,
    pub conversion_implementation_id: String,
    pub target_pcm_frames: u64,
    pub playback_resource_pool_id: String,
    pub playback_blocks_committed: u64,
    pub playback_frames_committed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RecordedHouseWavJourneyReceipt {
    pub house: RecordedHouseProofReceipt,
    pub recognized_text: String,
    pub response_text: String,
    pub speech_implementation_id: String,
    pub source_pcm_frames: u32,
    pub conversion_implementation_id: String,
    pub target_pcm_frames: u64,
    pub wav_pcm_bytes: u32,
    pub wav_blocks_written: u16,
}
