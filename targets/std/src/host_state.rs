//! Retained std Host state and one construction-default responsibility.
use super::*;

pub(super) struct TodoCheckpointResidenceRoot {
    pub(super) path: std::path::PathBuf,
    #[cfg(unix)]
    pub(super) device: u64,
    #[cfg(unix)]
    pub(super) inode: u64,
}

pub struct StdHost {
    pub(super) advertisement: HostAdvertisement,
    #[cfg(unix)]
    pub(super) terminal_attachment: Option<std::os::unix::net::UnixStream>,
    pub(super) image_identity: Option<conduit_host_make::ImageBootIdentity>,
    pub(super) playback: Option<hosted_audio::HostedPlaybackSelection>,
    pub(super) wav_artifact: Option<hosted_wav_artifact::WavArtifactSelection>,
    pub(super) speech_synthesis: Option<hosted_speech_synthesis::EspeakSpeechAdapter>,
    pub(super) midi_input: Option<hosted_midi::HostedRawMidiSelection>,
    pub(super) midi_output: Option<hosted_midi::MidiOutputSelection>,
    pub(super) local_model: Option<Box<dyn hosted_local_model::HostedLocalModelAdapter>>,
    pub(super) speech_recognition: Option<hosted_speech_recognition::WhisperSpeechAdapter>,
    pub(super) microphone: Option<hosted_microphone::AlsaMicrophoneAdapter>,
    pub(super) base_registry: conduit_core::BaseRegistry,
    pub(super) vector_search: Option<Box<dyn hosted_vector_search::HostedVectorSearchAdapter>>,
    pub(super) model_work: Option<Box<dyn hosted_model_work::HostedModelWorkAdapter>>,
    pub(super) calendar: Option<Box<dyn hosted_calendar::HostedCalendarAdapter>>,
    pub(super) body_conversation_context: Option<BodyConversationContextSource>,
    pub(super) vision: Option<hosted_vision::FiniteHostedVisionBase>,
    pub(super) kernel_resources: kernel_preparation::KernelResourceLedger,
    pub(super) numerical_preparation: numerical_host::preparation_cache::NumericalPreparationCache,
    pub(super) todo_checkpoint_root: Option<TodoCheckpointResidenceRoot>,
    pub(super) next_kernel_play_sequence: u64,
    pub(super) next_kernel_sign_sequence: u64,
}

pub(super) fn empty_base_registry() -> conduit_core::BaseRegistry {
    conduit_core::BaseRegistry::new(conduit_core::BaseRegistryLimits {
        maximum_bases: 16,
        maximum_capabilities_per_base: 16,
        maximum_resources_per_base: 16,
        maximum_advertised_capabilities: 256,
        maximum_advertised_resources: 256,
    })
    .expect("std Base registry bounds are fixed and valid")
}

impl StdHost {
    /// One state initializer after the caller has built its exact advertisement
    /// and resource ledger. This neither normalizes offers nor initializes any
    /// external Base; specialized constructors retain their selected owners.
    pub(crate) fn from_prepared_state(
        advertisement: HostAdvertisement,
        kernel_resources: kernel_preparation::KernelResourceLedger,
        base_registry: conduit_core::BaseRegistry,
    ) -> Self {
        Self {
            advertisement,
            #[cfg(unix)]
            terminal_attachment: None,
            image_identity: None,
            playback: None,
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry,
            vector_search: None,
            model_work: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            numerical_preparation: Default::default(),
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        }
    }
}
