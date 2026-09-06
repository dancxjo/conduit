//! Closed inventory of operations installed in the production std kernel profile.

use super::alife_operations::{
    LeniaStepOperation, OrbiumSeedOperation, ScalarFieldPresentationOperation,
};
use super::audio_play_operation::AudioPlayOperation;
use super::bool_presentation::BoolPresentationOperation;
use super::calendar_proposal_operation::CalendarProposalOperation;
use super::calendar_provider_operation::CalendarProviderOperation;
use super::count_operations::{CountPresentationOperation, StateCountOperation};
use super::final_normalized_pattern_operation::FinalNormalizedPatternOperation;
use super::flow_gate_operation::FlowGateScalarOperation;
use super::flow_state_operations::{FlowTeeScalarOperation, StateLatestScalarOperation};
use super::generate_text::GenerateTextOperation;
use super::http::{HttpClientOperation, HttpServerOperation};
use super::input_semantic_operations::{InputSemanticOperation, KeyEventTeeOperation};
use super::instrument_map_operation::InstrumentMapOperation;
use super::json_operations::JsonOperation;
use super::keyboard_input_operation::KeyboardInputOperation;
use super::layout_operations::LayoutOperation;
use super::local_model_operation::LocalModelOperation;
use super::logic_operations::{
    LogicCompareScalarOperation, LogicNotOperation, LogicSelectScalarOperation,
};
use super::math_operations::MathScalarOperation;
use super::midi_input_operation::MidiInputOperation;
use super::midi_output_operation::MidiOutputOperation;
use super::pacing_operations::{DelayOperation, ThrottleOperation};
use super::pattern_comparison_operation::PatternComparisonOperation;
use super::presentation_composition::{
    GraphicsPresentationOperation, PresentationCompositionOperation,
};
use super::recurrence_operation::RecurrenceOperation;
use super::render_demand_operation::AudioRenderDemandOperation;
use super::rhythm_compare_operation::RhythmCompareOperation;
use super::robotics_effect::SimulatedDriveEffect;
use super::robotics_operations::{RoboticsDriveOperation, RoboticsSourceOperation};
use super::sequence_normalization_operation::SequenceNormalizationOperation;
use super::state_select_operation::StateSelectScalarOperation;
use super::structured_selector_operation::StructuredSelectorOperation;
use super::structured_values_operation::{
    StructuredLiteralOperation, StructuredPresentationOperation,
};
use super::synth_operation::MusicSynthOperation;
use super::template_storage_operation::TemplateStorageOperation;
#[cfg(test)]
use super::test_json_codec::{TestJsonSinkOperation, TestJsonSourceOperation};
#[cfg(any(test, feature = "local-model-proof"))]
use super::test_local_model_io::{TestLocalModelSinkOperation, TestLocalModelSourceOperation};
#[cfg(test)]
use super::test_structured_selector::{
    SinkOperation as TestStructuredSinkOperation, SourceOperation as TestStructuredSourceOperation,
};
use super::text_operations::{
    TextLiteralOperation, TextPresentationOperation, TextTransformOperation,
};
#[cfg(test)]
use super::tick_operations::TestObserverOperation;
use super::tick_operations::TickOperation;
use super::tick_presentation::TickPresentationOperation;
use super::timed_button_attempt_operation::TimedButtonAttemptOperation;
use super::timed_pattern_operation::TimedPatternOperation;
use super::timing_operations::{DebounceOperation, TimeoutOperation};
use super::toggle_operation::StateToggleOperation;
use super::vector_search_operation::VectorSearchOperation;
use conduit_kernel::{Failure, FailureCode, OperationAction};

