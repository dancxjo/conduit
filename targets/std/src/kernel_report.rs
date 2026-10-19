//! Finished native execution report; evidence allocation occurs after Play.
use crate::{ContentEvidenceReport, ExternalForeStreamStats};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdKernelExecutionReport {
    pub content_evidence: ContentEvidenceReport,
    pub external_fore_stream: Option<ExternalForeStreamStats>,
    pub active_play: conduit_core::ActivePlayIdentity,
    pub active_play_id: conduit_core::ActivePlayId,
    pub decisions: u32,
    pub logical_invocations: u64,
    pub kernel_events: u16,
    pub kernel_sign: Vec<conduit_kernel::KernelEvent>,
    pub value_allocation_capacity_before: (usize, usize),
    pub value_allocation_capacity_after: (usize, usize),
    pub face_ids: Vec<conduit_core::FaceId>,
    pub playback: Vec<crate::hosted_audio::PlaybackReport>,
    pub wav_artifacts: Vec<crate::hosted_wav_artifact::WavArtifactReport>,
    pub midi_input: Vec<crate::hosted_midi::MidiInputReport>,
    pub midi_output: Vec<crate::hosted_midi::MidiOutputReport>,
    pub identity: conduit_plan_lowering::lowering::KernelExecutionIdentityMap,
    pub fore_endpoints: Vec<conduit_plan_lowering::lowering::KernelForeEndpointIdentity>,
    #[cfg(test)]
    pub post_play_start_allocations: usize,
}
