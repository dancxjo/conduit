pub(super) use super::factory::{InstalledFactory, OperationBudget};
pub(super) use super::operation_kind::InstalledOperation;
use conduit_kernel::{Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef};

impl Operation for InstalledOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::KeyboardInput(operation) => operation.start(),
            Self::Tick(operation) => operation.start(),
            Self::TimeDebounce(operation) => operation.start(),
            Self::TimeTimeout(operation) => operation.start(),
            Self::TimeDelay(operation) => operation.start(),
            Self::TimeThrottle(operation) => operation.start(),
            Self::Recurrence(operation) => operation.start(),
            Self::CalendarProposal(operation) => operation.start(),
            Self::CalendarProvider(operation) => operation.start(),
            Self::TickPresentation(operation) => operation.start(),
            Self::BoolPresentation(operation) => operation.start(),
            Self::OrbiumSeed(operation) => operation.start(),
            Self::LeniaStep(operation) => operation.start(),
            Self::ScalarFieldPresentation(operation) => operation.start(),
            Self::TextLiteral(operation) => operation.start(),
            Self::TextUpper(operation) => operation.start(),
            Self::TextJoin(operation) => operation.start(),
            Self::TextPresentation(operation) => operation.start(),
            Self::StateCount(operation) => operation.start(),
            Self::StateToggle(operation) => operation.start(),
            Self::CountPresentation(operation) => operation.start(),
            Self::StateLatestScalar(operation) => operation.start(),
            Self::FlowTeeScalar(operation) => operation.start(),
            Self::StateSelectScalar(operation) => operation.start(),
            Self::FlowGateScalar(operation) => operation.start(),
            Self::KeyEventTee(operation) => operation.start(),
            Self::InputKeymap(operation) | Self::InputChords(operation) => operation.start(),
            Self::InstrumentMap(operation) => operation.start(),
            Self::RhythmCompare(operation) => operation.start(),
            Self::LogicCompareScalar(operation) => operation.start(),
            Self::LogicNot(operation) => operation.start(),
            Self::LogicSelectScalar(operation) => operation.start(),
            Self::MathScalar(operation) => operation.start(),
            Self::Layout(operation) => operation.start(),
            Self::PresentationComposition(operation) => operation.start(),
            Self::GraphicsPresentation(operation) => operation.start(),
            #[cfg(test)]
            Self::TestPresentationSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestLayoutSink(operation) => operation.start(),
            Self::RoboticsSource(operation) => operation.start(),
            Self::RoboticsDrive(operation) => operation.start(),
            Self::MusicSynth(operation) => operation.start(),
            Self::AudioRenderDemand(operation) => operation.start(),
            Self::AudioPlay(operation) => operation.start(),
            Self::MidiOutput(operation) => operation.start(),
            Self::MidiInput(operation) => operation.start(),
            Self::ExternalWebSocketListener(operation) => operation.start(),
            Self::GenerateText(operation) => operation.start(),
            Self::LocalModel(operation) => operation.start(),
            Self::VectorSearch(operation) => operation.start(),
            Self::HttpClient(operation) => operation.start(),
            Self::HttpServer(operation) => operation.start(),
            Self::ImageText(operation) => operation.start(),
            Self::ImageTextRecord(operation) => operation.start(),
            Self::TypedRecordFrame(operation) => operation.start(),
            Self::JsonEncode(operation) | Self::JsonDecode(operation) => operation.start(),
            Self::StructuredSelector(operation) => operation.start(),
            Self::StructuredLiteral(operation) => operation.start(),
            Self::StructuredPresentation(operation) => operation.start(),
            #[cfg(test)]
            Self::TestTextSource(operation) => operation.emit_or_complete(),
            #[cfg(test)]
            Self::TestMidiSource(operation) => operation.emit_or_complete(),
            #[cfg(test)]
            Self::TestRecurrenceSink(operation) => operation.start(),
            Self::TestPcmSource(operation) => operation.emit_or_complete(),
            #[cfg(test)]
            Self::TestJsonSource(operation) => operation.emit_or_complete(),
            #[cfg(test)]
            Self::TestJsonSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestStructuredSource(operation) => operation.start(),
            #[cfg(test)]
            Self::TestStructuredSink(operation) => operation.start(),
            #[cfg(any(test, feature = "local-model-proof"))]
            Self::TestLocalModelSource(operation) => operation.emit_or_complete(),
            #[cfg(any(test, feature = "local-model-proof"))]
            Self::TestLocalModelSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestKeyEventSource(operation) => operation.start(),
            #[cfg(test)]
            Self::TestChordSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestScalarSource(operation) => operation.start(),
            #[cfg(test)]
            Self::TestScalarLiteral(operation) => operation.start(),
            #[cfg(test)]
            Self::TestScalarSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestGateScript(operation) => operation.start(),
            #[cfg(test)]
            Self::TestLogicScript(operation) => operation.start(),
            #[cfg(test)]
            Self::TestLogicSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestSlowScalarSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestTimingSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestTimingSource(operation) => operation.start(),
            #[cfg(test)]
            Self::TestObserver(operation) => operation.start(),
            Self::Inactive => OperationAction::Complete,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (Self::KeyboardInput(_), _) => Self::fail(109),
            (Self::Tick(operation), input) => operation.resume(input),
            (Self::TextLiteral(operation), input) => operation.resume(input),
            (Self::TimeDebounce(operation), input) => operation.resume(input),
            (Self::TimeTimeout(operation), input) => operation.resume(input),
            (Self::TimeDelay(operation), input) => operation.resume(input),
            (Self::TimeThrottle(operation), input) => operation.resume(input),
            (Self::Recurrence(operation), input) => operation.resume(input),
            (Self::CalendarProposal(operation), input) => operation.resume(input),
            (Self::CalendarProvider(operation), input) => operation.resume(input),
            (Self::TextUpper(operation), input) => operation.resume(input),
            (Self::TextJoin(operation), input) => operation.resume(input),
            (Self::TextPresentation(operation), input) => operation.resume(input),
            (Self::TickPresentation(operation), input) => operation.resume(input),
            (Self::BoolPresentation(operation), input) => operation.resume(input),
            (Self::OrbiumSeed(_), _) => Self::fail(180),
            (Self::LeniaStep(operation), input) => operation.resume(input),
            (Self::ScalarFieldPresentation(operation), input) => operation.resume(input),
            (Self::StateCount(operation), input) => operation.resume(input),
            (Self::StateToggle(operation), input) => operation.resume(input),
            (Self::CountPresentation(operation), input) => operation.resume(input),
            (Self::StateLatestScalar(operation), input) => operation.resume(input),
            (Self::FlowTeeScalar(operation), input) => operation.resume(input),
            (Self::StateSelectScalar(operation), input) => operation.resume(input),
            (Self::FlowGateScalar(operation), input) => operation.resume(input),
            (Self::KeyEventTee(operation), input) => operation.resume(input),
            (Self::InputKeymap(operation), input) | (Self::InputChords(operation), input) => {
                operation.resume(input)
            }
            (Self::InstrumentMap(operation), input) => operation.resume(input),
            (Self::RhythmCompare(operation), input) => operation.resume(input),
            (Self::LogicCompareScalar(operation), input) => operation.resume(input),
            (Self::LogicNot(operation), input) => operation.resume(input),
            (Self::LogicSelectScalar(operation), input) => operation.resume(input),
            (Self::MathScalar(operation), input) => operation.resume(input),
            (Self::Layout(operation), input) => operation.resume(input),
            (Self::PresentationComposition(operation), input) => operation.resume(input),
            (Self::GraphicsPresentation(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestPresentationSink(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestLayoutSink(operation), input) => operation.resume(input),
            (Self::RoboticsSource(operation), input) => operation.resume(input),
            (Self::RoboticsDrive(operation), input) => operation.resume(input),
            (Self::MusicSynth(operation), input) => operation.resume(input),
            (Self::AudioRenderDemand(operation), input) => operation.resume(input),
            (Self::AudioPlay(operation), input) => operation.resume(input),
            (Self::MidiOutput(operation), input) => operation.resume(input),
            (Self::MidiInput(operation), _) => operation.resume(),
            (Self::ExternalWebSocketListener(operation), input) => operation.resume(input),
            (Self::GenerateText(operation), input) => operation.resume(input),
            (Self::LocalModel(operation), input) => operation.resume(input),
            (Self::VectorSearch(operation), input) => operation.resume(input),
            (Self::HttpClient(operation), input) => operation.resume(input),
            (Self::HttpServer(operation), input) => operation.resume(input),
            (Self::ImageText(operation), input) => operation.resume(input),
            (Self::ImageTextRecord(operation), input) => operation.resume(input),
            (Self::TypedRecordFrame(operation), input) => operation.resume(input),
            (Self::JsonEncode(operation), input) | (Self::JsonDecode(operation), input) => {
                operation.resume(input)
            }
            (Self::StructuredSelector(operation), input) => operation.resume(input),
            (Self::StructuredLiteral(_), _) => Self::fail(153),
            (Self::StructuredPresentation(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestTextSource(_), _) => Self::fail(6),
            #[cfg(test)]
            (Self::TestMidiSource(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestRecurrenceSink(operation), input) => operation.resume(input),
            (Self::TestPcmSource(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestJsonSource(_), _) => Self::fail(104),
            #[cfg(test)]
            (Self::TestJsonSink(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestStructuredSource(_), _) => Self::fail(152),
            #[cfg(test)]
            (Self::TestStructuredSink(operation), input) => operation.resume(input),
            #[cfg(any(test, feature = "local-model-proof"))]
            (Self::TestLocalModelSource(_), _) => Self::fail(141),
            #[cfg(any(test, feature = "local-model-proof"))]
            (Self::TestLocalModelSink(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestKeyEventSource(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestChordSink(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestObserver(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestScalarSink(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestScalarSource(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestScalarLiteral(_), _) => Self::fail(26),
            #[cfg(test)]
            (Self::TestGateScript(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestLogicScript(_), _) => Self::fail(23),
            #[cfg(test)]
            (Self::TestLogicSink(_), _) => Self::fail(24),
            #[cfg(test)]
            (Self::TestSlowScalarSink(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestTimingSink(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestTimingSource(operation), input) => operation.resume(input),
            (Self::Inactive, _) => Self::fail(4),
        }
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> OperationAction {
        match self {
            Self::LeniaStep(operation) => operation.resume_value(port, value, canonical),
            Self::LogicCompareScalar(operation) => operation.resume_value(port, value, canonical),
            Self::LogicNot(operation) => operation.resume_value(port, value, canonical),
            Self::LogicSelectScalar(operation) => operation.resume_value(port, value, canonical),
            Self::StateSelectScalar(operation) => operation.resume_value(port, value, canonical),
            Self::RoboticsDrive(operation) => operation.resume_value(port, value, canonical),
            #[cfg(test)]
            Self::TestLogicSink(operation) => operation.resume_value(port, value, canonical),
            #[cfg(test)]
            Self::TestChordSink(operation) => operation.resume_value(port, canonical),
            #[cfg(test)]
            Self::TestRecurrenceSink(operation) => operation.resume_value(port, canonical),
            #[cfg(test)]
            Self::TestStructuredSink(operation) => operation.resume_value(port, canonical),
            Self::InstrumentMap(operation) => operation.resume_value(port, canonical),
            _ => self.resume(OperationInput::Value { port, value }),
        }
    }

    fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: conduit_kernel::HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        match self {
            Self::KeyboardInput(operation) => {
                operation.resume_host_operation(request, outcome, canonical)
            }
            Self::MidiInput(operation) => {
                operation.resume_host_operation(request, outcome, canonical)
            }
            _ => self.resume(OperationInput::HostOperationCompleted { request, outcome }),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::KeyboardInput(operation) => operation.advance(),
            Self::Tick(operation) => operation.advance(),
            Self::TickPresentation(_) => OperationAction::Await,
            Self::BoolPresentation(_) => OperationAction::Await,
            Self::OrbiumSeed(operation) => operation.advance(),
            Self::LeniaStep(operation) => operation.advance(),
            Self::ScalarFieldPresentation(_) => OperationAction::Await,
            Self::TimeDebounce(operation) => operation.advance(),
            Self::TimeTimeout(operation) => operation.advance(),
            Self::TimeDelay(operation) => operation.advance(),
            Self::TimeThrottle(operation) => operation.advance(),
            Self::Recurrence(operation) => operation.advance(),
            Self::CalendarProposal(operation) => operation.advance(),
            Self::CalendarProvider(operation) => operation.advance(),
            Self::TextLiteral(operation) => operation.advance(),
            Self::TextUpper(_) => OperationAction::Await,
            Self::TextJoin(_) => OperationAction::Await,
            Self::TextPresentation(_) => OperationAction::Await,
            Self::JsonEncode(operation) | Self::JsonDecode(operation) => operation.advance(),
            Self::StructuredSelector(operation) => operation.advance(),
            Self::StructuredLiteral(operation) => operation.advance(),
            Self::StructuredPresentation(_) => OperationAction::Await,
            Self::StateCount(operation) => operation.advance(),
            Self::StateToggle(operation) => operation.advance(),
            Self::CountPresentation(_) => OperationAction::Await,
            Self::StateLatestScalar(operation) => operation.advance(),
            Self::FlowTeeScalar(operation) => operation.advance(),
            Self::StateSelectScalar(operation) => operation.advance(),
            Self::FlowGateScalar(operation) => operation.advance(),
            Self::KeyEventTee(operation) => operation.advance(),
            Self::InputKeymap(_) | Self::InputChords(_) => OperationAction::Await,
            Self::InstrumentMap(operation) => operation.advance(),
            Self::RhythmCompare(operation) => operation.advance(),
            Self::LogicCompareScalar(_) | Self::LogicNot(_) | Self::LogicSelectScalar(_) => {
                OperationAction::Complete
            }
            Self::MathScalar(_) => OperationAction::Complete,
            Self::Layout(operation) => operation.advance(),
            Self::PresentationComposition(operation) => operation.advance(),
            Self::GraphicsPresentation(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestPresentationSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestLayoutSink(_) => OperationAction::Await,
            Self::RoboticsSource(operation) => operation.advance(),
            Self::RoboticsDrive(operation) => operation.advance(),
            Self::MusicSynth(operation) => operation.advance(),
            Self::AudioRenderDemand(operation) => operation.advance(),
            Self::AudioPlay(_) => OperationAction::Await,
            Self::MidiOutput(_) => OperationAction::Await,
            Self::MidiInput(operation) => operation.advance(),
            Self::ExternalWebSocketListener(operation) => operation.advance(),
            Self::GenerateText(operation) => operation.advance(),
            Self::LocalModel(operation) => operation.advance(),
            Self::VectorSearch(operation) => operation.advance(),
            Self::HttpClient(operation) => operation.advance(),
            Self::HttpServer(operation) => operation.advance(),
            Self::ImageText(_) => OperationAction::Await,
            Self::ImageTextRecord(_) => OperationAction::Await,
            Self::TypedRecordFrame(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestTextSource(operation) => {
                operation.next += 1;
                operation.emit_or_complete()
            }
            #[cfg(test)]
            Self::TestMidiSource(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestRecurrenceSink(_) => OperationAction::Await,
            Self::TestPcmSource(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestKeyEventSource(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestChordSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestScalarSource(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestScalarLiteral(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestScalarSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestGateScript(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestLogicScript(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestJsonSource(operation) => operation.advance(),
            #[cfg(any(test, feature = "local-model-proof"))]
            Self::TestLocalModelSource(operation) => operation.advance(),
            #[cfg(any(test, feature = "local-model-proof"))]
            Self::TestLocalModelSink(_) => OperationAction::Complete,
            #[cfg(test)]
            Self::TestJsonSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestStructuredSource(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestStructuredSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestLogicSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestSlowScalarSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestTimingSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestTimingSource(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestObserver(operation) => operation.advance(),
            Self::Inactive => OperationAction::Complete,
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::KeyboardInput(operation) => operation.cancel(),
            Self::Tick(operation) => operation.cancel(),
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
            Self::ImageText(operation) => operation.cancel(),
            Self::ImageTextRecord(operation) => operation.cancel(),
            Self::TypedRecordFrame(operation) => operation.cancel(),
            Self::JsonEncode(operation) | Self::JsonDecode(operation) => operation.cancel(),
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
            Self::TestStructuredSource(_) | Self::TestStructuredSink(_) => {}
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

    fn retains_resumed_value(&self) -> bool {
        match self {
            Self::StateLatestScalar(operation) => operation.retains_resumed_value(),
            Self::RhythmCompare(operation) => operation.retains_resumed_value(),
            Self::LogicSelectScalar(operation) => operation.retains_resumed_value(),
            Self::TimeDebounce(operation) => operation.retains_resumed_value(),
            Self::TimeDelay(operation) => operation.retains_resumed_value(),
            Self::MusicSynth(operation) => operation.retains_resumed_value(),
            _ => false,
        }
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        match self {
            Self::StateLatestScalar(operation) => operation.take_released_value(),
            Self::RhythmCompare(operation) => operation.take_released_value(),
            Self::LogicCompareScalar(operation) => operation.take_released_value(),
            Self::LogicNot(operation) => operation.take_released_value(),
            Self::LogicSelectScalar(operation) => operation.take_released_value(),
            Self::TimeDebounce(operation) => operation.take_released_value(),
            Self::TimeTimeout(operation) => operation.take_released_value(),
            Self::TimeDelay(operation) => operation.take_released_value(),
            Self::TimeThrottle(operation) => operation.take_released_value(),
            Self::MusicSynth(operation) => operation.take_released_value(),
            Self::HttpServer(operation) => operation.take_released_value(),
            _ => None,
        }
    }

    fn accepts_input_while_host_operation_pending(&self) -> bool {
        matches!(
            self,
            Self::TimeDebounce(_)
                | Self::TimeTimeout(_)
                | Self::TimeDelay(_)
                | Self::TimeThrottle(_)
        )
    }

    fn take_host_operation_cancellation(&mut self) -> Option<RequestId> {
        match self {
            Self::TimeDebounce(operation) => operation.take_host_operation_cancellation(),
            Self::TimeTimeout(operation) => operation.take_host_operation_cancellation(),
            Self::TimeThrottle(operation) => operation.take_host_operation_cancellation(),
            _ => None,
        }
    }
}