pub(super) enum InstalledOperation {
    TypedState(crate::state_value::TypedStateOperation),
    KeyboardInput(KeyboardInputOperation),
    Tick(TickOperation),
    PulseObserve(super::pulse_observation_operation::PulseObservationOperation),
    #[cfg(test)]
    TestPulseSink(super::pulse_observation_sink::Sink),
    TimeDebounce(DebounceOperation),
    TimeTimeout(TimeoutOperation),
    TimeDelay(DelayOperation),
    TimeThrottle(ThrottleOperation),
    Recurrence(RecurrenceOperation),
    CalendarProposal(CalendarProposalOperation),
    CalendarProvider(CalendarProviderOperation),
    TickPresentation(TickPresentationOperation),
    BoolPresentation(BoolPresentationOperation),
    OrbiumSeed(OrbiumSeedOperation),
    LeniaStep(LeniaStepOperation),
    ScalarFieldPresentation(ScalarFieldPresentationOperation),
    TextLiteral(TextLiteralOperation),
    TextUpper(TextTransformOperation),
    TextJoin(TextTransformOperation),
    TextPresentation(TextPresentationOperation),
    StateCount(StateCountOperation),
    StateToggle(StateToggleOperation),
    CountPresentation(CountPresentationOperation),
    StateLatestScalar(StateLatestScalarOperation),
    FlowTeeScalar(FlowTeeScalarOperation),
    StateSelectScalar(StateSelectScalarOperation),
    FlowGateScalar(FlowGateScalarOperation),
    KeyEventTee(KeyEventTeeOperation),
    InputKeymap(InputSemanticOperation),
    InputChords(InputSemanticOperation),
    InstrumentMap(InstrumentMapOperation),
    RhythmCompare(RhythmCompareOperation),
    PatternComparison(PatternComparisonOperation),
    SequenceNormalization(SequenceNormalizationOperation),
    FinalNormalizedPattern(FinalNormalizedPatternOperation),
    TimedPattern(TimedPatternOperation),
    TimedButtonAttempt(TimedButtonAttemptOperation),
    TemplateStorage(TemplateStorageOperation),
    LogicCompareScalar(LogicCompareScalarOperation),
    LogicNot(LogicNotOperation),
    LogicSelectScalar(LogicSelectScalarOperation),
    MathScalar(MathScalarOperation),
    Layout(LayoutOperation),
    PresentationComposition(PresentationCompositionOperation),
    GraphicsPresentation(GraphicsPresentationOperation),
    #[cfg(test)]
    TestPresentationSink(super::presentation_composition::PresentationSinkOperation),
    #[cfg(test)]
    TestLayoutSink(super::layout_operations::LayoutSinkOperation),
    RoboticsSource(RoboticsSourceOperation),
    RoboticsDrive(RoboticsDriveOperation),
    MusicSynth(MusicSynthOperation),
    AudioRenderDemand(AudioRenderDemandOperation),
    AudioPlay(AudioPlayOperation),
    MidiOutput(MidiOutputOperation),
    MidiInput(Box<MidiInputOperation>),
    ExternalWebSocketListener(super::external_websocket::ExternalWebSocketListenerOperation),
    GenerateText(GenerateTextOperation),
    LocalModel(LocalModelOperation),
    VectorSearch(VectorSearchOperation),
    HttpClient(HttpClientOperation),
    HttpServer(HttpServerOperation),
    Json(JsonOperation),
    StructuredSelector(StructuredSelectorOperation),
    StructuredLiteral(StructuredLiteralOperation),
    StructuredPresentation(StructuredPresentationOperation),
    #[cfg(test)]
    TestTextSource(super::test_text_source::TestTextSourceOperation),
    #[cfg(test)]
    TestMidiSource(super::test_midi_source::TestMidiSourceOperation),
    #[cfg(test)]
    TestRecurrenceSink(super::test_recurrence_sink::TestRecurrenceSinkOperation),
    TestPcmSource(Box<super::test_audio_source::TestPcmSourceOperation>),
    #[cfg(test)]
    TestJsonSource(TestJsonSourceOperation),
    #[cfg(test)]
    TestJsonSink(TestJsonSinkOperation),
    #[cfg(test)]
    TestStructuredSource(TestStructuredSourceOperation),
    #[cfg(test)]
    TestStructuredSink(TestStructuredSinkOperation),
    #[cfg(any(test, feature = "local-model-proof"))]
    TestLocalModelSource(TestLocalModelSourceOperation),
    #[cfg(any(test, feature = "local-model-proof"))]
    TestLocalModelSink(TestLocalModelSinkOperation),
    #[cfg(test)]
    TestKeyEventSource(super::test_input_semantics::TestKeyEventSourceOperation),
    #[cfg(test)]
    TestChordSink(super::test_input_semantics::TestChordSinkOperation),
    #[cfg(test)]
    TestScalarSource(super::test_scalar_flow::TestScalarSourceOperation),
    #[cfg(test)]
    TestScalarLiteral(super::test_scalar_flow::TestScalarLiteralOperation),
    #[cfg(test)]
    TestScalarSink(super::test_scalar_flow::TestScalarSinkOperation),
    #[cfg(test)]
    TestGateScript(super::test_gate::TestGateScriptOperation),
    #[cfg(test)]
    TestLogicScript(super::test_logic::TestLogicScriptOperation),
    #[cfg(test)]
    TestLogicSink(super::test_logic::TestLogicSinkOperation),
    #[cfg(test)]
    TestSlowScalarSink(super::test_gate::TestSlowScalarSinkOperation),
    #[cfg(test)]
    TestTimingSink(super::test_timing_sink::TestTimingSinkOperation),
    #[cfg(test)]
    TestTimingSource(super::test_timing_sink::TestTimingSourceOperation),
    #[cfg(test)]
    TestObserver(TestObserverOperation),
    Inactive,
}

impl InstalledOperation {
    pub(super) fn inactive() -> Self {
        Self::Inactive
    }

    pub(super) fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    pub(super) fn simulated_drive_effect(&self) -> Option<SimulatedDriveEffect> {
        match self {
            Self::RoboticsDrive(operation) => operation.effect(),
            _ => None,
        }
    }
}
