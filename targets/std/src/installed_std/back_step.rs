//! Direct finite-Step dispatch for every Back installed in the std profile.

use super::back_kind::InstalledBack;
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::RequestId;

macro_rules! installed_step_dispatch {
    ($( $(#[$attribute:meta])* $variant:ident ),+ $(,)?) => {
        impl<const PORTS: usize> StepBack<PORTS> for InstalledBack {
            fn step_committed(&mut self) {
                match self {
                    Self::TypedState(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
                    Self::MidiInput(operation) => StepBack::<PORTS>::step_committed(operation.as_mut()),
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
                    Self::TypedState(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::MidiInput(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    Self::TestPcmSource(operation) => StepBack::<PORTS>::step(operation.as_mut(), io, input_bytes),
                    $(
                        $(#[$attribute])*
                        Self::$variant(operation) => StepBack::<PORTS>::step(operation, io, input_bytes),
                    )+
                    Self::Inactive => StepOutcome::Complete,
                }
            }

            fn accepts_input_while_host_call_pending(&self) -> bool {
                match self {
                    Self::TypedState(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
                    Self::MidiInput(operation) => StepBack::<PORTS>::accepts_input_while_host_call_pending(operation.as_ref()),
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
                    Self::TypedState(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
                    Self::MidiInput(operation) => StepBack::<PORTS>::retains_host_call_input(operation.as_ref(), request, value),
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
                    Self::TypedState(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::ButtonMapper(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
                    Self::MidiInput(operation) => StepBack::<PORTS>::cancel(operation.as_mut()),
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
    FlowGateScalar,
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
    AudioRenderDemand,
    AudioPlay,
    WavArtifact,
    PcmProfileConversion,
    MidiOutput,
    ExternalWebSocketListener,
    GenerateText,
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
