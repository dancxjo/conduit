//! Cancellation dispatch for installed operation-owned pending work.
use super::operation::InstalledOperation;
use conduit_kernel::Operation;

impl InstalledOperation {
    pub(super) fn cancel_installed(&mut self) {
        match self {
            Self::TypedState(operation) => operation.cancel(),
            Self::KeyboardInput(operation) => operation.cancel(),
            Self::ButtonInput(operation) => operation.cancel(),
            Self::ButtonMapper(_) => {}
            Self::Tick(operation) => operation.cancel(),
            Self::PulseObserve(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestPulseSink(_) => {}
            Self::TickPresentation(operation) => operation.cancel(),
            Self::BoolPresentation(operation) => operation.cancel(),
            Self::OrbiumSeed(_) => {}
            Self::LeniaStep(operation) => operation.cancel(),
            Self::ScalarFieldPresentation(operation) => operation.cancel(),
            Self::TimeDebounce(operation) => operation.cancel(),
            Self::TimeTimeout(operation) => operation.cancel(),
            Self::TimeDelay(operation) => operation.cancel(),
            Self::TimeThrottle(operation) => operation.cancel(),
            Self::Recurrence(_) => {}
            Self::CalendarProposal(_) => {}
            Self::CalendarProvider(operation) => operation.cancel(),
            Self::TextLiteral(_) => {}
            Self::TextUpper(operation) => operation.cancel(),
            Self::TextJoin(operation) => operation.cancel(),
            Self::TextPresentation(operation) => operation.cancel(),
            Self::StateCount(_) => {}
            Self::StateToggle(_) => {}
            Self::CountPresentation(operation) => operation.cancel(),
            Self::StateLatestScalar(operation) => operation.cancel(),
            Self::FlowTeeScalar(operation) => operation.cancel(),
            Self::StateSelectScalar(operation) => operation.cancel(),
            Self::FlowGateScalar(operation) => operation.cancel(),
            Self::KeyEventTee(operation) => operation.cancel(),
            Self::InputKeymap(operation) | Self::InputChords(operation) => operation.cancel(),
            Self::InstrumentMap(operation) => operation.cancel(),
            Self::RhythmCompare(operation) => operation.cancel(),
            Self::PatternComparison(operation) => operation.cancel(),
            Self::SequenceNormalization(operation) => operation.cancel(),
            Self::FinalNormalizedPattern(operation) => operation.cancel(),
            Self::TimedPattern(operation) => operation.cancel(),
            Self::TimedButtonAttempt(operation) => operation.cancel(),
            Self::TemplateStorage(operation) => operation.cancel(),
            Self::LogicCompareScalar(operation) => operation.cancel(),
            Self::LogicNot(operation) => operation.cancel(),
            Self::LogicSelectScalar(operation) => operation.cancel(),
            Self::MathScalar(operation) => operation.cancel(),
            Self::Layout(operation) => operation.cancel(),
            Self::PresentationComposition(operation) => operation.cancel(),
            Self::GraphicsPresentation(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestPresentationSink(_) => {}
            #[cfg(test)]
            Self::TestLayoutSink(_) => {}
            Self::RoboticsSource(operation) => operation.cancel(),
            Self::RoboticsDrive(operation) => operation.cancel(),
            Self::MusicSynth(operation) => operation.cancel(),
            Self::AudioRenderDemand(operation) => operation.cancel(),
            Self::AudioPlay(operation) => operation.cancel(),
            Self::MidiOutput(operation) => operation.cancel(),
            Self::MidiInput(operation) => operation.cancel(),
            Self::ExternalWebSocketListener(operation) => operation.cancel(),
            Self::GenerateText(operation) => operation.cancel(),
            Self::LocalModel(operation) => operation.cancel(),
            Self::VectorSearch(operation) => operation.cancel(),
            Self::HttpClient(operation) => operation.cancel(),
            Self::HttpServer(operation) => operation.cancel(),
            Self::Json(operation) => operation.cancel(),
            Self::ImageText(operation) => operation.cancel(),
            Self::ImageTextRecord(operation) => operation.cancel(),
            Self::TypedRecordFrame(operation) => operation.cancel(),
            Self::StructuredSelector(operation) => operation.cancel(),
            Self::StructuredLiteral(_) => {}
            Self::StructuredPresentation(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestTextSource(_) => {}
            #[cfg(test)]
            Self::TestMidiSource(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestRecurrenceSink(_) => {}
            Self::TestPcmSource(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestJsonSource(_) => {}
            #[cfg(test)]
            Self::TestJsonSink(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestStructuredSource(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestStructuredSink(_) => {}
            #[cfg(any(test, feature = "local-model-proof"))]
            Self::TestLocalModelSource(_) | Self::TestLocalModelSink(_) => {}
            #[cfg(test)]
            Self::TestKeyEventSource(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestChordSink(_) => {}
            #[cfg(test)]
            Self::TestScalarSource(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestScalarLiteral(_) => {}
            #[cfg(test)]
            Self::TestScalarSink(_) => {}
            #[cfg(test)]
            Self::TestGateScript(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestLogicScript(_) => {}
            #[cfg(test)]
            Self::TestLogicSink(_) => {}
            #[cfg(test)]
            Self::TestSlowScalarSink(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestTimingSink(_) => {}
            #[cfg(test)]
            Self::TestTimingSource(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestObserver(operation) => operation.cancel(),
            Self::Inactive => {}
        }
    }
}
