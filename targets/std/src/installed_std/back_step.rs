//! Direct finite-Step dispatch for every Back installed in the std profile.

use super::back_kind::InstalledBack;
use conduit_kernel::scheduler::{
    AssignedTerminalTransduction, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::RequestId;

macro_rules! installed_step_dispatch {
    ($( $(#[$attribute:meta])* $variant:ident ),+ $(,)?) => {
        impl<const PORTS: usize> StepBack<PORTS> for InstalledBack {
            fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
                match self {
                    Self::NativeSpeech(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::TypedState(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::DurableState(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::MidiInput(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::FlowCollect(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::BodyScan(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::FlowJoinByKey(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::TimeWindow(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::terminal_transductions(operation.as_ref()),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => {
                            StepBack::<PORTS>::terminal_transductions(operation)
                        }
                    )+
                    Self::Inactive => [None; PORTS],
                }
            }

            fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
                match self {
                    Self::NativeSpeech(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::TypedState(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::DurableState(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::MidiInput(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::FlowCollect(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::BodyScan(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::FlowJoinByKey(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::TimeWindow(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::terminal_transduction(operation.as_ref()),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => {
                            StepBack::<PORTS>::terminal_transduction(operation)
                        }
                    )+
                    Self::Inactive => None,
                }
            }

            fn step_committed(&mut self) {
                match self {
                    Self::NativeSpeech(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::TypedState(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::DurableState(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::MidiInput(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::FlowCollect(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::BodyScan(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::FlowJoinByKey(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::TimeWindow(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => StepBack::<PORTS>::step_committed(operation),
                    )+
                    Self::Inactive => {}
                }
            }

            fn step(
                &mut self,
                io: &mut StepIo<PORTS>,
                input_bytes: &StepInputBytes<'_, PORTS>,
            ) -> StepOutcome {
                match self {
                    Self::NativeSpeech(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::TypedState(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::DurableState(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::MidiInput(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::FlowCollect(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::BodyScan(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::FlowJoinByKey(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::TimeWindow(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => StepBack::<PORTS>::step(operation, io, input_bytes),
                    )+
                    Self::Inactive => StepOutcome::Complete,
                }
            }

            fn prepared_output(&self, port: conduit_kernel::PortId) -> Option<&[u8]> {
                match self {
                    Self::NativeSpeech(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::TypedState(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::DurableState(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::MidiInput(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::FlowCollect(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::BodyScan(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::FlowJoinByKey(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::TimeWindow(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::prepared_output(operation.as_ref(), port),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => StepBack::<PORTS>::prepared_output(operation, port),
                    )+
                    Self::Inactive => None,
                }
            }

            fn accepts_input_while_host_call_pending(&self) -> bool {
                match self {
                    Self::NativeSpeech(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::TypedState(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::DurableState(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::MidiInput(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::FlowCollect(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::BodyScan(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::FlowJoinByKey(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::TimeWindow(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => {
                            StepBack::<PORTS>::accepts_input_while_host_call_pending(operation)
                        }
                    )+
                    Self::Inactive => false,
                }
            }

            fn retains_host_call_input(
                &self,
                request: RequestId,
                value: conduit_kernel::ValueRef,
            ) -> bool {
                match self {
                    Self::NativeSpeech(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::TypedState(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::DurableState(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::MidiInput(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::FlowCollect(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::BodyScan(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::FlowJoinByKey(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::TimeWindow(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => StepBack::<PORTS>::retains_host_call_input(operation, request, value),
                    )+
                    Self::Inactive => false,
                }
            }

            fn cancel(&mut self) {
                match self {
                    Self::NativeSpeech(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::TypedState(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::DurableState(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::MidiInput(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::FlowCollect(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::BodyScan(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::FlowJoinByKey(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::TimeWindow(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => StepBack::<PORTS>::cancel(operation),
                    )+
                    Self::Inactive => {}
                }
            }
        }
    };
}

installed_step_dispatch!(
    DistanceFrequency,
    #[cfg(any(test, feature = "local-model-proof"))]
    RecordedSpeech,
    WhisperSpeech,
    MicrophoneClip,
    AddressDetect,
    RecognitionText,
    RecognizedTurnCommit,
    SpeechWindowToClip,
    SpeechResultToEventStream,
    KeyboardInput,
    ButtonInput,
    Tick,
    PulseObserve,
    #[cfg(test)]
    TestPulseSink,
    TimeDebounce,
    TimeTimeout,
    TimeDelay,
    TimeThrottle,
    TimeDeadline,
    TimeSample,
    Recurrence,
    CalendarProposal,
    CalendarProvider,
    TickPresentation,
    BoolPresentation,
    OrbiumSeed,
    LeniaStep,
    ScalarFieldPresentation,
    TextLiteral,
    TextUpper,
    TextJoin,
    TextPresentation,
    TextState,
    StateCount,
    StateToggle,
    CountPresentation,
    FlowBackpressure,
    FlowCoalesceLatest,
    StateLatestScalar,
    FlowTeeScalar,
    StateSelectScalar,
    CurrentSample,
    CombineLatest,
    TodoCombine,
    TodoCheckpoint,
    FlowZip,
    FlowGateScalar,
    FlowFirst,
    KeyEventTee,
    InputKeymap,
    InputChords,
    InstrumentMap,
    RhythmCompare,
    PatternComparison,
    SequenceNormalization,
    FinalNormalizedPattern,
    TimedPattern,
    TimedButtonAttempt,
    TemplateStorage,
    DataSaveText,
    DataLoadText,
    LogicCompareScalar,
    LogicNot,
    LogicSelectScalar,
    MathScalar,
    Layout,
    PresentationComposition,
    GraphicsPresentation,
    #[cfg(test)]
    TestPresentationSink,
    #[cfg(test)]
    TestLayoutSink,
    RoboticsSource,
    RoboticsDrive,
    MusicSynth,
    SpeechSynthesis,
    SpokenPresentationRequest,
    GeneratedValidationEnvelope,
    GeneratedSemanticValidator,
    RetainGeneratedValidation,
    SpokenGeneratedSpeech,
    DirectFaceWording,
    SpokenArtifact,
    SpokenArtifactShow,
    SpokenNoInteraction,
    PresentationTee,
    AudioRenderDemand,
    AudioPlay,
    AudioTone,
    WavArtifact,
    PcmProfileConversion,
    MidiOutput,
    ExternalWebSocketListener,
    HousePrompt,
    BodyChatPrompt,
    BodyConversationContext,
    LocalModel,
    LocalVision,
    VisionDescribe,
    VisionExperience,
    ModelText,
    GeneratedSpeechCommit,
    Navigation,
    VectorSearch,
    HttpClient,
    HttpServer,
    Json,
    ImageText,
    ImageTextRecord,
    TypedRecord,
    RecordSingletonStream,
    RecordExactlyOne,
    RecordQueue,
    RecordDeliveryStatus,
    RecordTranscript,
    StructuredSelector,
    PureFilter,
    PureExpression,
    StructuredLiteral,
    StructuredPresentation,
    #[cfg(test)]
    TestTextSource,
    #[cfg(test)]
    TestMidiSource,
    #[cfg(test)]
    TestRecurrenceSink,
    #[cfg(any(test, feature = "local-model-proof"))]
    TestSpeechSink,
    #[cfg(test)]
    TestJsonSource,
    #[cfg(test)]
    TestJsonSink,
    #[cfg(test)]
    TestStructuredSource,
    #[cfg(test)]
    TestStructuredSink,
    #[cfg(any(test, feature = "local-model-proof"))]
    TestLocalModelSource,
    #[cfg(any(test, feature = "local-model-proof"))]
    TestLocalModelSink,
    #[cfg(test)]
    TestKeyEventSource,
    #[cfg(test)]
    TestChordSink,
    #[cfg(test)]
    TestScalarSource,
    #[cfg(test)]
    TestScalarLiteral,
    #[cfg(test)]
    TestScalarSink,
    #[cfg(test)]
    TestFrequencySource,
    #[cfg(test)]
    TestDistanceSource,
    #[cfg(test)]
    TestTonePcmSink,
    #[cfg(test)]
    TestCancellationSource,
    #[cfg(test)]
    TestToneTerminalRecovery,
    #[cfg(test)]
    TestNormalCloseSink,
    #[cfg(test)]
    TestDataTerminalRecovery,
    #[cfg(test)]
    TestDataTextSink,
    #[cfg(test)]
    TestDataTextSource,
    #[cfg(test)]
    TestGateScript,
    #[cfg(test)]
    TestLogicScript,
    #[cfg(test)]
    TestLogicSink,
    #[cfg(test)]
    TestSlowScalarSink,
    #[cfg(test)]
    TestTimingSink,
    #[cfg(test)]
    TestTimingSource,
    #[cfg(test)]
    TestObserver,
);
