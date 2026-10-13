//! Isolated Step frames for portable conformance; excluded from product builds.
use super::*;

impl<const PORTS: usize> StepIo<PORTS> {
    /// Construct one isolated transactional Step frame for conformance tests.
    pub const fn test_frame(
        inputs: [Option<ValueRef>; PORTS],
        input_closed: [bool; PORTS],
        output_maximum_bytes: [Option<u32>; PORTS],
        host_completion: Option<(RequestId, HostCallOutcome)>,
        maximum_fuel: u16,
    ) -> Self {
        Self {
            inputs,
            input_closed,
            input_abnormal: [None; PORTS],
            output_maximum_bytes,
            consumed: [false; PORTS],
            retained_inputs: [false; PORTS],
            consumed_closed: [false; PORTS],
            outputs: [None; PORTS],
            canonical_output: None,
            prepared_output: None,
            discards: [None; PORTS],
            host_completion,
            consumed_host_completion: false,
            host_request: None,
            host_cancellation: None,
            maximum_fuel,
            fuel_consumed: 0,
            fault: None,
        }
    }

    /// Add an abnormal observation to an isolated conformance test frame.
    pub fn test_with_input_abnormal(mut self, port: PortId, terminal: CanonicalValue) -> Self {
        *self
            .input_abnormal
            .get_mut(usize::from(port.0))
            .expect("test input port") = Some(terminal);
        self
    }

    pub fn test_consumed(&self, port: PortId) -> bool {
        self.consumed
            .get(usize::from(port.0))
            .copied()
            .unwrap_or(false)
    }

    pub fn test_retained(&self, port: PortId) -> bool {
        self.retained_inputs
            .get(usize::from(port.0))
            .copied()
            .unwrap_or(false)
    }

    pub fn test_consumed_closed(&self, port: PortId) -> bool {
        self.consumed_closed
            .get(usize::from(port.0))
            .copied()
            .unwrap_or(false)
    }

    pub fn test_output(&self, port: PortId) -> Option<ValueRef> {
        self.outputs.get(usize::from(port.0)).copied().flatten()
    }

    pub fn test_canonical_output(&self) -> Option<&(PortId, CanonicalValue)> {
        self.canonical_output.as_ref()
    }

    pub const fn test_prepared_output(&self) -> Option<(PortId, u32)> {
        self.prepared_output
    }

    pub fn test_discards(&self) -> &[Option<ValueRef>; PORTS] {
        &self.discards
    }

    pub const fn test_host_completion_consumed(&self) -> bool {
        self.consumed_host_completion
    }

    pub const fn test_host_request(&self) -> Option<(RequestId, HostCallId, BoundedValueRef)> {
        self.host_request
    }

    pub const fn test_host_cancellation(&self) -> Option<RequestId> {
        self.host_cancellation
    }

    pub const fn test_fuel_consumed(&self) -> u16 {
        self.fuel_consumed
    }

    pub const fn test_fault(&self) -> Option<SchedulerError> {
        self.fault
    }
}
