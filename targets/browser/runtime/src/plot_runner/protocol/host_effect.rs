//! Platform effect envelopes, preserving their existing serialized contract.
use super::*;

#[derive(Debug, Serialize)]
pub(in crate::plot_runner) struct TourAudioIoEffect {
    pub(in crate::plot_runner) schema: &'static str,
    pub(in crate::plot_runner) effect_kind: &'static str,
    pub(in crate::plot_runner) source_document_id: String,
    pub(in crate::plot_runner) checked_plot_id: String,
    pub(in crate::plot_runner) expanded_plot_id: String,
    pub(in crate::plot_runner) plan_id: String,
    pub(in crate::plot_runner) fragment_id: String,
    pub(in crate::plot_runner) active_play_id: String,
    pub(in crate::plot_runner) placement_id: String,
    pub(in crate::plot_runner) host_id: String,
    pub(in crate::plot_runner) boot_id: String,
    pub(in crate::plot_runner) request_sequence: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::plot_runner) maximum_output_bytes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::plot_runner) maximum_gain_millionths: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::plot_runner) frame_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::plot_runner) source_interaction: Option<SourceInteractionEvidence>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(in crate::plot_runner) enum TourHostEffect {
    StemPrepare(Box<TourKeyEventEffect>),
    AudioCue(Box<crate::plot_runner::audio::AudioEffect>),
    AudioIo(Box<TourAudioIoEffect>),
    PitchTone(Box<crate::plot_runner::audio::PitchToneEffect>),
    ClockObservation(Box<TourKeyEventEffect>),
    CivilClockObservation(Box<TourKeyEventEffect>),
    Snapshot(Box<SnapshotEffect>),
    Manifestation(Box<TourEffect>),
    Timer(Box<TourTimerEffect>),
    KeyEvent(Box<TourKeyEventEffect>),
    PointerEvent(Box<TourKeyEventEffect>),
    ButtonTransition(Box<TourButtonTransitionEffect>),
    ApplicationEvent(Box<TourKeyEventEffect>),
    TutorialPresenterRequest(Box<TourKeyEventEffect>),
}

impl TourHostEffect {
    pub(in crate::plot_runner) fn attach_source_interaction(
        &mut self,
        source_interaction: SourceInteractionEvidence,
    ) {
        match self {
            Self::StemPrepare(effect) => effect.source_interaction = Some(source_interaction),
            Self::AudioCue(effect) => effect.source_interaction = Some(source_interaction),
            Self::AudioIo(effect) => effect.source_interaction = Some(source_interaction),
            Self::PitchTone(effect) => effect.source_interaction = Some(source_interaction),
            Self::ClockObservation(effect) | Self::CivilClockObservation(effect) => {
                effect.source_interaction = Some(source_interaction)
            }
            Self::Snapshot(effect) => effect.source_interaction = Some(source_interaction),
            Self::Manifestation(effect) => effect.source_interaction = Some(source_interaction),
            Self::Timer(effect) => effect.source_interaction = Some(source_interaction),
            Self::KeyEvent(effect) => effect.source_interaction = Some(source_interaction),
            Self::PointerEvent(effect) => effect.source_interaction = Some(source_interaction),
            Self::ButtonTransition(effect) => effect.source_interaction = Some(source_interaction),
            Self::ApplicationEvent(effect) => effect.source_interaction = Some(source_interaction),
            Self::TutorialPresenterRequest(effect) => {
                effect.source_interaction = Some(source_interaction)
            }
        }
    }
}
