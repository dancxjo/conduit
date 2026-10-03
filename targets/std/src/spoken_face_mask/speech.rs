use super::*;

/// One source-correlated input to an admitted `speech/synthesize-stream` route.
/// `segment` is a real Tongues value, not a flattened whole-view string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenSegment {
    pub face_id: String,
    pub face_revision: u64,
    pub show_id: String,
    pub clause_index: Option<usize>,
    pub clause_provenance: Option<FaceUtteranceProvenance>,
    pub segment: SpeakableSegment,
    pub text_sha256: String,
}

impl SpokenSegment {
    /// The exact portable value for an already planned streaming synthesis
    /// input. Encoding failure is a refusal, never a text-only substitute.
    pub fn encode_tongues(&self) -> Result<Vec<u8>, SpokenFaceRefusal> {
        conduit_tongues::encode_speakable_segment(&self.segment)
            .map_err(|_| SpokenFaceRefusal::InvalidValue)
    }
}

/// This receipt is supplied only by the selected audio effect after its output
/// has completed. It proves produced PCM, never speaker playback or hearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenAudioReceipt {
    pub stream_identity: String,
    pub sequence: u32,
    /// The Face Show that made this spoken reading relevant.
    pub source_show_id: String,
    /// Identities of the *speech* Plan/Play, which may differ from the source
    /// graphical or terminal Show's Plan/Play.
    pub speech_plan_id: String,
    pub speech_play_id: String,
    pub text_sha256: String,
    pub provider_sha256: String,
    pub pcm_sha256: String,
    pub pcm_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpokenDelivery {
    Completed(SpokenAudioReceipt),
    Cancelled,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenTurnReceipt {
    pub face_id: String,
    pub face_revision: u64,
    pub show_id: String,
    pub completed_segments: u32,
    pub produced_pcm_bytes: u64,
    pub provider_sha256: Option<String>,
    pub correlation_sha256: String,
    pub outcome: SpokenTurnOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpokenTurnOutcome {
    Completed,
    Cancelled,
    Failed(String),
}

pub(super) fn split_at_char_boundary(text: &str, maximum: usize) -> usize {
    let mut cut = text.len().min(maximum);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    if cut < text.len() {
        if let Some((index, boundary)) = text[..cut]
            .char_indices()
            .rev()
            .find(|(index, character)| *index >= cut / 2 && character.is_whitespace())
        {
            cut = index + boundary.len_utf8();
        }
    }
    cut
}
