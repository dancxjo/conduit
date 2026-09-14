#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RecordedHouseProofReceipt {
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
