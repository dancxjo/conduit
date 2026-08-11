use super::count_operations::{CountPresentationOperation, StateCountOperation};
use super::flow_gate_operation::FlowGateScalarOperation;
use super::flow_state_operations::{FlowTeeScalarOperation, StateLatestScalarOperation};
use super::generate_text::GenerateTextOperation;
use super::input_semantic_operations::{InputSemanticOperation, KeyEventTeeOperation};
use super::layout_operations::LayoutOperation;
use super::logic_operations::{
    LogicCompareScalarOperation, LogicNotOperation, LogicSelectScalarOperation,
};
use super::math_operations::MathScalarOperation;
use super::pacing_operations::{DelayOperation, ThrottleOperation};
use super::presentation_composition::PresentationCompositionOperation;
use super::robotics_effect::SimulatedDriveEffect;
use super::robotics_operations::{RoboticsDriveOperation, RoboticsSourceOperation};
use super::synth_operation::MusicSynthOperation;
use super::text_operations::{
    TextLiteralOperation, TextPresentationOperation, TextTransformOperation,
};
#[cfg(test)]
use super::tick_operations::TestObserverOperation;
use super::tick_operations::TickOperation;
use super::tick_presentation::TickPresentationOperation;
use super::timing_operations::{DebounceOperation, TimeoutOperation};
use super::toggle_operation::StateToggleOperation;
use conduit_core::PlannedGear;
use conduit_kernel::{
    Failure, FailureCode, Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

pub(super) struct OperationBudget {
    pub(super) value_items: u16,
    pub(super) value_bytes: u32,
    pub(super) host_requests: usize,
    pub(super) sign_items: u16,
    pub(super) maximum_value_bytes: u32,
}

pub(super) struct InstalledFactory {
    pub(super) implementation_id: &'static str,
    pub(super) budget: fn(&PlannedGear) -> Result<OperationBudget, String>,
    pub(super) prepare: fn(
        &PlannedGear,
        &mut conduit_kernel::HostedValueStore,
    ) -> Result<InstalledOperation, String>,
}

pub(super) enum InstalledOperation {
    Tick(TickOperation),
    TimeDebounce(DebounceOperation),
    TimeTimeout(TimeoutOperation),
    TimeDelay(DelayOperation),
    TimeThrottle(ThrottleOperation),
    TickPresentation(TickPresentationOperation),
    TextLiteral(TextLiteralOperation),
    TextUpper(TextTransformOperation),
    TextJoin(TextTransformOperation),
    TextPresentation(TextPresentationOperation),
    StateCount(StateCountOperation),
    StateToggle(StateToggleOperation),
    CountPresentation(CountPresentationOperation),
    StateLatestScalar(StateLatestScalarOperation),
    FlowTeeScalar(FlowTeeScalarOperation),
    FlowGateScalar(FlowGateScalarOperation),
    KeyEventTee(KeyEventTeeOperation),
    InputKeymap(InputSemanticOperation),
    InputChords(InputSemanticOperation),
    LogicCompareScalar(LogicCompareScalarOperation),
    LogicNot(LogicNotOperation),
    LogicSelectScalar(LogicSelectScalarOperation),
    MathScalar(MathScalarOperation),
    Layout(LayoutOperation),
    PresentationComposition(PresentationCompositionOperation),
    #[cfg(test)]
    TestPresentationSink(super::presentation_composition::PresentationSinkOperation),
    #[cfg(test)]
    TestLayoutSink(super::layout_operations::LayoutSinkOperation),
    RoboticsSource(RoboticsSourceOperation),
    RoboticsDrive(RoboticsDriveOperation),
    MusicSynth(MusicSynthOperation),
    ExternalWebSocketListener(super::external_websocket::ExternalWebSocketListenerOperation),
    GenerateText(GenerateTextOperation),
    #[cfg(test)]
    TestTextSource(super::test_text_source::TestTextSourceOperation),
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
}

impl Operation for InstalledOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Tick(operation) => operation.start(),
            Self::TimeDebounce(operation) => operation.start(),
            Self::TimeTimeout(operation) => operation.start(),
            Self::TimeDelay(operation) => operation.start(),
            Self::TimeThrottle(operation) => operation.start(),
            Self::TickPresentation(operation) => operation.start(),
            Self::TextLiteral(operation) => operation.start(),
            Self::TextUpper(operation) => operation.start(),
            Self::TextJoin(operation) => operation.start(),
            Self::TextPresentation(operation) => operation.start(),
            Self::StateCount(operation) => operation.start(),
            Self::StateToggle(operation) => operation.start(),
            Self::CountPresentation(operation) => operation.start(),
            Self::StateLatestScalar(operation) => operation.start(),
            Self::FlowTeeScalar(operation) => operation.start(),
            Self::FlowGateScalar(operation) => operation.start(),
            Self::KeyEventTee(operation) => operation.start(),
            Self::InputKeymap(operation) | Self::InputChords(operation) => operation.start(),
            Self::LogicCompareScalar(operation) => operation.start(),
            Self::LogicNot(operation) => operation.start(),
            Self::LogicSelectScalar(operation) => operation.start(),
            Self::MathScalar(operation) => operation.start(),
            Self::Layout(operation) => operation.start(),
            Self::PresentationComposition(operation) => operation.start(),
            #[cfg(test)]
            Self::TestPresentationSink(operation) => operation.start(),
            #[cfg(test)]
            Self::TestLayoutSink(operation) => operation.start(),
            Self::RoboticsSource(operation) => operation.start(),
            Self::RoboticsDrive(operation) => operation.start(),
            Self::MusicSynth(operation) => operation.start(),
            Self::ExternalWebSocketListener(operation) => operation.start(),
            Self::GenerateText(operation) => operation.start(),
            #[cfg(test)]
            Self::TestTextSource(operation) => operation.emit_or_complete(),
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
            (Self::Tick(operation), input) => operation.resume(input),
            (Self::TextLiteral(operation), input) => operation.resume(input),
            (Self::TimeDebounce(operation), input) => operation.resume(input),
            (Self::TimeTimeout(operation), input) => operation.resume(input),
            (Self::TimeDelay(operation), input) => operation.resume(input),
            (Self::TimeThrottle(operation), input) => operation.resume(input),
            (Self::TextUpper(operation), input) => operation.resume(input),
            (Self::TextJoin(operation), input) => operation.resume(input),
            (Self::TextPresentation(operation), input) => operation.resume(input),
            (Self::TickPresentation(operation), input) => operation.resume(input),
            (Self::StateCount(operation), input) => operation.resume(input),
            (Self::StateToggle(operation), input) => operation.resume(input),
            (Self::CountPresentation(operation), input) => operation.resume(input),
            (Self::StateLatestScalar(operation), input) => operation.resume(input),
            (Self::FlowTeeScalar(operation), input) => operation.resume(input),
            (Self::FlowGateScalar(operation), input) => operation.resume(input),
            (Self::KeyEventTee(operation), input) => operation.resume(input),
            (Self::InputKeymap(operation), input) | (Self::InputChords(operation), input) => {
                operation.resume(input)
            }
            (Self::LogicCompareScalar(operation), input) => operation.resume(input),
            (Self::LogicNot(operation), input) => operation.resume(input),
            (Self::LogicSelectScalar(operation), input) => operation.resume(input),
            (Self::MathScalar(operation), input) => operation.resume(input),
            (Self::Layout(operation), input) => operation.resume(input),
            (Self::PresentationComposition(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestPresentationSink(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestLayoutSink(operation), input) => operation.resume(input),
            (Self::RoboticsSource(operation), input) => operation.resume(input),
            (Self::RoboticsDrive(operation), input) => operation.resume(input),
            (Self::MusicSynth(operation), input) => operation.resume(input),
            (Self::ExternalWebSocketListener(operation), input) => operation.resume(input),
            (Self::GenerateText(operation), input) => operation.resume(input),
            #[cfg(test)]
            (Self::TestTextSource(_), _) => Self::fail(6),
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
            Self::LogicCompareScalar(operation) => operation.resume_value(port, value, canonical),
            Self::LogicNot(operation) => operation.resume_value(port, value, canonical),
            Self::LogicSelectScalar(operation) => operation.resume_value(port, value, canonical),
            Self::RoboticsDrive(operation) => operation.resume_value(port, value, canonical),
            #[cfg(test)]
            Self::TestLogicSink(operation) => operation.resume_value(port, value, canonical),
            #[cfg(test)]
            Self::TestChordSink(operation) => operation.resume_value(port, canonical),
            _ => self.resume(OperationInput::Value { port, value }),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Tick(operation) => operation.advance(),
            Self::TickPresentation(_) => OperationAction::Await,
            Self::TimeDebounce(operation) => operation.advance(),
            Self::TimeTimeout(operation) => operation.advance(),
            Self::TimeDelay(operation) => operation.advance(),
            Self::TimeThrottle(operation) => operation.advance(),
            Self::TextLiteral(operation) => operation.advance(),
            Self::TextUpper(_) => OperationAction::Await,
            Self::TextJoin(_) => OperationAction::Await,
            Self::TextPresentation(_) => OperationAction::Await,
            Self::StateCount(operation) => operation.advance(),
            Self::StateToggle(operation) => operation.advance(),
            Self::CountPresentation(_) => OperationAction::Await,
            Self::StateLatestScalar(operation) => operation.advance(),
            Self::FlowTeeScalar(operation) => operation.advance(),
            Self::FlowGateScalar(operation) => operation.advance(),
            Self::KeyEventTee(operation) => operation.advance(),
            Self::InputKeymap(_) | Self::InputChords(_) => OperationAction::Await,
            Self::LogicCompareScalar(_) | Self::LogicNot(_) | Self::LogicSelectScalar(_) => {
                OperationAction::Complete
            }
            Self::MathScalar(_) => OperationAction::Complete,
            Self::Layout(operation) => operation.advance(),
            Self::PresentationComposition(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestPresentationSink(_) => OperationAction::Await,
            #[cfg(test)]
            Self::TestLayoutSink(_) => OperationAction::Await,
            Self::RoboticsSource(operation) => operation.advance(),
            Self::RoboticsDrive(operation) => operation.advance(),
            Self::MusicSynth(operation) => operation.advance(),
            Self::ExternalWebSocketListener(operation) => operation.advance(),
            Self::GenerateText(operation) => operation.advance(),
            #[cfg(test)]
            Self::TestTextSource(operation) => {
                operation.next += 1;
                operation.emit_or_complete()
            }
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
            Self::Tick(operation) => operation.cancel(),
            Self::TickPresentation(operation) => operation.cancel(),
            Self::TimeDebounce(operation) => operation.cancel(),
            Self::TimeTimeout(operation) => operation.cancel(),
            Self::TimeDelay(operation) => operation.cancel(),
            Self::TimeThrottle(operation) => operation.cancel(),
            Self::TextLiteral(_) => {}
            Self::TextUpper(operation) => operation.cancel(),
            Self::TextJoin(operation) => operation.cancel(),
            Self::TextPresentation(operation) => operation.cancel(),
            Self::StateCount(_) => {}
            Self::StateToggle(_) => {}
            Self::CountPresentation(operation) => operation.cancel(),
            Self::StateLatestScalar(operation) => operation.cancel(),
            Self::FlowTeeScalar(operation) => operation.cancel(),
            Self::FlowGateScalar(operation) => operation.cancel(),
            Self::KeyEventTee(operation) => operation.cancel(),
            Self::InputKeymap(operation) | Self::InputChords(operation) => operation.cancel(),
            Self::LogicCompareScalar(operation) => operation.cancel(),
            Self::LogicNot(operation) => operation.cancel(),
            Self::LogicSelectScalar(operation) => operation.cancel(),
            Self::MathScalar(operation) => operation.cancel(),
            Self::Layout(operation) => operation.cancel(),
            Self::PresentationComposition(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestPresentationSink(_) => {}
            #[cfg(test)]
            Self::TestLayoutSink(_) => {}
            Self::RoboticsSource(operation) => operation.cancel(),
            Self::RoboticsDrive(operation) => operation.cancel(),
            Self::MusicSynth(operation) => operation.cancel(),
            Self::ExternalWebSocketListener(operation) => operation.cancel(),
            Self::GenerateText(operation) => operation.cancel(),
            #[cfg(test)]
            Self::TestTextSource(_) => {}
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
            Self::LogicCompareScalar(operation) => operation.take_released_value(),
            Self::LogicNot(operation) => operation.take_released_value(),
            Self::LogicSelectScalar(operation) => operation.take_released_value(),
            Self::TimeDebounce(operation) => operation.take_released_value(),
            Self::TimeTimeout(operation) => operation.take_released_value(),
            Self::TimeDelay(operation) => operation.take_released_value(),
            Self::TimeThrottle(operation) => operation.take_released_value(),
            Self::MusicSynth(operation) => operation.take_released_value(),
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

impl InstalledOperation {
    pub(super) fn simulated_drive_effect(&self) -> Option<SimulatedDriveEffect> {
        match self {
            Self::RoboticsDrive(operation) => operation.effect(),
            _ => None,
        }
    }
}
