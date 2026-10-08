//! Closed inventory of operations installed in the production std kernel profile.

use super::alife_backs::{LeniaStepBack, OrbiumSeedBack, ScalarFieldPresentationBack};
use super::audio_play_back::AudioPlayBack;
use super::audio_tone_back::AudioToneBack;
use super::body_chat_prompt_back::BodyChatPromptBack;
use super::body_conversation_context_back::BodyConversationContextBack;
use super::bool_presentation::BoolPresentationBack;
use super::calendar_proposal_back::CalendarProposalBack;
use super::calendar_provider_back::CalendarProviderBack;
use super::combine_latest_back::CombineLatestBack;
use super::count_backs::{CountPresentationBack, StateCountBack};
use super::current_sample_back::CurrentSampleBack;
use super::data_text_back::DataTextBack;
use super::distance_frequency_back::DistanceFrequencyBack;
use super::final_normalized_pattern_back::FinalNormalizedPatternBack;
use super::flow_first_back::FlowFirstBack;
use super::flow_gate_back::FlowGateScalarBack;
use super::flow_join_by_key_back::FlowJoinByKeyBack;
use super::flow_pressure_backs::FlowPressureBack;
use super::flow_state_backs::{FlowTeeScalarBack, StateLatestScalarBack};
use super::flow_zip_back::FlowZipBack;
use super::generated_speech_commit_back::GeneratedSpeechCommitBack;
use super::house_prompt_back::HousePromptBack;
use super::http::{HttpClientBack, HttpServerBack};
use super::image_text_back::ImageTextBack;
use super::image_text_record_back::ImageTextRecordBack;
use super::input_semantic_backs::{InputSemanticBack, KeyEventTeeBack};
use super::instrument_map_back::InstrumentMapBack;
use super::json_backs::JsonBack;
use super::keyboard_input_back::KeyboardInputBack;
use super::layout_backs::LayoutBack;
use super::local_model_back::LocalModelBack;
use super::local_vision_back::LocalVisionBack;
use super::logic_backs::{LogicCompareScalarBack, LogicNotBack, LogicSelectScalarBack};
use super::math_backs::MathScalarBack;
use super::microphone_clip_back::MicrophoneClipBack;
use super::midi_input_back::MidiInputBack;
use super::midi_output_back::MidiOutputBack;
use super::model_text_back::ModelTextBack;
use super::model_work_back::ModelWorkBack;
use super::navigation_backs::NavigationBack;
use super::pacing_backs::{DelayBack, ThrottleBack};
use super::pattern_comparison_back::PatternComparisonBack;
use super::pcm_profile_conversion_back::PcmProfileConversionBack;
use super::presentation_composition::{GraphicsPresentationBack, PresentationCompositionBack};
use super::pure_expression_back::PureExpressionBack;
use super::recognized_turn_commit_back::RecognizedTurnCommitBack;
use super::record_delivery_back::RecordDeliveryStatusBack;
use super::record_queue_back::RecordQueueBack;
use super::record_temporal_back::{RecordExactlyOneBack, RecordSingletonStreamBack};
use super::record_transcript_back::RecordTranscriptBack;
use super::recurrence_back::RecurrenceBack;
use super::render_demand_back::AudioRenderDemandBack;
use super::rhythm_compare_back::RhythmCompareBack;
use super::robotics_backs::{RoboticsDriveBack, RoboticsSourceBack};
use super::robotics_effect::SimulatedDriveEffect;
use super::sequence_normalization_back::SequenceNormalizationBack;
use super::speech_recognition_adapter_back::{
    SpeechResultToEventStreamBack, SpeechWindowToClipBack,
};
use super::speech_synthesis_back::SpeechSynthesisBack;
use super::spoken_mask_backs::{PresentationTeeBack, SpokenArtifactBack};
use super::state_select_back::StateSelectScalarBack;
use super::structured_selector_back::StructuredSelectorBack;
use super::structured_values_back::{StructuredLiteralBack, StructuredPresentationBack};
use super::synth_back::MusicSynthBack;
use super::template_storage_back::TemplateStorageBack;
#[cfg(test)]
use super::test_json_codec::{TestJsonSinkBack, TestJsonSourceBack};
#[cfg(any(test, feature = "local-model-proof"))]
use super::test_local_model_io::{TestLocalModelSinkBack, TestLocalModelSourceBack};
#[cfg(test)]
use super::test_structured_selector::{
    SinkBack as TestStructuredSinkBack, SourceBack as TestStructuredSourceBack,
};
use super::text_backs::{TextLiteralBack, TextPresentationBack, TextTransformBack};
use super::text_state_back::TextStateBack;
#[cfg(test)]
use super::tick_backs::TestObserverBack;
use super::tick_backs::TickBack;
use super::tick_presentation::TickPresentationBack;
use super::time_sample_back::TimeSampleBack;
use super::time_window_back::TimeWindowBack;
use super::timed_button_attempt_back::TimedButtonAttemptBack;
use super::timed_pattern_back::TimedPatternBack;
use super::timing_backs::{DeadlineBack, DebounceBack, TimeoutBack};
use super::toggle_back::StateToggleBack;
use super::typed_record_back::TypedRecordBack;
use super::vector_search_back::VectorSearchBack;
use super::vision_describe_back::VisionDescribeBack;
use super::vision_experience_back::VisionExperienceBack;
use super::wav_artifact_back::WavArtifactBack;
use conduit_data::FlowCollectBack;

