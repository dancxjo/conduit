//! Pre-Play allocation-capacity accounting for installed operations.

use super::operation::InstalledOperation;

impl InstalledOperation {
    pub(super) fn allocation_capacity(&self) -> usize {
        match self {
            Self::KeyboardInput(_) => 0,
            Self::ButtonInput(operation) => operation.allocation_capacity(),
            Self::Tick(operation) => operation.allocation_capacity(),
            Self::PulseObserve(operation) => operation.allocation_capacity(),
            Self::TimeDebounce(operation) => operation.allocation_capacity(),
            Self::TimeTimeout(operation) => operation.allocation_capacity(),
            Self::TimeDelay(operation) => operation.allocation_capacity(),
            Self::TimeThrottle(operation) => operation.allocation_capacity(),
            Self::TimedButtonAttempt(operation) => operation.allocation_capacity(),
            Self::StateCount(operation) => operation.allocation_capacity(),
            Self::RoboticsSource(operation) => operation.allocation_capacity(),
            Self::MusicSynth(_) => 0,
            Self::AudioRenderDemand(operation) => operation.allocation_capacity(),
            Self::AudioPlay(_) => 0,
            #[cfg(test)]
            Self::TestTextSource(operation) => operation.values.capacity(),
            #[cfg(test)]
            Self::TestStructuredSource(operation) => {
                operation.values.capacity() + operation.waits.capacity()
            }
            Self::TestPcmSource(_) => 0,
            #[cfg(test)]
            Self::TestKeyEventSource(operation) => {
                operation.values.capacity() + operation.waits.capacity()
            }
            #[cfg(test)]
            Self::TestScalarSource(operation) => {
                operation.values.capacity() + operation.waits.capacity()
            }
            #[cfg(test)]
            Self::TestGateScript(operation) => {
                operation.items.capacity() + operation.waits.capacity()
            }
            #[cfg(test)]
            Self::TestSlowScalarSink(operation) => operation.waits.capacity(),
            #[cfg(test)]
            Self::TestTimingSource(operation) => {
                operation.values.capacity() + operation.waits.capacity()
            }
            _ => 0,
        }
    }
}