pub(super) enum InstalledBack {
    NativeSpeech(Box<conduit_speech::kernel::NativeSpeechBack>),
    DistanceFrequency(DistanceFrequencyBack),
    #[cfg(any(test, feature = "local-model-proof"))]
    RecordedSpeech(super::recorded_speech_back::RecordedSpeechBack),
    WhisperSpeech(super::whisper_speech_back::WhisperSpeechBack),
    MicrophoneClip(MicrophoneClipBack),
    AddressDetect(super::address_detect_back::AddressDetectBack),
    RecognitionText(super::recognition_text_back::RecognitionTextBack),
    RecognizedTurnCommit(RecognizedTurnCommitBack),
    SpeechWindowToClip(SpeechWindowToClipBack),
    SpeechResultToEventStream(SpeechResultToEventStreamBack),
    TypedState(Box<crate::state_value::TypedStateBack>),
    DurableState(Box<crate::state_value::InstalledDurableStateBack>),
    KeyboardInput(KeyboardInputBack),
    ButtonInput(super::keyboard_input_back::button::ButtonBack),
    ButtonMapper(Box<super::keyboard_input_back::button::indicator::Mapper>),
    Tick(TickBack),
    PulseObserve(conduit_time::PulseObservationBack),
    #[cfg(test)]
    TestPulseSink(super::pulse_observation_sink::Sink),
    TimeDebounce(DebounceBack),
    TimeTimeout(TimeoutBack),
    TimeDelay(DelayBack),
    TimeThrottle(ThrottleBack),
    TimeDeadline(DeadlineBack),
    TimeSample(TimeSampleBack),
    TimeWindow(Box<TimeWindowBack>),
    Recurrence(RecurrenceBack),
    CalendarProposal(CalendarProposalBack),
    CalendarProvider(CalendarProviderBack),
    TickPresentation(TickPresentationBack),
    BoolPresentation(BoolPresentationBack),
    OrbiumSeed(OrbiumSeedBack),
    LeniaStep(LeniaStepBack),
    ScalarFieldPresentation(ScalarFieldPresentationBack),
    TextLiteral(TextLiteralBack),
    TextUpper(TextTransformBack),
    TextJoin(TextTransformBack),
    TextPresentation(TextPresentationBack),
    TextState(TextStateBack),
    StateCount(StateCountBack),
    StateToggle(StateToggleBack),
    CountPresentation(CountPresentationBack),
    FlowBackpressure(FlowPressureBack),
    FlowCoalesceLatest(FlowPressureBack),
    StateLatestScalar(StateLatestScalarBack),
    FlowTeeScalar(FlowTeeScalarBack),
    StateSelectScalar(StateSelectScalarBack),
    CurrentSample(CurrentSampleBack),
    CombineLatest(CombineLatestBack),
    FlowZip(FlowZipBack),
    FlowCollect(Box<FlowCollectBack>),
    FlowJoinByKey(Box<FlowJoinByKeyBack>),
    FlowGateScalar(FlowGateScalarBack),
    FlowFirst(FlowFirstBack),
    KeyEventTee(KeyEventTeeBack),
    InputKeymap(InputSemanticBack),
    InputChords(InputSemanticBack),
    InstrumentMap(InstrumentMapBack),
    RhythmCompare(RhythmCompareBack),
    PatternComparison(PatternComparisonBack),
    SequenceNormalization(SequenceNormalizationBack),
    FinalNormalizedPattern(FinalNormalizedPatternBack),
    TimedPattern(TimedPatternBack),
    TimedButtonAttempt(TimedButtonAttemptBack),
    TemplateStorage(TemplateStorageBack),
    DataSaveText(DataTextBack),
    DataLoadText(DataTextBack),
    LogicCompareScalar(LogicCompareScalarBack),
    LogicNot(LogicNotBack),
    LogicSelectScalar(LogicSelectScalarBack),
    MathScalar(MathScalarBack),
    Layout(LayoutBack),
    PresentationComposition(PresentationCompositionBack),
    GraphicsPresentation(GraphicsPresentationBack),
    #[cfg(test)]
    TestPresentationSink(super::presentation_composition::PresentationSinkBack),
    #[cfg(test)]
    TestLayoutSink(super::layout_backs::LayoutSinkBack),
    RoboticsSource(RoboticsSourceBack),
    RoboticsDrive(RoboticsDriveBack),
    MusicSynth(MusicSynthBack),
    SpeechSynthesis(SpeechSynthesisBack),
    SpokenPresentationRequest(crate::spoken_mask_runtime::PresentationToGenerativeRequestBack),
    GeneratedValidationEnvelope(super::generated_validation_backs::GeneratedValidationBack),
    GeneratedSemanticValidator(super::generated_validation_backs::GeneratedValidatorBack),
    RetainGeneratedValidation(super::generated_validation_backs::GeneratedValidationBack),
    SpokenGeneratedSpeech(crate::spoken_mask_runtime::GeneratedManifestationToSpeechBack),
    DirectFaceWording(crate::direct_spoken_mask_runtime::DirectFaceWordingBack),
    SpokenArtifact(SpokenArtifactBack),
    SpokenArtifactShow(crate::spoken_mask_runtime::ArtifactAcknowledgedShowBack),
    SpokenNoInteraction(crate::spoken_mask_runtime::ClosingNoInteractionBack),
    PresentationTee(PresentationTeeBack),
    AudioRenderDemand(AudioRenderDemandBack),
    AudioPlay(AudioPlayBack),
    AudioTone(AudioToneBack),
    WavArtifact(WavArtifactBack),
    PcmProfileConversion(PcmProfileConversionBack),
    MidiOutput(MidiOutputBack),
    MidiInput(Box<MidiInputBack>),
    ExternalWebSocketListener(super::external_websocket::ExternalWebSocketListenerBack),
    HousePrompt(HousePromptBack),
    BodyChatPrompt(BodyChatPromptBack),
    BodyConversationContext(BodyConversationContextBack),
    LocalModel(LocalModelBack),
    LocalVision(LocalVisionBack),
    VisionDescribe(VisionDescribeBack),
    VisionExperience(VisionExperienceBack),
    ModelText(ModelTextBack),
    GeneratedSpeechCommit(GeneratedSpeechCommitBack),
    Navigation(NavigationBack),
    VectorSearch(VectorSearchBack),
    ModelWork(ModelWorkBack),
    HttpClient(HttpClientBack),
    HttpServer(HttpServerBack),
    Json(JsonBack),
    ImageText(ImageTextBack),
    ImageTextRecord(ImageTextRecordBack),
    TypedRecord(TypedRecordBack),
    RecordSingletonStream(RecordSingletonStreamBack),
    RecordExactlyOne(RecordExactlyOneBack),
    RecordQueue(RecordQueueBack),
    RecordDeliveryStatus(RecordDeliveryStatusBack),
    RecordTranscript(RecordTranscriptBack),
    StructuredSelector(StructuredSelectorBack),
    PureFilter(StructuredSelectorBack),
    PureExpression(PureExpressionBack),
    StructuredLiteral(StructuredLiteralBack),
    StructuredPresentation(StructuredPresentationBack),
    #[cfg(test)]
    TestTextSource(super::test_text_source::TestTextSourceBack),
    #[cfg(test)]
    TestMidiSource(super::test_midi_source::TestMidiSourceBack),
    #[cfg(test)]
    TestRecurrenceSink(super::test_recurrence_sink::TestRecurrenceSinkBack),
    TestPcmSource(Box<super::test_audio_source::TestPcmSourceBack>),
    #[cfg(any(test, feature = "local-model-proof"))]
    TestSpeechSink(super::test_speech_sink::TestSpeechSinkBack),
    #[cfg(test)]
    TestJsonSource(TestJsonSourceBack),
    #[cfg(test)]
    TestJsonSink(TestJsonSinkBack),
    #[cfg(test)]
    TestStructuredSource(TestStructuredSourceBack),
    #[cfg(test)]
    TestStructuredSink(TestStructuredSinkBack),
    #[cfg(any(test, feature = "local-model-proof"))]
    TestLocalModelSource(TestLocalModelSourceBack),
    #[cfg(any(test, feature = "local-model-proof"))]
    TestLocalModelSink(TestLocalModelSinkBack),
    #[cfg(test)]
    TestKeyEventSource(super::test_input_semantics::TestKeyEventSourceBack),
    #[cfg(test)]
    TestChordSink(super::test_input_semantics::TestChordSinkBack),
    #[cfg(test)]
    TestScalarSource(super::test_scalar_flow::TestScalarSourceBack),
    #[cfg(test)]
    TestScalarLiteral(super::test_scalar_flow::TestScalarLiteralBack),
    #[cfg(test)]
    TestScalarSink(super::test_scalar_flow::TestScalarSinkBack),
    #[cfg(test)]
    TestFrequencySource(super::test_audio_tone::FrequencySourceBack),
    #[cfg(test)]
    TestDistanceSource(super::test_audio_tone::DistanceSourceBack),
    #[cfg(test)]
    TestTonePcmSink(super::test_audio_tone::TonePcmSinkBack),
    #[cfg(test)]
    TestCancellationSource(super::test_audio_tone::CancellationSourceBack),
    #[cfg(test)]
    TestToneTerminalRecovery(super::test_audio_tone::ToneTerminalRecoveryBack),
    #[cfg(test)]
    TestNormalCloseSink(super::test_audio_tone::NormalCloseSinkBack),
    #[cfg(test)]
    TestDataTerminalRecovery(super::test_data_terminal_recovery::DataTerminalRecoveryBack),
    #[cfg(test)]
    TestDataTextSink(super::test_data_terminal_recovery::DataTextSinkBack),
    #[cfg(test)]
    TestDataTextSource(super::test_data_terminal_recovery::DataTextSourceBack),
    #[cfg(test)]
    TestGateScript(super::test_gate::TestGateScriptBack),
    #[cfg(test)]
    TestLogicScript(super::test_logic::TestLogicScriptBack),
    #[cfg(test)]
    TestLogicSink(super::test_logic::TestLogicSinkBack),
    #[cfg(test)]
    TestSlowScalarSink(super::test_gate::TestSlowScalarSinkBack),
    #[cfg(test)]
    TestTimingSink(super::test_timing_sink::TestTimingSinkBack),
    #[cfg(test)]
    TestTimingSource(super::test_timing_sink::TestTimingSourceBack),
    #[cfg(test)]
    TestObserver(TestObserverBack),
    Inactive,
}

impl InstalledBack {
    pub(super) fn inactive() -> Self {
        Self::Inactive
    }

    pub(super) fn simulated_drive_effect(&self) -> Option<SimulatedDriveEffect> {
        match self {
            Self::RoboticsDrive(operation) => operation.effect(),
            _ => None,
        }
    }
}
