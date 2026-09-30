//! Fixed-capacity deterministic scheduler over the port-aware kernel contract.

mod host_input_ownership;

use crate::{
    debug_observation::{
        DebugBreakpoint, DebugControlRefusal, DebugEventKind, DebugObservationRefusal,
        DebugObserverControl, DebugRuntimeControl, DebugRuntimeEvent, DebugSuspension,
    },
    BoundedValueRef, CordEndpoint, CordId, FixedHostCallBindings, FixedRoutes, HostCallBinding,
    HostCallId, HostCallOutcome, KernelEventKind, NodeId, PortId, ProtocolError, RemoteEndpointId,
    RequestId, RouteTarget, SignError, SignSink, StorageError, ValueRef, ValueStorage,
};
pub use conduit_assigned_plan::{AssignedConnectionTrack, AssignedPressurePolicy};

mod active_capacity;
mod debug_control;
mod derived_value;
mod retirement;
use active_capacity::validate_active_capacity;
use debug_control::DebugControlState;
pub use derived_value::CanonicalValue;
pub use retirement::RetiredExecution;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeSpec<const PORTS: usize> {
    /// Exact inbound cord for each input-port ordinal.
    pub input_cords: [Option<CordId>; PORTS],
    /// Plan-owned fuel granted to each invocation of this Back.
    ///
    /// The cooperative kernel charges every kernel-visible action against this
    /// grant. A Back must also charge private computation explicitly. Code
    /// which cannot be trusted to do that requires a Host confinement boundary
    /// (for example Wasm instruction fuel); this field does not claim native
    /// in-process preemption.
    pub maximum_step_fuel: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedTerminalTransduction {
    pub input: PortId,
    pub output: PortId,
    pub normal_close: AssignedNormalCloseTransduction,
    pub abnormal: AssignedAbnormalTransduction,
    pub cancellation: AssignedCancellationTransduction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedFiniteTerminalEmission {
    pub maximum_items: u16,
    pub maximum_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignedNormalCloseTransduction {
    NotAccepted,
    PropagateAfterDrain,
    Consume,
    FlushThenPropagate(AssignedFiniteTerminalEmission),
    PropagateWhenAllClose,
    FlushThenPropagateWhenAllClose(AssignedFiniteTerminalEmission),
    DomainSpecific { law: [u8; 32] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignedAbnormalTransduction {
    NotAccepted,
    PropagateAfterDrain,
    Recover,
    FinalizeThenPropagate(AssignedFiniteTerminalEmission),
    DomainSpecific { law: [u8; 32] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignedCancellationTransduction {
    NotCancellable,
    Request {
        input: PortId,
        disposition_kind: [u8; 32],
    },
    DomainSpecific {
        law: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActiveTerminalPhase {
    Normal {
        input: PortId,
        emitted: AssignedFiniteTerminalEmission,
    },
    Abnormal {
        input: PortId,
        terminal: CanonicalValue,
        emitted: AssignedFiniteTerminalEmission,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct CordSpec {
    pub cord: CordId,
    pub source: CordEndpoint,
    pub sink: CordEndpoint,
    pub slot_start: u16,
    pub item_capacity: u16,
    pub byte_capacity: u32,
    /// Largest single canonical payload admitted on this Cord. This is the
    /// semantic value envelope, independent of the Cord's aggregate queue
    /// storage budget.
    pub maximum_value_bytes: u32,
    pub pressure_policy: AssignedPressurePolicy,
    pub track: AssignedConnectionTrack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CordCapacity {
    pub slot_start: u16,
    pub item_capacity: u16,
    pub byte_capacity: u32,
    pub pressure_policy: AssignedPressurePolicy,
}

impl CordSpec {
    pub const fn new(
        cord: CordId,
        source: CordEndpoint,
        sink: CordEndpoint,
        capacity: CordCapacity,
    ) -> Self {
        Self {
            cord,
            source,
            sink,
            slot_start: capacity.slot_start,
            item_capacity: capacity.item_capacity,
            byte_capacity: capacity.byte_capacity,
            maximum_value_bytes: capacity.byte_capacity,
            pressure_policy: capacity.pressure_policy,
            track: AssignedConnectionTrack::Payload,
        }
    }

    /// Padding entry for fixed-capacity tables. Active-prefix validation keeps
    /// this sentinel outside the executable plan.
    pub const fn inactive() -> Self {
        Self {
            cord: CordId(u16::MAX),
            source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
            sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
            slot_start: u16::MAX,
            item_capacity: 0,
            byte_capacity: 0,
            maximum_value_bytes: 0,
            pressure_policy: AssignedPressurePolicy::PreserveOrder,
            track: AssignedConnectionTrack::Payload,
        }
    }

    pub const fn local(
        cord: CordId,
        source: (NodeId, PortId),
        sink: (NodeId, PortId),
        capacity: CordCapacity,
    ) -> Self {
        Self::new(
            cord,
            CordEndpoint::local(source.0, source.1),
            CordEndpoint::local(sink.0, sink.1),
            capacity,
        )
    }

    pub const fn with_track(mut self, track: AssignedConnectionTrack) -> Self {
        self.track = track;
        self
    }

    pub const fn with_maximum_value_bytes(mut self, maximum_value_bytes: u32) -> Self {
        self.maximum_value_bytes = maximum_value_bytes;
        self
    }

    pub const fn remote_egress(
        cord: CordId,
        source: (NodeId, PortId),
        endpoint: RemoteEndpointId,
        capacity: CordCapacity,
    ) -> Self {
        Self::new(
            cord,
            CordEndpoint::local(source.0, source.1),
            CordEndpoint::Remote(endpoint),
            capacity,
        )
    }

    pub const fn remote_ingress(
        cord: CordId,
        endpoint: RemoteEndpointId,
        sink: (NodeId, PortId),
        capacity: CordCapacity,
    ) -> Self {
        Self::new(
            cord,
            CordEndpoint::Remote(endpoint),
            CordEndpoint::local(sink.0, sink.1),
            capacity,
        )
    }

    pub const fn source_local(self) -> Option<(NodeId, PortId)> {
        match self.source {
            CordEndpoint::Local { node, port } => Some((node, port)),
            CordEndpoint::Remote(_) => None,
        }
    }

    pub const fn sink_local(self) -> Option<(NodeId, PortId)> {
        match self.sink {
            CordEndpoint::Local { node, port } => Some((node, port)),
            CordEndpoint::Remote(_) => None,
        }
    }

    pub const fn remote_endpoint(self) -> Option<RemoteEndpointId> {
        match (self.source, self.sink) {
            (CordEndpoint::Remote(endpoint), CordEndpoint::Local { .. })
            | (CordEndpoint::Local { .. }, CordEndpoint::Remote(endpoint)) => Some(endpoint),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteValueOffer {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub sequence: u64,
    pub value: ValueRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteIngressOutcome {
    Accepted { sequence: u64 },
    Full { sequence: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteTerminalDisposition {
    NormalClose,
    Abnormal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepOutcome {
    Progress,
    Await,
    Yield,
    Complete,
    /// Explicit semantic abnormal termination of this Gear's endpoint contract.
    ///
    /// This is deliberately distinct from [`StepOutcome::Fail`], which reports
    /// mechanism/Back trouble and does not manufacture semantic terminal truth.
    Abnormal {
        /// Exact output Fore port whose typed `!` contract is being fulfilled.
        port: PortId,
        terminal: CanonicalValue,
    },
    Fail(crate::Failure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostCallRequest {
    pub node: NodeId,
    pub request: RequestId,
    pub call: HostCallId,
    pub input: BoundedValueRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostCallCancellation {
    pub node: NodeId,
    pub request: RequestId,
    pub call: HostCallId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingHostCall {
    request: HostCallRequest,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
    dispatched: bool,
    cancellation_requested: bool,
    cancellation_dispatched: bool,
    completion: Option<HostCallOutcome>,
}

pub trait StepBack<const PORTS: usize> {
    /// Exact terminal contracts implemented by this prepared Back. Preparation
    /// must match these to the Plan-lowered contracts before play.
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut contracts = [None; PORTS];
        if let Some(contract) = self.terminal_transduction() {
            let input = usize::from(contract.input.0);
            if input < PORTS {
                contracts[input] = Some(contract);
            }
        }
        contracts
    }

    /// Convenience for existing single-input Backs. Multi-input Backs override
    /// `terminal_transductions`; the scheduler and Plan boundary are plural.
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        None
    }

    /// Finalize private state only after successful transactional I/O commit.
    fn step_committed(&mut self) {}
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome;
    /// Bytes for one output staged by [`StepIo::send_prepared`]. The storage
    /// belongs to this already-prepared Back and must remain unchanged until
    /// `step_committed`; the scheduler copies it into admitted value storage
    /// only after the Step has passed transactional validation.
    fn prepared_output(&self, _port: PortId) -> Option<&[u8]> {
        None
    }
    fn accepts_input_while_host_call_pending(&self) -> bool {
        false
    }
    fn retains_host_call_input(&self, _request: RequestId, _value: ValueRef) -> bool {
        false
    }
    fn cancel(&mut self) {}
}

/// Read-only canonical bytes for the exact inputs presented in one step.
///
/// This view cannot resolve arbitrary [`ValueRef`]s and does not escape the
/// scheduler-owned value store.
pub struct StepInputBytes<'a, const PORTS: usize> {
    inputs: [Option<&'a [u8]>; PORTS],
    host_output: Option<&'a [u8]>,
}

impl<const PORTS: usize> StepInputBytes<'_, PORTS> {
    pub fn input(&self, port: PortId) -> Option<&[u8]> {
        self.inputs.get(usize::from(port.0)).copied().flatten()
    }

    pub fn host_output(&self) -> Option<&[u8]> {
        self.host_output
    }
}

#[cfg(feature = "step-test-support")]
impl<'a, const PORTS: usize> StepInputBytes<'a, PORTS> {
    /// Construct the exact byte view presented to one Step in conformance tests.
    pub const fn test_frame(
        inputs: [Option<&'a [u8]>; PORTS],
        host_output: Option<&'a [u8]>,
    ) -> Self {
        Self {
            inputs,
            host_output,
        }
    }
}

impl StepInputBytes<'static, 1> {
    pub(crate) const fn single_source() -> Self {
        Self {
            inputs: [None],
            host_output: None,
        }
    }
}

pub struct StepIo<const PORTS: usize> {
    inputs: [Option<ValueRef>; PORTS],
    input_closed: [bool; PORTS],
    input_abnormal: [Option<CanonicalValue>; PORTS],
    output_maximum_bytes: [Option<u32>; PORTS],
    consumed: [bool; PORTS],
    retained_inputs: [bool; PORTS],
    consumed_closed: [bool; PORTS],
    outputs: [Option<ValueRef>; PORTS],
    canonical_output: Option<(PortId, CanonicalValue)>,
    prepared_output: Option<(PortId, u32)>,
    discards: [Option<ValueRef>; PORTS],
    host_completion: Option<(RequestId, HostCallOutcome)>,
    consumed_host_completion: bool,
    host_request: Option<(RequestId, HostCallId, BoundedValueRef)>,
    host_cancellation: Option<RequestId>,
    maximum_fuel: u16,
    fuel_consumed: u16,
    fault: Option<SchedulerError>,
}

#[derive(Clone, Copy)]
struct StagedStep<const PORTS: usize> {
    consumed: [bool; PORTS],
    retained_inputs: [bool; PORTS],
    consumed_closed: [bool; PORTS],
    outputs: [Option<ValueRef>; PORTS],
    discards: [Option<ValueRef>; PORTS],
    consumed_host_completion: bool,
    host_request: Option<(RequestId, HostCallId, BoundedValueRef)>,
    host_cancellation: Option<RequestId>,
}

impl<const PORTS: usize> StepIo<PORTS> {
    pub fn input(&self, port: PortId) -> Option<ValueRef> {
        self.inputs.get(usize::from(port.0)).copied().flatten()
    }

    pub fn input_closed(&self, port: PortId) -> bool {
        self.input_closed
            .get(usize::from(port.0))
            .copied()
            .unwrap_or(true)
    }

    /// Exact bounded abnormal terminal truth observed after ordinary values on
    /// this input have drained. This is not mechanism failure and is distinct
    /// from normal close.
    pub fn input_abnormal(&self, port: PortId) -> Option<CanonicalValue> {
        self.input_abnormal
            .get(usize::from(port.0))
            .copied()
            .flatten()
    }

    /// Consume one exact abnormal input terminal as part of this transactional
    /// Step. The Back remains responsible for the checked transduction promised
    /// by its Kind; the kernel does not turn the terminal into generic failure.
    pub fn consume_abnormal(&mut self, port: PortId) -> Result<CanonicalValue, SchedulerError> {
        self.consume_fuel(1)?;
        let index = usize::from(port.0);
        let terminal = self
            .input_abnormal
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::InvalidPortAccess)?;
        if self.inputs.get(index).copied().flatten().is_some()
            || self.consumed_closed.get(index).copied().unwrap_or(true)
        {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        self.consumed_closed[index] = true;
        Ok(terminal)
    }

    pub fn consume(&mut self, port: PortId) -> Result<ValueRef, SchedulerError> {
        self.consume_input(port, false)
    }

    pub fn take_input(&mut self, port: PortId) -> Result<ValueRef, SchedulerError> {
        self.consume_input(port, true)
    }

    /// Pin the presented input as one admitted Host Call's request without
    /// consuming it from its cord. A later step must consume or otherwise
    /// resolve the same input after the exact completion is observed.
    pub fn borrow_input_for_call(&mut self, port: PortId) -> Result<ValueRef, SchedulerError> {
        self.consume_fuel(1)?;
        let index = usize::from(port.0);
        let value = self
            .inputs
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::InvalidPortAccess)?;
        if self.consumed.get(index).copied().unwrap_or(true)
            || self.retained_inputs.get(index).copied().unwrap_or(true)
        {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        self.retained_inputs[index] = true;
        Ok(value)
    }

    pub fn consume_closed(&mut self, port: PortId) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        let index = usize::from(port.0);
        if !self.input_closed.get(index).copied().unwrap_or(false)
            || self.inputs.get(index).copied().flatten().is_some()
            || self.consumed_closed.get(index).copied().unwrap_or(true)
        {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        self.consumed_closed[index] = true;
        Ok(())
    }

    fn consume_input(
        &mut self,
        port: PortId,
        retain_for_call: bool,
    ) -> Result<ValueRef, SchedulerError> {
        self.consume_fuel(1)?;
        let index = usize::from(port.0);
        let value = self
            .inputs
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::InvalidPortAccess)?;
        if self.consumed.get(index).copied().unwrap_or(true) {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        self.consumed[index] = true;
        self.retained_inputs[index] = retain_for_call;
        Ok(value)
    }

    pub fn output_ready(&self, port: PortId) -> bool {
        self.output_maximum_bytes
            .get(usize::from(port.0))
            .copied()
            .flatten()
            .is_some()
    }

    pub fn send(&mut self, port: PortId, value: ValueRef) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        let index = usize::from(port.0);
        let maximum = self
            .output_maximum_bytes
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::OutputBlocked)?;
        if value.byte_len > maximum || self.outputs.get(index).is_none() {
            return self.fail(SchedulerError::OutputBlocked);
        }
        if self.outputs[index].is_some() {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        self.outputs[index] = Some(value);
        Ok(())
    }

    pub fn send_canonical(
        &mut self,
        port: PortId,
        value: CanonicalValue,
    ) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        let index = usize::from(port.0);
        let maximum = self
            .output_maximum_bytes
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::OutputBlocked)?;
        if value.as_slice().len() as u32 > maximum
            || self.outputs.get(index).is_none()
            || self.outputs[index].is_some()
            || self.canonical_output.is_some()
            || self.prepared_output.is_some()
        {
            return self.fail(SchedulerError::OutputBlocked);
        }
        self.canonical_output = Some((port, value));
        Ok(())
    }

    /// Stage one value held in allocation-prepared Back storage.
    ///
    /// The Back exposes the exact bytes through [`StepBack::prepared_output`].
    /// This records no pointer and grants no storage authority; it only binds
    /// the output port and byte length into the current Step transaction.
    pub fn send_prepared(&mut self, port: PortId, byte_len: u32) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        let index = usize::from(port.0);
        let maximum = self
            .output_maximum_bytes
            .get(index)
            .copied()
            .flatten()
            .ok_or(SchedulerError::OutputBlocked)?;
        if byte_len > maximum
            || self.outputs.get(index).is_none()
            || self.outputs[index].is_some()
            || self.canonical_output.is_some()
            || self.prepared_output.is_some()
        {
            return self.fail(SchedulerError::OutputBlocked);
        }
        self.prepared_output = Some((port, byte_len));
        Ok(())
    }

    pub fn host_completion(&self) -> Option<(RequestId, HostCallOutcome)> {
        self.host_completion
    }

    pub fn consume_host_completion(
        &mut self,
    ) -> Result<(RequestId, HostCallOutcome), SchedulerError> {
        self.consume_fuel(1)?;
        if self.consumed_host_completion {
            return self.fail(SchedulerError::InvalidHostCallAccess);
        }
        let completion = self
            .host_completion
            .ok_or(SchedulerError::InvalidHostCallAccess)?;
        self.consumed_host_completion = true;
        Ok(completion)
    }

    pub fn request_host_call(
        &mut self,
        request: RequestId,
        call: HostCallId,
        input: BoundedValueRef,
    ) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        if self.host_request.is_some() {
            return self.fail(SchedulerError::InvalidHostCallAccess);
        }
        self.host_request = Some((request, call, input));
        Ok(())
    }

    pub fn cancel_host_call(&mut self, request: RequestId) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        if self.host_cancellation.is_some() {
            return self.fail(SchedulerError::InvalidHostCallAccess);
        }
        self.host_cancellation = Some(request);
        Ok(())
    }

    pub fn discard(&mut self, value: ValueRef) -> Result<(), SchedulerError> {
        self.consume_fuel(1)?;
        if self
            .discards
            .iter()
            .flatten()
            .any(|discard| *discard == value)
        {
            return self.fail(SchedulerError::InvalidPortAccess);
        }
        let Some(slot) = self.discards.iter_mut().find(|discard| discard.is_none()) else {
            return self.fail(SchedulerError::InvalidPortAccess);
        };
        *slot = Some(value);
        Ok(())
    }

    /// Consume cooperative computation fuel owned by this Step.
    ///
    /// Kernel-visible actions call this automatically. A Back performing
    /// private iteration must call it at bounded intervals. Failure is sticky:
    /// once the grant is exceeded, the whole Step is rejected atomically.
    pub fn consume_fuel(&mut self, units: u16) -> Result<(), SchedulerError> {
        let consumed = self
            .fuel_consumed
            .checked_add(units)
            .ok_or(SchedulerError::StepFuelExceeded)?;
        if consumed > self.maximum_fuel {
            return self.fail(SchedulerError::StepFuelExceeded);
        }
        self.fuel_consumed = consumed;
        Ok(())
    }

    /// Fuel still available to cooperative private computation in this Step.
    pub const fn remaining_fuel(&self) -> u16 {
        self.maximum_fuel - self.fuel_consumed
    }

    /// Consume the remainder of the grant before returning [`StepOutcome::Yield`].
    pub fn exhaust_fuel(&mut self) {
        self.fuel_consumed = self.maximum_fuel;
    }

    fn fail<T>(&mut self, error: SchedulerError) -> Result<T, SchedulerError> {
        if self.fault.is_none() {
            self.fault = Some(error);
        }
        Err(error)
    }

    fn staged(&self) -> bool {
        self.consumed.iter().any(|value| *value)
            || self.outputs.iter().any(Option::is_some)
            || self.canonical_output.is_some()
            || self.prepared_output.is_some()
            || self.discards.iter().any(Option::is_some)
            || self.consumed_host_completion
            || self.host_request.is_some()
            || self.host_cancellation.is_some()
            || self.consumed_closed.iter().any(|value| *value)
    }

    fn staged_step(&self) -> StagedStep<PORTS> {
        StagedStep {
            consumed: self.consumed,
            retained_inputs: self.retained_inputs,
            consumed_closed: self.consumed_closed,
            outputs: self.outputs,
            discards: self.discards,
            consumed_host_completion: self.consumed_host_completion,
            host_request: self.host_request,
            host_cancellation: self.host_cancellation,
        }
    }
}

#[cfg(feature = "step-test-support")]
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

impl StepIo<1> {
    pub(crate) fn single_source(
        maximum_output_bytes: u32,
        maximum_fuel: u16,
        host_completion: Option<(RequestId, HostCallOutcome)>,
    ) -> Self {
        Self {
            inputs: [None],
            input_closed: [false],
            input_abnormal: [None],
            output_maximum_bytes: [Some(maximum_output_bytes)],
            consumed: [false],
            retained_inputs: [false],
            consumed_closed: [false],
            outputs: [None],
            canonical_output: None,
            prepared_output: None,
            discards: [None],
            host_completion,
            consumed_host_completion: false,
            host_request: None,
            host_cancellation: None,
            maximum_fuel,
            fuel_consumed: 0,
            fault: None,
        }
    }

    pub(crate) fn single_source_start_request(
        &self,
    ) -> Option<(RequestId, HostCallId, BoundedValueRef)> {
        if self.fault.is_none()
            && self.host_request.is_some()
            && !self.consumed_host_completion
            && self.outputs[0].is_none()
            && self.canonical_output.is_none()
            && self.prepared_output.is_none()
            && self.discards[0].is_none()
            && self.host_cancellation.is_none()
        {
            self.host_request
        } else {
            None
        }
    }

    pub(crate) fn single_source_completion_output(&self) -> Option<ValueRef> {
        if self.fault.is_none()
            && self.consumed_host_completion
            && self.host_request.is_none()
            && self.canonical_output.is_none()
            && self.prepared_output.is_none()
            && self.discards[0].is_none()
            && self.host_cancellation.is_none()
        {
            self.outputs[0]
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerStatus {
    Progress {
        node: NodeId,
    },
    Idle,
    /// Every gear has settled and every Cord has drained.
    ///
    /// This is structural scheduler truth, not a claim that the form's
    /// meaning is complete. The play lifecycle must separately classify a
    /// drained scheduler as quiescent or semantically completed.
    Drained,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    InvalidPlan,
    InvalidActiveCapacity,
    InvalidPortAccess,
    InvalidHostCallAccess,
    HostCallCancellationRejected,
    HostCallCancellationDuplicate,
    HostCallCancellationUndispatched,
    OutputBlocked,
    QueueCapacityExceeded,
    QueueByteCapacityExceeded,
    SemanticValueBoundExceeded,
    StepFuelExceeded,
    FalseProgress,
    DecisionLimitExceeded,
    BackFailed(crate::Failure),
    SemanticAbnormal {
        node: NodeId,
        port: PortId,
        terminal: CanonicalValue,
    },
    HostCallCapacityExceeded,
    HostCallRequestDuplicate,
    HostCallCompletionRejected,
    HostCallOutputExceeded,
    InvalidRemoteCordAccess,
    RemoteSequenceRejected,
    RemoteDeliveryRejected,
    ValueOwnershipViolation,
    Cancelled,
    DebugSuspended,
    Storage(StorageError),
    Sign(SignError),
    Routing(ProtocolError),
}

impl From<StorageError> for SchedulerError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<SignError> for SchedulerError {
    fn from(value: SignError) -> Self {
        Self::Sign(value)
    }
}

impl From<ProtocolError> for SchedulerError {
    fn from(value: ProtocolError) -> Self {
        Self::Routing(value)
    }
}

#[derive(Clone, Copy, Debug)]
struct CordState {
    head: u16,
    len: u16,
    queued_bytes: u32,
    producer_closed: bool,
    producer_abnormal: bool,
    abnormal_terminal: Option<CanonicalValue>,
    next_remote_sequence: u64,
    offered_remote_sequence: Option<u64>,
    remote_accepted: bool,
    quiescence_armed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnresolvedAbnormal {
    port: PortId,
    terminal: CanonicalValue,
}

impl CordState {
    const EMPTY: Self = Self {
        head: 0,
        len: 0,
        queued_bytes: 0,
        producer_closed: false,
        producer_abnormal: false,
        abnormal_terminal: None,
        next_remote_sequence: 0,
        offered_remote_sequence: None,
        remote_accepted: false,
        quiescence_armed: false,
    };
}

pub struct FixedScheduler<
    D,
    S,
    E,
    const NODES: usize,
    const CORDS: usize,
    const PORTS: usize,
    const QUEUE_SLOTS: usize,
    const ROUTE_SLOTS: usize,
    const ROUTE_TARGETS: usize,
    const HOST_BINDING_SLOTS: usize = 0,
    const PENDING_REQUESTS: usize = 0,
> where
    D: StepBack<PORTS>,
    S: ValueStorage,
    E: SignSink,
{
    node_specs: [NodeSpec<PORTS>; NODES],
    terminal_transductions: [[Option<AssignedTerminalTransduction>; PORTS]; NODES],
    terminal_inputs_consumed: [[bool; PORTS]; NODES],
    terminal_phases: [Option<ActiveTerminalPhase>; NODES],
    terminal_cancellation_pending: [bool; NODES],
    unresolved_abnormal: [Option<UnresolvedAbnormal>; NODES],
    projected_recovery_source: [Option<NodeId>; NODES],
    recovery_for_cord: [Option<NodeId>; CORDS],
    cord_specs: [CordSpec; CORDS],
    active_nodes: usize,
    active_cords: usize,
    routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
    host_bindings: Option<FixedHostCallBindings<HOST_BINDING_SLOTS>>,
    pending_host_calls: [Option<PendingHostCall>; PENDING_REQUESTS],
    drivers: [D; NODES],
    values: S,
    signs: E,
    cords: [CordState; CORDS],
    queue_slots: [Option<ValueRef>; QUEUE_SLOTS],
    ready: [bool; NODES],
    completed: [bool; NODES],
    cursor: usize,
    host_request_cursor: usize,
    decisions: u32,
    last_host_request: [Option<RequestId>; NODES],
    cancelled: bool,
    debug_control: DebugControlState,
}

impl<
        D,
        S,
        E,
        const NODES: usize,
        const CORDS: usize,
        const PORTS: usize,
        const QUEUE_SLOTS: usize,
        const ROUTE_SLOTS: usize,
        const ROUTE_TARGETS: usize,
        const HOST_BINDING_SLOTS: usize,
        const PENDING_REQUESTS: usize,
    >
    FixedScheduler<
        D,
        S,
        E,
        NODES,
        CORDS,
        PORTS,
        QUEUE_SLOTS,
        ROUTE_SLOTS,
        ROUTE_TARGETS,
        HOST_BINDING_SLOTS,
        PENDING_REQUESTS,
    >
where
    D: StepBack<PORTS>,
    S: ValueStorage,
    E: SignSink,
{
    pub fn new(
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<Self, SchedulerError> {
        if NODES == 0 || CORDS == 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        Self::new_with_active_counts(
            NODES, CORDS, node_specs, cord_specs, routes, drivers, values, signs,
        )
    }

    /// Installs an admitted topology prefix inside the compile-time capacity.
    /// Slots outside the active counts remain inert for the scheduler lifetime.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_active_counts(
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<Self, SchedulerError> {
        validate_active_capacity(active_nodes, NODES, active_cords, CORDS)?;
        if PORTS == 0 || QUEUE_SLOTS == 0 || !routes.is_sealed() {
            return Err(SchedulerError::InvalidPlan);
        }
        routes.validate_active_prefix(active_nodes, active_cords)?;
        validate_plan::<NODES, CORDS, PORTS, QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS>(
            active_nodes,
            active_cords,
            &node_specs,
            &cord_specs,
            &routes,
        )?;
        Ok(Self {
            node_specs,
            terminal_transductions: [[None; PORTS]; NODES],
            terminal_inputs_consumed: [[false; PORTS]; NODES],
            terminal_phases: [None; NODES],
            terminal_cancellation_pending: [false; NODES],
            unresolved_abnormal: [None; NODES],
            projected_recovery_source: [None; NODES],
            recovery_for_cord: [None; CORDS],
            cord_specs,
            active_nodes,
            active_cords,
            routes,
            host_bindings: None,
            pending_host_calls: [None; PENDING_REQUESTS],
            drivers,
            values,
            signs,
            cords: [CordState::EMPTY; CORDS],
            queue_slots: [None; QUEUE_SLOTS],
            ready: core::array::from_fn(|node| node < active_nodes),
            completed: [false; NODES],
            cursor: 0,
            host_request_cursor: 0,
            decisions: 0,
            last_host_request: [None; NODES],
            cancelled: false,
            debug_control: DebugControlState::new(),
        })
    }

    pub fn new_with_host_calls(
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostCallBindings<HOST_BINDING_SLOTS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<Self, SchedulerError> {
        if NODES == 0 || CORDS == 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        Self::new_with_active_counts_and_host_calls(
            NODES,
            CORDS,
            node_specs,
            cord_specs,
            routes,
            host_bindings,
            drivers,
            values,
            signs,
        )
    }

    /// Installs an admitted topology prefix and its sealed Host Call table
    /// inside the compile-time capacities.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_active_counts_and_host_calls(
        active_nodes: usize,
        active_cords: usize,
        node_specs: [NodeSpec<PORTS>; NODES],
        cord_specs: [CordSpec; CORDS],
        routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
        host_bindings: FixedHostCallBindings<HOST_BINDING_SLOTS>,
        drivers: [D; NODES],
        values: S,
        signs: E,
    ) -> Result<Self, SchedulerError> {
        validate_active_capacity(active_nodes, NODES, active_cords, CORDS)?;
        if PENDING_REQUESTS == 0 || !host_bindings.is_sealed() {
            return Err(SchedulerError::InvalidPlan);
        }
        host_bindings.validate_active_nodes(active_nodes)?;
        let mut scheduler = Self::new_with_active_counts(
            active_nodes,
            active_cords,
            node_specs,
            cord_specs,
            routes,
            drivers,
            values,
            signs,
        )?;
        scheduler.host_bindings = Some(host_bindings);
        Ok(scheduler)
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        if self.cancelled {
            return Ok(SchedulerStatus::Cancelled);
        }
        if self.debug_control.suspension().is_some() {
            return Err(SchedulerError::DebugSuspended);
        }
        let Some(node) = self.next_ready() else {
            if let Some(node) = self.emit_quiescence_transitions()? {
                return Ok(SchedulerStatus::Progress { node });
            }
            return if self.completed[..self.active_nodes]
                .iter()
                .all(|value| *value)
                && self.cords[..self.active_cords]
                    .iter()
                    .all(|cord| cord.len == 0)
            {
                self.drained_status()
            } else {
                Ok(SchedulerStatus::Idle)
            };
        };
        if self.debug_control.suspend_before(NodeId(as_u16(node)?)) {
            return Err(SchedulerError::DebugSuspended);
        }
        self.decisions = self
            .decisions
            .checked_add(1)
            .ok_or(SchedulerError::DecisionLimitExceeded)?;
        self.signs.record(
            NodeId(as_u16(node)?),
            None,
            None,
            KernelEventKind::StepFuelGranted,
        )?;
        self.signs.observe_debug(DebugRuntimeEvent {
            node: NodeId(as_u16(node)?),
            port: None,
            cord: None,
            kind: DebugEventKind::GearStarted,
            type_identity: None,
            value: None,
            fault_code: None,
        });

        let mut io = self.context(node)?;
        let mut current_input_bytes = [None; PORTS];
        for (port, value) in io.inputs.iter().copied().enumerate() {
            if let Some(value) = value {
                current_input_bytes[port] = Some(self.values.get(value)?);
            }
        }
        let input_bytes = StepInputBytes {
            inputs: current_input_bytes,
            host_output: io
                .host_completion
                .and_then(|(_, outcome)| outcome.output)
                .map(|output| self.values.get(output.value))
                .transpose()?,
        };
        let outcome = self.drivers[node].step(&mut io, &input_bytes);
        if let Some(fault) = io.fault {
            if fault == SchedulerError::StepFuelExceeded {
                self.signs.record(
                    NodeId(as_u16(node)?),
                    None,
                    None,
                    KernelEventKind::StepFuelExceeded,
                )?;
            }
            return Err(fault);
        }
        self.apply_step(node, outcome, io)?;
        if self.completed[..self.active_nodes]
            .iter()
            .all(|value| *value)
            && self.cords[..self.active_cords]
                .iter()
                .all(|cord| cord.len == 0)
        {
            self.drained_status()
        } else {
            Ok(SchedulerStatus::Progress {
                node: NodeId(as_u16(node)?),
            })
        }
    }

    /// Bind the exact Plan-lowered terminal contracts before play. A prepared
    /// Back may not claim a different mapping or behavior.
    pub fn bind_terminal_transductions(
        &mut self,
        contracts: [[Option<AssignedTerminalTransduction>; PORTS]; NODES],
    ) -> Result<(), SchedulerError> {
        for (node, contract) in contracts.iter().copied().enumerate() {
            if node < self.active_nodes {
                if self.drivers[node].terminal_transductions() != contract {
                    return Err(SchedulerError::InvalidPlan);
                }
            } else if contract.iter().any(Option::is_some) {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        self.terminal_transductions = contracts;
        self.bind_projected_recoveries()?;
        Ok(())
    }

    fn bind_projected_recoveries(&mut self) -> Result<(), SchedulerError> {
        self.recovery_for_cord = [None; CORDS];
        for cord in 0..self.active_cords {
            let spec = self.cord_specs[cord];
            if spec.track != AssignedConnectionTrack::AbnormalTerminal {
                continue;
            }
            let CordEndpoint::Local {
                node: sink,
                port: sink_port,
            } = spec.sink
            else {
                continue;
            };
            let Some(contract) = self.terminal_transductions[usize::from(sink.0)]
                .get(usize::from(sink_port.0))
                .copied()
                .flatten()
            else {
                continue;
            };
            if contract.input == sink_port
                && matches!(contract.abnormal, AssignedAbnormalTransduction::Recover)
            {
                if self.recovery_for_cord[..cord]
                    .iter()
                    .enumerate()
                    .any(|(other, recovery)| {
                        *recovery == Some(sink)
                            || (*recovery).is_some() && self.cord_specs[other].source == spec.source
                    })
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                self.recovery_for_cord[cord] = Some(sink);
            }
        }
        Ok(())
    }

    fn drained_status(&self) -> Result<SchedulerStatus, SchedulerError> {
        if let Some((node, unresolved)) = self.unresolved_abnormal[..self.active_nodes]
            .iter()
            .copied()
            .enumerate()
            .find_map(|(node, value)| value.map(|value| (node, value)))
        {
            return Err(SchedulerError::SemanticAbnormal {
                node: NodeId(as_u16(node)?),
                port: unresolved.port,
                terminal: unresolved.terminal,
            });
        }
        Ok(SchedulerStatus::Drained)
    }

    pub fn run(&mut self, maximum_decisions: u32) -> Result<(), SchedulerError> {
        if maximum_decisions == 0 {
            return Err(SchedulerError::DecisionLimitExceeded);
        }
        for _ in 0..maximum_decisions {
            match self.step()? {
                SchedulerStatus::Drained => return Ok(()),
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => return Err(SchedulerError::FalseProgress),
                SchedulerStatus::Cancelled => return Err(SchedulerError::Cancelled),
            }
        }
        Err(SchedulerError::DecisionLimitExceeded)
    }

    pub fn decisions(&self) -> u32 {
        self.decisions
    }

    pub fn drivers(&self) -> &[D; NODES] {
        &self.drivers
    }

    pub fn values(&self) -> &S {
        &self.values
    }

    pub fn signs(&self) -> &E {
        &self.signs
    }

    /// Attaches a debugger projection without exposing mutable mandatory Signs.
    pub fn attach_debug_observer(
        &mut self,
        history: E::History,
    ) -> Result<(), DebugObservationRefusal>
    where
        E: DebugObserverControl,
    {
        self.signs.attach_debug_observer(history)
    }

    /// Detaches the debugger projection while authoritative execution continues.
    pub fn detach_debug_observer(&mut self) -> Result<E::History, DebugObservationRefusal>
    where
        E: DebugObserverControl,
    {
        self.signs.detach_debug_observer()
    }

    /// Arms one exact unconditional breakpoint for this scheduler-owned Play.
    pub fn request_debug_breakpoint(
        &mut self,
        breakpoint: DebugBreakpoint,
    ) -> Result<(), DebugControlRefusal>
    where
        E: DebugRuntimeControl,
    {
        let node = self.signs.validate_breakpoint(breakpoint)?;
        if usize::from(node.0) >= self.active_nodes || self.completed[usize::from(node.0)] {
            return Err(DebugControlRefusal::UnknownSubject);
        }
        self.debug_control.arm(breakpoint, node)
    }

    /// Resumes the exact suspension. The armed v1 breakpoint is one-shot.
    pub fn resume_debug_suspension(
        &mut self,
        suspension: DebugSuspension,
    ) -> Result<(), DebugControlRefusal> {
        let node = self.debug_control.resume(suspension)?;
        self.cursor = usize::from(node.0);
        Ok(())
    }

    pub const fn debug_suspension(&self) -> Option<DebugSuspension> {
        self.debug_control.suspension()
    }

    pub fn cord_usage(&self, cord: CordId) -> Result<(u16, u32), SchedulerError> {
        if usize::from(cord.0) >= self.active_cords {
            return Err(SchedulerError::InvalidPlan);
        }
        let state = self
            .cords
            .get(usize::from(cord.0))
            .ok_or(SchedulerError::InvalidPlan)?;
        Ok((state.len, state.queued_bytes))
    }

    /// Offers the head of a remote egress cord without transferring ownership.
    /// Repeated calls return the same sequence and value until exact delivery
    /// is acknowledged.
    pub fn remote_egress_offer(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<RemoteValueOffer>, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        let cord_index = usize::from(cord.0);
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (source_node, source_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Local { node, port }, CordEndpoint::Remote(candidate))
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        let Some(value) = self.peek(cord_index)? else {
            return Ok(None);
        };
        let existing = self.cords[cord_index].offered_remote_sequence;
        let sequence = existing.unwrap_or(self.cords[cord_index].next_remote_sequence);
        if existing.is_none() {
            self.ensure_sign_capacity(1)?;
            self.ensure_remote_sign_capacity(1)?;
            self.cords[cord_index].offered_remote_sequence = Some(sequence);
            self.signs.record_remote(
                source_node,
                source_port,
                KernelEventKind::RemoteValueOffered,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord,
                    direction: crate::RemoteCordDirection::Egress,
                    sequence,
                },
            )?;
        }
        Ok(Some(RemoteValueOffer {
            endpoint,
            cord,
            sequence,
            value,
        }))
    }

    pub fn remote_egress_accept(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        let cord_index = usize::from(cord.0);
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (source_node, source_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Local { node, port }, CordEndpoint::Remote(candidate))
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        let state = self
            .cords
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if state.offered_remote_sequence != Some(sequence) {
            return Err(SchedulerError::RemoteSequenceRejected);
        }
        if state.remote_accepted {
            return Ok(());
        }
        self.ensure_sign_capacity(1)?;
        self.ensure_remote_sign_capacity(1)?;
        self.cords[cord_index].remote_accepted = true;
        self.signs.record_remote(
            source_node,
            source_port,
            KernelEventKind::RemoteValueAccepted,
            crate::RemoteLifecycleIdentity {
                endpoint,
                cord,
                direction: crate::RemoteCordDirection::Egress,
                sequence,
            },
        )?;
        Ok(())
    }

    /// Releases the source value only after the line reports the exact
    /// sequence as delivered by the peer kernel.
    pub fn remote_egress_delivered(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::RemoteDeliveryRejected);
        }
        let cord_index = usize::from(cord.0);
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (source_node, source_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Local { node, port }, CordEndpoint::Remote(candidate))
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        let state = self
            .cords
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if state.offered_remote_sequence != Some(sequence) || !state.remote_accepted {
            return Err(SchedulerError::RemoteDeliveryRejected);
        }
        let next_sequence = state
            .next_remote_sequence
            .checked_add(1)
            .ok_or(SchedulerError::RemoteSequenceRejected)?;
        self.ensure_sign_capacity(1)?;
        self.ensure_remote_sign_capacity(1)?;
        let value = self.pop(cord_index)?;
        self.values.release(value)?;
        let state = &mut self.cords[cord_index];
        state.next_remote_sequence = next_sequence;
        state.offered_remote_sequence = None;
        state.remote_accepted = false;
        self.ready[usize::from(source_node.0)] = true;
        self.signs.record_remote(
            source_node,
            source_port,
            KernelEventKind::RemoteValueDelivered,
            crate::RemoteLifecycleIdentity {
                endpoint,
                cord,
                direction: crate::RemoteCordDirection::Egress,
                sequence,
            },
        )?;
        Ok(())
    }

    /// Admits bytes through a remote ingress cord into the kernel-owned value
    /// store and queue. `Full` performs no allocation or sequence advance, so
    /// the line must retry the same sequence.
    pub fn admit_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        let cord_index = usize::from(cord.0);
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (sink_node, sink_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Remote(candidate), CordEndpoint::Local { node, port })
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        let state = self
            .cords
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if state.producer_closed || state.next_remote_sequence != sequence {
            return Err(SchedulerError::RemoteSequenceRejected);
        }
        let byte_len =
            u32::try_from(bytes.len()).map_err(|_| SchedulerError::QueueByteCapacityExceeded)?;
        let available = self.admission_maximum(spec, state)?;
        if byte_len > available {
            return Ok(RemoteIngressOutcome::Full { sequence });
        }
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(SchedulerError::RemoteSequenceRejected)?;
        self.ensure_sign_capacity(1)?;
        self.ensure_remote_sign_capacity(1)?;
        let value = self.values.store(bytes)?;
        match self.push(cord_index, value) {
            Ok(Some(superseded)) => self.values.release(superseded)?,
            Ok(None) => {}
            Err(error) => {
                self.values.release(value)?;
                return Err(error);
            }
        }
        self.cords[cord_index].next_remote_sequence = next_sequence;
        self.ready[usize::from(sink_node.0)] = true;
        self.signs.record_remote(
            sink_node,
            sink_port,
            KernelEventKind::RemoteInputAdmitted,
            crate::RemoteLifecycleIdentity {
                endpoint,
                cord,
                direction: crate::RemoteCordDirection::Ingress,
                sequence,
            },
        )?;
        Ok(RemoteIngressOutcome::Accepted { sequence })
    }

    /// Atomically admits one external payload to every exact ingress branch.
    /// If any branch is pressured, no branch observes the value and the same
    /// sequence remains retryable across the whole fan-out.
    pub fn admit_remote_input_fanout(
        &mut self,
        targets: &[(RemoteEndpointId, CordId)],
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        if targets.is_empty() || targets.len() > usize::from(u16::MAX) {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let byte_len =
            u32::try_from(bytes.len()).map_err(|_| SchedulerError::QueueByteCapacityExceeded)?;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(SchedulerError::RemoteSequenceRejected)?;
        for (index, (endpoint, cord)) in targets.iter().copied().enumerate() {
            if targets[..index].contains(&(endpoint, cord)) {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let cord_index = usize::from(cord.0);
            if cord_index >= self.active_cords {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let spec = self.cord_specs[cord_index];
            if !matches!(spec.source, CordEndpoint::Remote(candidate) if candidate == endpoint)
                || !matches!(spec.sink, CordEndpoint::Local { .. })
            {
                return Err(SchedulerError::InvalidRemoteCordAccess);
            }
            let state = &self.cords[cord_index];
            if state.producer_closed || state.next_remote_sequence != sequence {
                return Err(SchedulerError::RemoteSequenceRejected);
            }
            if byte_len > self.admission_maximum(spec, state)? {
                return Ok(RemoteIngressOutcome::Full { sequence });
            }
        }
        self.ensure_sign_capacity(targets.len())?;
        self.ensure_remote_sign_capacity(targets.len())?;
        let value = self.values.store(bytes)?;
        for references in 1..targets.len() {
            if let Err(error) = self.values.retain(value) {
                for _ in 0..references {
                    self.values.release(value)?;
                }
                return Err(error.into());
            }
        }
        for (endpoint, cord) in targets.iter().copied() {
            let cord_index = usize::from(cord.0);
            let spec = self.cord_specs[cord_index];
            let (sink_node, sink_port) = spec.sink_local().ok_or(SchedulerError::InvalidPlan)?;
            if let Some(superseded) = self.push(cord_index, value)? {
                self.values.release(superseded)?;
            }
            self.cords[cord_index].next_remote_sequence = next_sequence;
            self.ready[usize::from(sink_node.0)] = true;
            self.signs.record_remote(
                sink_node,
                sink_port,
                KernelEventKind::RemoteInputAdmitted,
                crate::RemoteLifecycleIdentity {
                    endpoint,
                    cord,
                    direction: crate::RemoteCordDirection::Ingress,
                    sequence,
                },
            )?;
        }
        Ok(RemoteIngressOutcome::Accepted { sequence })
    }

    pub fn close_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<(), SchedulerError> {
        self.close_remote_input_with_disposition(
            endpoint,
            cord,
            RemoteTerminalDisposition::NormalClose,
        )
    }

    pub fn close_remote_input_with_disposition(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        disposition: RemoteTerminalDisposition,
    ) -> Result<(), SchedulerError> {
        if disposition == RemoteTerminalDisposition::Abnormal {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        self.close_remote_input_terminal(endpoint, cord, None)
    }

    /// Admit exact typed semantic abnormal truth from a remote source. The
    /// session layer validates the bytes against the connection's declared F
    /// before constructing this bounded value.
    pub fn close_remote_input_abnormal(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        terminal: CanonicalValue,
    ) -> Result<(), SchedulerError> {
        self.close_remote_input_terminal(endpoint, cord, Some(terminal))
    }

    fn close_remote_input_terminal(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        abnormal_terminal: Option<CanonicalValue>,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        let cord_index = usize::from(cord.0);
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        let (sink_node, sink_port) = match (spec.source, spec.sink) {
            (CordEndpoint::Remote(candidate), CordEndpoint::Local { node, port })
                if candidate == endpoint =>
            {
                (node, port)
            }
            _ => return Err(SchedulerError::InvalidRemoteCordAccess),
        };
        if self.cords[cord_index].producer_closed {
            return if self.cords[cord_index].abnormal_terminal == abnormal_terminal {
                Ok(())
            } else {
                Err(SchedulerError::RemoteDeliveryRejected)
            };
        }
        self.ensure_sign_capacity(1)?;
        self.ensure_remote_sign_capacity(1)?;
        self.cords[cord_index].producer_closed = true;
        self.cords[cord_index].producer_abnormal = abnormal_terminal.is_some();
        self.cords[cord_index].abnormal_terminal = abnormal_terminal;
        self.ready[usize::from(sink_node.0)] = true;
        self.signs.record_remote(
            sink_node,
            sink_port,
            KernelEventKind::RemoteInputClosed,
            crate::RemoteLifecycleIdentity {
                endpoint,
                cord,
                direction: crate::RemoteCordDirection::Ingress,
                sequence: self.cords[cord_index].next_remote_sequence,
            },
        )?;
        Ok(())
    }

    pub fn remote_egress_terminal(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<bool, SchedulerError> {
        let cord_index = usize::from(cord.0);
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if !matches!(
            (spec.source, spec.sink),
            (
                CordEndpoint::Local { .. },
                CordEndpoint::Remote(candidate)
            ) if candidate == endpoint
        ) {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        Ok(self
            .remote_egress_terminal_disposition(endpoint, cord)?
            .is_some())
    }

    pub fn remote_egress_terminal_disposition(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<RemoteTerminalDisposition>, SchedulerError> {
        let cord_index = usize::from(cord.0);
        if cord_index >= self.active_cords {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let spec = *self
            .cord_specs
            .get(cord_index)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if !matches!(
            (spec.source, spec.sink),
            (
                CordEndpoint::Local { .. },
                CordEndpoint::Remote(candidate)
            ) if candidate == endpoint
        ) {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        let state = self.cords[cord_index];
        Ok(
            (state.producer_closed && state.len == 0).then_some(if state.producer_abnormal {
                RemoteTerminalDisposition::Abnormal
            } else {
                RemoteTerminalDisposition::NormalClose
            }),
        )
    }

    /// Exact typed terminal bytes for an abnormal remote egress after its
    /// ordinary values have drained.
    pub fn remote_egress_abnormal_terminal(
        &self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<CanonicalValue>, SchedulerError> {
        if self.remote_egress_terminal_disposition(endpoint, cord)?
            != Some(RemoteTerminalDisposition::Abnormal)
        {
            return Ok(None);
        }
        self.cords
            .get(usize::from(cord.0))
            .map(|state| state.abnormal_terminal)
            .ok_or(SchedulerError::InvalidRemoteCordAccess)
    }

    pub fn next_host_request(&mut self) -> Option<HostCallRequest> {
        self.next_host_request_matching(|_| true)
    }

    /// Selects the next undispatched request accepted by the host adapter.
    /// Non-matching requests remain undispatched and retain their exact order
    /// for a later call.
    pub fn next_host_request_matching(
        &mut self,
        mut accepts: impl FnMut(&HostCallRequest) -> bool,
    ) -> Option<HostCallRequest> {
        for offset in 0..PENDING_REQUESTS {
            let slot = (self.host_request_cursor + offset) % PENDING_REQUESTS;
            let Some(pending) = self.pending_host_calls[slot].as_mut() else {
                continue;
            };
            if pending.dispatched || !accepts(&pending.request) {
                continue;
            }
            pending.dispatched = true;
            self.host_request_cursor = (slot + 1) % PENDING_REQUESTS;
            return Some(pending.request);
        }
        None
    }

    pub fn has_ready_work(&self) -> bool {
        (0..self.active_nodes).any(|node| {
            let waiting_for_host_completion =
                self.pending_host_calls.iter().flatten().any(|pending| {
                    usize::from(pending.request.node.0) == node && pending.completion.is_none()
                });
            self.ready[node]
                && !self.completed[node]
                && (!waiting_for_host_completion
                    || self.drivers[node].accepts_input_while_host_call_pending())
        })
    }

    pub fn next_host_cancellation(&mut self) -> Option<HostCallCancellation> {
        let pending = self
            .pending_host_calls
            .iter_mut()
            .flatten()
            .find(|pending| {
                pending.cancellation_requested
                    && !pending.cancellation_dispatched
                    && pending.completion.is_none()
            })?;
        pending.cancellation_dispatched = true;
        Some(HostCallCancellation {
            node: pending.request.node,
            request: pending.request.request,
            call: pending.request.call,
        })
    }

    pub fn complete_host_call(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::HostCallCompletionRejected);
        }
        if usize::from(node.0) >= self.active_nodes {
            return Err(SchedulerError::HostCallCompletionRejected);
        }
        let slot = self
            .pending_host_calls
            .iter()
            .position(|pending| {
                pending
                    .map(|pending| {
                        pending.request.node == node && pending.request.request == request
                    })
                    .unwrap_or(false)
            })
            .ok_or(SchedulerError::HostCallCompletionRejected)?;
        let pending =
            self.pending_host_calls[slot].ok_or(SchedulerError::HostCallCompletionRejected)?;
        if !pending.dispatched || pending.completion.is_some() {
            return Err(SchedulerError::HostCallCompletionRejected);
        }
        if let Some(output) = outcome.output {
            if output.admitted_bytes == 0
                || output.value.byte_len > output.admitted_bytes
                || output.admitted_bytes > pending.maximum_output_bytes
                || output.value.byte_len > pending.maximum_output_bytes
            {
                return Err(SchedulerError::HostCallOutputExceeded);
            }
            self.values.get(output.value)?;
        }
        self.ensure_sign_capacity(1)?;
        // Input ownership remains with the pending completion until its node
        // commits the resume step. That transaction may reuse the same exact
        // value for a re-armed bounded Host request.
        self.pending_host_calls[slot]
            .as_mut()
            .ok_or(SchedulerError::HostCallCompletionRejected)?
            .completion = Some(outcome);
        self.ready[usize::from(pending.request.node.0)] = true;
        self.signs.record(
            pending.request.node,
            None,
            Some(request),
            KernelEventKind::HostCallCompleted,
        )?;
        Ok(())
    }

    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        if self.cancelled {
            return Err(SchedulerError::Cancelled);
        }
        Ok(self.values.store(bytes)?)
    }

    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        Ok(self.values.get(value)?)
    }

    pub fn discard_host_value(&mut self, value: ValueRef) -> Result<(), SchedulerError> {
        if self.pending_host_calls.iter().flatten().any(|pending| {
            pending.request.input.value == value
                || pending
                    .completion
                    .and_then(|outcome| outcome.output)
                    .map(|output| output.value)
                    == Some(value)
        }) || self
            .queue_slots
            .iter()
            .flatten()
            .any(|queued| *queued == value)
        {
            return Err(SchedulerError::ValueOwnershipViolation);
        }
        Ok(self.values.release(value)?)
    }

    pub fn pending_host_call_count(&self) -> usize {
        self.pending_host_calls
            .iter()
            .filter(|pending| pending.is_some())
            .count()
    }

    fn next_ready(&mut self) -> Option<usize> {
        for offset in 0..self.active_nodes {
            let node = (self.cursor + offset) % self.active_nodes;
            let waiting_for_host_completion =
                self.pending_host_calls.iter().flatten().any(|pending| {
                    usize::from(pending.request.node.0) == node && pending.completion.is_none()
                });
            if self.ready[node]
                && !self.completed[node]
                && (!waiting_for_host_completion
                    || self.drivers[node].accepts_input_while_host_call_pending())
            {
                self.cursor = (node + 1) % self.active_nodes;
                return Some(node);
            }
        }
        None
    }

    fn context(&self, node: usize) -> Result<StepIo<PORTS>, SchedulerError> {
        let mut inputs = [None; PORTS];
        let mut input_closed = [false; PORTS];
        let mut input_abnormal = [None; PORTS];
        let mut output_maximum_bytes = [None; PORTS];
        let node_id = NodeId(as_u16(node)?);
        let host_completion = self
            .pending_host_calls
            .iter()
            .flatten()
            .find(|pending| usize::from(pending.request.node.0) == node)
            .and_then(|pending| {
                pending
                    .completion
                    .map(|outcome| (pending.request.request, outcome))
            });
        for (port, cord) in self.node_specs[node]
            .input_cords
            .iter()
            .copied()
            .enumerate()
        {
            let Some(cord) = cord else {
                continue;
            };
            if self.terminal_inputs_consumed[node][port] {
                continue;
            }
            let cord_index = usize::from(cord.0);
            let projected_recovery_handles_source = self.cords[cord_index].producer_abnormal
                && self.recovery_for_cord[..self.active_cords]
                    .iter()
                    .enumerate()
                    .any(|(recovery_cord, recovery)| {
                        *recovery == Some(node_id)
                            && self.cord_specs[recovery_cord].source
                                == self.cord_specs[cord_index].source
                    });
            inputs[port] = self.peek(cord_index)?;
            input_closed[port] = self.cords[cord_index].producer_closed
                && (!self.cords[cord_index].producer_abnormal || projected_recovery_handles_source)
                && self.cords[cord_index].len == 0;
            input_abnormal[port] = (self.cords[cord_index].producer_closed
                && self.cords[cord_index].producer_abnormal
                && !projected_recovery_handles_source
                && self.cords[cord_index].len == 0)
                .then_some(self.cords[cord_index].abnormal_terminal)
                .flatten();
        }
        for (port, output_maximum) in output_maximum_bytes.iter_mut().enumerate() {
            let Ok(targets) = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))
            else {
                continue;
            };
            let mut maximum = u32::MAX;
            let mut any = false;
            for target in targets {
                let cord = usize::from(target.cord.0);
                let state = self.cords.get(cord).ok_or(SchedulerError::InvalidPlan)?;
                let spec = self
                    .cord_specs
                    .get(cord)
                    .ok_or(SchedulerError::InvalidPlan)?;
                if spec.track != AssignedConnectionTrack::Payload {
                    continue;
                }
                if state.producer_closed {
                    maximum = 0;
                    any = true;
                    break;
                }
                maximum = maximum.min(self.admission_maximum(*spec, state)?);
                any = true;
            }
            if any && maximum > 0 {
                *output_maximum = Some(maximum);
            }
        }
        Ok(StepIo {
            inputs,
            input_closed,
            input_abnormal,
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
            maximum_fuel: self.node_specs[node].maximum_step_fuel,
            fuel_consumed: 0,
            fault: None,
        })
    }

    fn apply_step(
        &mut self,
        node: usize,
        outcome: StepOutcome,
        mut io: StepIo<PORTS>,
    ) -> Result<(), SchedulerError> {
        let staged = io.staged();
        let terminal_phase = self.validate_terminal_transduction(node, outcome, &io)?;
        let cancellation_pending = self.validate_terminal_cancellation(node, outcome, &io)?;
        let projected_recovery = self.projected_recovery_for_step(node, &io)?;
        match outcome {
            StepOutcome::Progress if !staged => return Err(SchedulerError::FalseProgress),
            StepOutcome::Await if staged => return Err(SchedulerError::FalseProgress),
            StepOutcome::Yield if staged || io.fuel_consumed != io.maximum_fuel => {
                return Err(SchedulerError::FalseProgress);
            }
            StepOutcome::Fail(code) => {
                self.signs.record(
                    NodeId(as_u16(node)?),
                    None,
                    None,
                    KernelEventKind::BackFailed,
                )?;
                self.signs.observe_debug(DebugRuntimeEvent {
                    node: NodeId(as_u16(node)?),
                    port: None,
                    cord: None,
                    kind: DebugEventKind::Fault,
                    type_identity: None,
                    value: None,
                    fault_code: Some(code.detail),
                });
                return Err(SchedulerError::BackFailed(code));
            }
            StepOutcome::Abnormal { .. }
                if io.host_request.is_some()
                    || io.host_cancellation.is_some()
                    || ((io.outputs.iter().any(Option::is_some)
                        || io.canonical_output.is_some()
                        || io.prepared_output.is_some())
                        && !matches!(
                            (
                                self.terminal_phases[node],
                                self.terminal_phases[node].and_then(|phase| {
                                    let input = match phase {
                                        ActiveTerminalPhase::Normal { input, .. }
                                        | ActiveTerminalPhase::Abnormal { input, .. } => input,
                                    };
                                    self.terminal_transductions[node][usize::from(input.0)]
                                })
                            ),
                            (
                                Some(ActiveTerminalPhase::Abnormal { .. }),
                                Some(AssignedTerminalTransduction {
                                    abnormal: AssignedAbnormalTransduction::FinalizeThenPropagate(
                                        _
                                    ),
                                    ..
                                })
                            )
                        )
                        && !self.step_begins_abnormal_finalization(node, &io)) =>
            {
                return Err(SchedulerError::FalseProgress);
            }
            _ => {}
        }

        if matches!(
            outcome,
            StepOutcome::Progress | StepOutcome::Complete | StepOutcome::Abnormal { .. }
        ) {
            let complete_sign_records = if matches!(
                outcome,
                StepOutcome::Complete | StepOutcome::Abnormal { .. }
            ) {
                if io.host_request.is_some() || io.host_cancellation.is_some() {
                    return Err(SchedulerError::InvalidHostCallAccess);
                }
                self.output_route_count(node)?
                    .checked_add(1)
                    .and_then(|count| {
                        count.checked_add(usize::from(
                            outcome == StepOutcome::Complete
                                && (projected_recovery.is_some()
                                    || self.projected_recovery_source[node].is_some()),
                        ))
                    })
                    .ok_or(SchedulerError::InvalidPlan)?
            } else {
                0
            };
            let host_sign_records = usize::from(io.host_request.is_some())
                .checked_add(usize::from(io.host_cancellation.is_some()))
                .and_then(|count| count.checked_add(complete_sign_records))
                .ok_or(SchedulerError::InvalidPlan)?;
            let generated = if let Some((port, declared_len)) = io.prepared_output {
                let bytes = self.drivers[node]
                    .prepared_output(port)
                    .ok_or(SchedulerError::InvalidPlan)?;
                if u32::try_from(bytes.len()).ok() != Some(declared_len) {
                    return Err(SchedulerError::InvalidPlan);
                }
                let value = self.values.store(bytes)?;
                let output = io
                    .outputs
                    .get_mut(usize::from(port.0))
                    .ok_or(SchedulerError::InvalidPortAccess)?;
                if output.is_some() {
                    self.values.release(value)?;
                    return Err(SchedulerError::InvalidPortAccess);
                }
                *output = Some(value);
                Some(value)
            } else {
                derived_value::materialize(&mut self.values, io.canonical_output, &mut io.outputs)?
            };
            let mut sign_records =
                match self.commit_event_count(node, &io.consumed, &io.consumed_closed, &io.outputs)
                {
                    Ok(count) => count,
                    Err(error) => {
                        if let Some(value) = generated {
                            let _ = self.values.release(value);
                        }
                        return Err(error);
                    }
                };
            sign_records = match sign_records.checked_add(host_sign_records) {
                Some(count) => count,
                None => {
                    if let Some(value) = generated {
                        let _ = self.values.release(value);
                    }
                    return Err(SchedulerError::InvalidPlan);
                }
            };
            if let Err(error) = self.ensure_sign_capacity(sign_records) {
                if let Some(value) = generated {
                    let _ = self.values.release(value);
                }
                return Err(error);
            }
            if let Err(error) = self.commit(node, io.staged_step()) {
                if let Some(value) = generated {
                    let _ = self.values.release(value);
                }
                return Err(error);
            }
            self.drivers[node].step_committed();
            if let Some(source) = projected_recovery {
                self.projected_recovery_source[node] = Some(source);
            }
        }
        match outcome {
            StepOutcome::Progress => {
                self.ready[node] = io.host_request.is_none() && io.host_cancellation.is_none()
            }
            StepOutcome::Yield => {
                self.signs.record(
                    NodeId(as_u16(node)?),
                    None,
                    None,
                    KernelEventKind::StepYielded,
                )?;
                self.ready[node] = true;
            }
            StepOutcome::Await => self.ready[node] = false,
            StepOutcome::Complete => {
                self.completed[node] = true;
                self.ready[node] = false;
                self.close_outputs(node)?;
                if let Some(source) = self.projected_recovery_source[node].take() {
                    self.unresolved_abnormal[usize::from(source.0)] = None;
                    self.signs.record(
                        NodeId(as_u16(node)?),
                        self.terminal_transductions[node]
                            .iter()
                            .flatten()
                            .find(|contract| {
                                matches!(contract.abnormal, AssignedAbnormalTransduction::Recover)
                            })
                            .map(|contract| contract.input),
                        None,
                        KernelEventKind::SemanticAbnormalRecovered,
                    )?;
                }
                self.signs.record(
                    NodeId(as_u16(node)?),
                    None,
                    None,
                    KernelEventKind::BackCompleted,
                )?;
                self.signs.observe_debug(DebugRuntimeEvent {
                    node: NodeId(as_u16(node)?),
                    port: None,
                    cord: None,
                    kind: DebugEventKind::GearCompleted,
                    type_identity: None,
                    value: None,
                    fault_code: None,
                });
            }
            StepOutcome::Abnormal { port, terminal } => {
                self.completed[node] = true;
                self.ready[node] = false;
                let routed = self.abnormally_terminate_outputs(node, port, terminal)?;
                self.signs.record(
                    NodeId(as_u16(node)?),
                    Some(port),
                    None,
                    KernelEventKind::SemanticAbnormal,
                )?;
                if !routed {
                    return Err(SchedulerError::SemanticAbnormal {
                        node: NodeId(as_u16(node)?),
                        port,
                        terminal,
                    });
                }
            }
            StepOutcome::Fail(_) => unreachable!(),
        }
        if matches!(outcome, StepOutcome::Progress | StepOutcome::Yield) {
            self.arm_quiescence_outputs(node)?;
        }
        self.terminal_phases[node] = terminal_phase;
        self.terminal_cancellation_pending[node] = cancellation_pending;
        Ok(())
    }

    fn arm_quiescence_outputs(&mut self, node: usize) -> Result<(), SchedulerError> {
        let node = NodeId(as_u16(node)?);
        for cord in 0..self.active_cords {
            if self.cord_specs[cord].track == AssignedConnectionTrack::Quiescence
                && self.cord_specs[cord].source_local().map(|source| source.0) == Some(node)
            {
                self.cords[cord].quiescence_armed = true;
            }
        }
        Ok(())
    }

    /// Emit one wakeable edge for every source that became active and has now
    /// returned the whole play to quiescence. Empty queues alone do not arm an
    /// edge, and terminal sources are never reported as quiescent.
    fn emit_quiescence_transitions(&mut self) -> Result<Option<NodeId>, SchedulerError> {
        let mut count = 0_usize;
        let mut first_source = None;
        for cord in 0..self.active_cords {
            let spec = self.cord_specs[cord];
            if spec.track != AssignedConnectionTrack::Quiescence
                || !self.cords[cord].quiescence_armed
            {
                continue;
            }
            let Some((source, _)) = spec.source_local() else {
                continue;
            };
            if self.completed[usize::from(source.0)] || self.cords[cord].len != 0 {
                continue;
            }
            first_source.get_or_insert(source);
            count = count.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
        }
        if count == 0 {
            return Ok(None);
        }
        self.ensure_sign_capacity(count)?;
        let value = self.values.store(&[])?;
        for _ in 1..count {
            self.values.retain(value)?;
        }
        for cord in 0..self.active_cords {
            let spec = self.cord_specs[cord];
            if spec.track != AssignedConnectionTrack::Quiescence
                || !self.cords[cord].quiescence_armed
            {
                continue;
            }
            let Some((source, port)) = spec.source_local() else {
                continue;
            };
            if self.completed[usize::from(source.0)] || self.cords[cord].len != 0 {
                continue;
            }
            self.push(cord, value)?;
            self.cords[cord].quiescence_armed = false;
            if let Some((sink, _)) = spec.sink_local() {
                self.ready[usize::from(sink.0)] = true;
            }
            self.signs
                .record(source, Some(port), None, KernelEventKind::ValueRouted)?;
        }
        Ok(first_source)
    }

    fn projected_recovery_for_step(
        &self,
        node: usize,
        io: &StepIo<PORTS>,
    ) -> Result<Option<NodeId>, SchedulerError> {
        let mut source = None;
        for (port, consumed) in io.consumed.iter().copied().enumerate() {
            if !consumed {
                continue;
            }
            let Some(cord) = self.node_specs[node].input_cords[port] else {
                continue;
            };
            let cord = usize::from(cord.0);
            if self.recovery_for_cord[cord] != Some(NodeId(as_u16(node)?)) {
                continue;
            }
            let CordEndpoint::Local {
                node: origin,
                port: origin_port,
            } = self.cord_specs[cord].source
            else {
                return Err(SchedulerError::InvalidPlan);
            };
            let unresolved = self.unresolved_abnormal[usize::from(origin.0)]
                .ok_or(SchedulerError::InvalidPlan)?;
            if unresolved.port != origin_port {
                return Err(SchedulerError::InvalidPlan);
            }
            let input = io.inputs[port].ok_or(SchedulerError::InvalidPlan)?;
            if self.values.get(input)? != unresolved.terminal.as_slice() {
                return Err(SchedulerError::InvalidPlan);
            }
            if source.replace(origin).is_some_and(|other| other != origin)
                || self.projected_recovery_source[node].is_some_and(|other| other != origin)
            {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        Ok(source)
    }

    fn step_begins_abnormal_finalization(&self, node: usize, io: &StepIo<PORTS>) -> bool {
        self.terminal_transductions[node]
            .iter()
            .flatten()
            .any(|contract| {
                let input = usize::from(contract.input.0);
                matches!(
                    contract.abnormal,
                    AssignedAbnormalTransduction::FinalizeThenPropagate(_)
                ) && input < PORTS
                    && io.input_abnormal[input].is_some()
                    && io.consumed_closed[input]
            })
    }

    fn validate_terminal_cancellation(
        &self,
        node: usize,
        outcome: StepOutcome,
        io: &StepIo<PORTS>,
    ) -> Result<bool, SchedulerError> {
        let Some(contract) = self.terminal_transductions[node]
            .iter()
            .flatten()
            .find(|contract| {
                matches!(
                    contract.cancellation,
                    AssignedCancellationTransduction::Request { .. }
                )
            })
            .copied()
        else {
            return Ok(false);
        };
        let AssignedCancellationTransduction::Request { input, .. } = contract.cancellation else {
            return Ok(false);
        };
        let input = usize::from(input.0);
        if input >= PORTS {
            return Err(SchedulerError::InvalidPlan);
        }
        let requested = io.consumed[input];
        let pending = self.terminal_cancellation_pending[node] || requested;
        if !pending {
            return Ok(false);
        }
        match outcome {
            StepOutcome::Abnormal { port, .. } if port == contract.output => Ok(false),
            StepOutcome::Abnormal { .. } | StepOutcome::Complete => {
                Err(SchedulerError::InvalidPlan)
            }
            StepOutcome::Progress | StepOutcome::Await | StepOutcome::Yield => Ok(true),
            StepOutcome::Fail(_) => Ok(pending),
        }
    }

    fn validate_terminal_transduction(
        &self,
        node: usize,
        outcome: StepOutcome,
        io: &StepIo<PORTS>,
    ) -> Result<Option<ActiveTerminalPhase>, SchedulerError> {
        let active_input = self.terminal_phases[node].map(|phase| match phase {
            ActiveTerminalPhase::Normal { input, .. }
            | ActiveTerminalPhase::Abnormal { input, .. } => input,
        });
        let first_consumed = io.consumed_closed.iter().position(|consumed| *consumed);
        let mut contracted_consumed = io
            .consumed_closed
            .iter()
            .enumerate()
            .filter(|(port, consumed)| {
                **consumed && self.terminal_transductions[node][*port].is_some()
            })
            .map(|(port, _)| port);
        let consumed_input = contracted_consumed.next().or(first_consumed);
        if contracted_consumed.next().is_some() {
            return Err(SchedulerError::InvalidPlan);
        }
        if io
            .input_abnormal
            .iter()
            .enumerate()
            .any(|(port, terminal)| {
                terminal.is_some() && self.terminal_transductions[node][port].is_none()
            })
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let selected_input = active_input
            .map(|port| usize::from(port.0))
            .or(consumed_input)
            .or_else(|| {
                (0..PORTS).find(|port| {
                    io.inputs[*port].is_none()
                        && (io.input_abnormal[*port].is_some()
                            || (io.input_closed[*port]
                                && self.terminal_transductions[node][*port].is_some()))
                })
            });
        let Some(selected_input) = selected_input else {
            return Ok(None);
        };
        let Some(contract) = self.terminal_transductions[node][selected_input] else {
            if io.input_abnormal[selected_input].is_some() {
                return Err(SchedulerError::InvalidPlan);
            }
            return Ok(None);
        };
        let input = usize::from(contract.input.0);
        if input >= PORTS {
            return Err(SchedulerError::InvalidPlan);
        }
        if let Some(phase) = self.terminal_phases[node] {
            return self.advance_terminal_phase(contract, phase, outcome, io);
        }
        let terminal_is_exposed = (io.input_abnormal[input].is_some()
            || (io.input_closed[input]
                && !matches!(
                    contract.normal_close,
                    AssignedNormalCloseTransduction::NotAccepted
                )))
            && io.inputs[input].is_none();
        if terminal_is_exposed && !io.consumed_closed[input] {
            let may_wait_for_finite_output_capacity = if io.input_abnormal[input].is_some() {
                matches!(
                    contract.abnormal,
                    AssignedAbnormalTransduction::FinalizeThenPropagate(_)
                )
            } else {
                matches!(
                    contract.normal_close,
                    AssignedNormalCloseTransduction::FlushThenPropagate(_)
                        | AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(_)
                )
            };
            return if may_wait_for_finite_output_capacity && outcome == StepOutcome::Await {
                Ok(None)
            } else {
                // Non-buffered contracts must consume exposed truth now. A
                // finite flushing Back may await only before beginning its
                // admitted terminal phase, typically under output pressure.
                Err(SchedulerError::InvalidPlan)
            };
        }
        let consumed = io
            .input_abnormal
            .iter()
            .zip(io.consumed_closed)
            .enumerate()
            .find_map(|(port, (terminal, consumed))| consumed.then_some((port, *terminal)))
            .and_then(|(port, terminal)| terminal.map(|terminal| (port, terminal)));
        if let Some((consumed_input, terminal)) = consumed {
            if input != consumed_input {
                return Err(SchedulerError::InvalidPlan);
            }
            return match contract.abnormal {
                AssignedAbnormalTransduction::NotAccepted => Err(SchedulerError::InvalidPlan),
                AssignedAbnormalTransduction::PropagateAfterDrain => {
                    if outcome
                        != (StepOutcome::Abnormal {
                            port: contract.output,
                            terminal,
                        })
                    {
                        return Err(SchedulerError::InvalidPlan);
                    }
                    Ok(None)
                }
                AssignedAbnormalTransduction::FinalizeThenPropagate(bound) => {
                    let emitted = self.accumulate_terminal_emission(
                        AssignedFiniteTerminalEmission {
                            maximum_items: 0,
                            maximum_bytes: 0,
                        },
                        io,
                        bound,
                    )?;
                    match outcome {
                        StepOutcome::Abnormal {
                            port,
                            terminal: propagated,
                        } if port == contract.output && propagated == terminal => Ok(None),
                        StepOutcome::Progress => Ok(Some(ActiveTerminalPhase::Abnormal {
                            input: contract.input,
                            terminal,
                            emitted,
                        })),
                        StepOutcome::Fail(_) => Ok(None),
                        _ => Err(SchedulerError::InvalidPlan),
                    }
                }
                AssignedAbnormalTransduction::Recover => {
                    if matches!(outcome, StepOutcome::Abnormal { .. }) {
                        Err(SchedulerError::InvalidPlan)
                    } else {
                        Ok(None)
                    }
                }
                AssignedAbnormalTransduction::DomainSpecific { .. } => match outcome {
                    StepOutcome::Abnormal { port, .. } if port != contract.output => {
                        Err(SchedulerError::InvalidPlan)
                    }
                    _ => Ok(None),
                },
            };
        }
        if !io.consumed_closed[input] {
            return Ok(None);
        }
        use AssignedNormalCloseTransduction as Normal;
        match contract.normal_close {
            Normal::NotAccepted => Err(SchedulerError::InvalidPlan),
            Normal::PropagateAfterDrain => {
                if outcome == StepOutcome::Complete {
                    Ok(None)
                } else {
                    Err(SchedulerError::InvalidPlan)
                }
            }
            Normal::FlushThenPropagate(bound) => {
                let emitted = self.accumulate_terminal_emission(
                    AssignedFiniteTerminalEmission {
                        maximum_items: 0,
                        maximum_bytes: 0,
                    },
                    io,
                    bound,
                )?;
                match outcome {
                    StepOutcome::Complete => Ok(None),
                    StepOutcome::Progress => Ok(Some(ActiveTerminalPhase::Normal {
                        input: contract.input,
                        emitted,
                    })),
                    StepOutcome::Fail(_) => Ok(None),
                    _ => Err(SchedulerError::InvalidPlan),
                }
            }
            Normal::FlushThenPropagateWhenAllClose(bound) => {
                let emitted = self.accumulate_terminal_emission(
                    AssignedFiniteTerminalEmission {
                        maximum_items: 0,
                        maximum_bytes: 0,
                    },
                    io,
                    bound,
                )?;
                let last =
                    self.terminal_transductions[node]
                        .iter()
                        .enumerate()
                        .all(|(port, profile)| {
                            profile.is_none()
                                || port == input
                                || self.terminal_inputs_consumed[node][port]
                        });
                if last {
                    match outcome {
                        StepOutcome::Complete => Ok(None),
                        StepOutcome::Progress => Ok(Some(ActiveTerminalPhase::Normal {
                            input: contract.input,
                            emitted,
                        })),
                        StepOutcome::Fail(_) => Ok(None),
                        _ => Err(SchedulerError::InvalidPlan),
                    }
                } else {
                    match outcome {
                        StepOutcome::Progress | StepOutcome::Fail(_) => Ok(None),
                        _ => Err(SchedulerError::InvalidPlan),
                    }
                }
            }
            Normal::PropagateWhenAllClose => {
                let last =
                    self.terminal_transductions[node]
                        .iter()
                        .enumerate()
                        .all(|(port, profile)| {
                            profile.is_none()
                                || port == input
                                || self.terminal_inputs_consumed[node][port]
                        });
                match (last, outcome) {
                    (false, StepOutcome::Progress | StepOutcome::Fail(_))
                    | (true, StepOutcome::Complete | StepOutcome::Fail(_)) => Ok(None),
                    _ => Err(SchedulerError::InvalidPlan),
                }
            }
            Normal::Consume => {
                if matches!(outcome, StepOutcome::Abnormal { .. }) {
                    Err(SchedulerError::InvalidPlan)
                } else {
                    Ok(None)
                }
            }
            Normal::DomainSpecific { .. } => match outcome {
                StepOutcome::Abnormal { port, .. } if port != contract.output => {
                    Err(SchedulerError::InvalidPlan)
                }
                _ => Ok(None),
            },
        }
    }

    fn advance_terminal_phase(
        &self,
        contract: AssignedTerminalTransduction,
        phase: ActiveTerminalPhase,
        outcome: StepOutcome,
        io: &StepIo<PORTS>,
    ) -> Result<Option<ActiveTerminalPhase>, SchedulerError> {
        match phase {
            ActiveTerminalPhase::Normal { input, emitted } => {
                let bound = match contract.normal_close {
                    AssignedNormalCloseTransduction::FlushThenPropagate(bound)
                    | AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(bound) => {
                        bound
                    }
                    _ => return Err(SchedulerError::InvalidPlan),
                };
                let emitted = self.accumulate_terminal_emission(emitted, io, bound)?;
                match outcome {
                    StepOutcome::Complete => Ok(None),
                    StepOutcome::Progress | StepOutcome::Await | StepOutcome::Yield => {
                        Ok(Some(ActiveTerminalPhase::Normal { input, emitted }))
                    }
                    _ => Err(SchedulerError::InvalidPlan),
                }
            }
            ActiveTerminalPhase::Abnormal {
                input,
                terminal,
                emitted,
            } => {
                let AssignedAbnormalTransduction::FinalizeThenPropagate(bound) = contract.abnormal
                else {
                    return Err(SchedulerError::InvalidPlan);
                };
                let emitted = self.accumulate_terminal_emission(emitted, io, bound)?;
                match outcome {
                    StepOutcome::Abnormal {
                        port,
                        terminal: propagated,
                    } if port == contract.output && propagated == terminal => Ok(None),
                    StepOutcome::Progress | StepOutcome::Await | StepOutcome::Yield => {
                        Ok(Some(ActiveTerminalPhase::Abnormal {
                            input,
                            terminal,
                            emitted,
                        }))
                    }
                    _ => Err(SchedulerError::InvalidPlan),
                }
            }
        }
    }

    fn accumulate_terminal_emission(
        &self,
        emitted: AssignedFiniteTerminalEmission,
        io: &StepIo<PORTS>,
        bound: AssignedFiniteTerminalEmission,
    ) -> Result<AssignedFiniteTerminalEmission, SchedulerError> {
        let mut items = emitted.maximum_items;
        let mut bytes = emitted.maximum_bytes;
        for value in io.outputs.iter().flatten() {
            items = items.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
            bytes = bytes
                .checked_add(
                    u32::try_from(self.values.get(*value)?.len())
                        .map_err(|_| SchedulerError::InvalidPlan)?,
                )
                .ok_or(SchedulerError::InvalidPlan)?;
        }
        if let Some((_, value)) = &io.canonical_output {
            items = items.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
            bytes = bytes
                .checked_add(
                    u32::try_from(value.as_slice().len())
                        .map_err(|_| SchedulerError::InvalidPlan)?,
                )
                .ok_or(SchedulerError::InvalidPlan)?;
        }
        if let Some((_, byte_len)) = io.prepared_output {
            items = items.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
            bytes = bytes
                .checked_add(byte_len)
                .ok_or(SchedulerError::InvalidPlan)?;
        }
        if items > bound.maximum_items || bytes > bound.maximum_bytes {
            return Err(SchedulerError::InvalidPlan);
        }
        Ok(AssignedFiniteTerminalEmission {
            maximum_items: items,
            maximum_bytes: bytes,
        })
    }

    fn commit(&mut self, node: usize, staged: StagedStep<PORTS>) -> Result<(), SchedulerError> {
        let StagedStep {
            consumed,
            retained_inputs,
            consumed_closed,
            outputs,
            discards,
            consumed_host_completion,
            host_request,
            host_cancellation,
        } = staged;
        let mut retained_values = [None; PORTS];
        for (port, retained) in retained_inputs.iter().copied().enumerate() {
            if retained {
                let cord = self.node_specs[node].input_cords[port]
                    .ok_or(SchedulerError::InvalidPortAccess)?;
                retained_values[port] = self.peek(usize::from(cord.0))?;
            }
        }
        let admitted_host_request = self.preflight_step(node, &staged, &retained_values)?;

        if let Some(request) = host_cancellation {
            let pending = self
                .pending_host_calls
                .iter_mut()
                .flatten()
                .find(|pending| {
                    usize::from(pending.request.node.0) == node
                        && pending.request.request == request
                })
                .ok_or(SchedulerError::HostCallCancellationRejected)?;
            pending.cancellation_requested = true;
            self.signs.record(
                NodeId(as_u16(node)?),
                None,
                Some(request),
                KernelEventKind::HostCallCancellationRequested,
            )?;
        }

        for (port, consumed) in consumed_closed.iter().copied().enumerate() {
            if consumed {
                let cord = self.node_specs[node].input_cords[port]
                    .ok_or(SchedulerError::InvalidPortAccess)?;
                let kind = if self.cords[usize::from(cord.0)].producer_abnormal {
                    KernelEventKind::InputAbnormal
                } else {
                    KernelEventKind::InputClosed
                };
                self.signs.record(
                    NodeId(as_u16(node)?),
                    Some(PortId(as_u16(port)?)),
                    None,
                    kind,
                )?;
                self.terminal_inputs_consumed[node][port] = true;
            }
        }

        let mut consumed_values = [None; PORTS];
        for (port, consume) in consumed.iter().copied().enumerate() {
            if !consume {
                continue;
            }
            let cord =
                self.node_specs[node].input_cords[port].ok_or(SchedulerError::InvalidPortAccess)?;
            let value = self.pop(usize::from(cord.0))?;
            consumed_values[port] = Some(value);
            let spec = self.cord_specs[usize::from(cord.0)];
            if let Some((source_node, _)) = spec.source_local() {
                self.ready[usize::from(source_node.0)] = true;
            }
            self.signs.record(
                NodeId(as_u16(node)?),
                Some(PortId(as_u16(port)?)),
                None,
                KernelEventKind::ValueConsumed,
            )?;
            self.signs.observe_debug(DebugRuntimeEvent {
                node: NodeId(as_u16(node)?),
                port: Some(PortId(as_u16(port)?)),
                cord: Some(cord),
                kind: DebugEventKind::ValueReceived,
                type_identity: None,
                value: Some(self.values.get(value)?),
                fault_code: None,
            });
        }

        let (consumed_host_input, consumed_host_value) = if consumed_host_completion {
            let slot = self
                .pending_host_calls
                .iter()
                .position(|pending| {
                    pending
                        .and_then(|pending| pending.completion)
                        .is_some_and(|_| {
                            pending
                                .map(|pending| usize::from(pending.request.node.0) == node)
                                .unwrap_or(false)
                        })
                })
                .ok_or(SchedulerError::InvalidHostCallAccess)?;
            let pending = self.pending_host_calls[slot]
                .take()
                .ok_or(SchedulerError::InvalidHostCallAccess)?;
            let retains_input = self.drivers[node]
                .retains_host_call_input(pending.request.request, pending.request.input.value);
            let completion = pending
                .completion
                .ok_or(SchedulerError::InvalidHostCallAccess)?;
            (
                (pending.maximum_input_bytes > 0
                    && !retains_input
                    && completion.output.map(|output| output.value)
                        != Some(pending.request.input.value))
                .then_some(pending.request.input.value),
                completion.output.map(|output| output.value),
            )
        } else {
            (None, None)
        };

        let mut handled = [None; PORTS];
        for value in outputs.iter().copied().flatten() {
            if handled.iter().flatten().any(|handled| *handled == value) {
                continue;
            }
            let consumed_references = consumed_values
                .iter()
                .flatten()
                .filter(|candidate| **candidate == value)
                .count()
                + usize::from(consumed_host_value == Some(value));
            let base_references = consumed_references.max(1);
            let target_references =
                self.step_target_count(node, &outputs, host_request, &retained_values, value)?;
            if target_references > base_references {
                for _ in 0..(target_references - base_references) {
                    self.values.retain(value)?;
                }
            } else {
                for _ in 0..(base_references - target_references) {
                    self.values.release(value)?;
                }
            }
            let slot = handled
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or(SchedulerError::InvalidPortAccess)?;
            *slot = Some(value);
        }
        if let Some((_, _, input)) = host_request {
            let value = input.value;
            if !outputs.iter().flatten().any(|output| *output == value) {
                let consumed_references = consumed_values
                    .iter()
                    .flatten()
                    .filter(|candidate| **candidate == value)
                    .count()
                    + usize::from(consumed_host_value == Some(value));
                if consumed_references == 0
                    && retained_values
                        .iter()
                        .flatten()
                        .any(|retained| *retained == value)
                {
                    // The cord keeps its reference while the admitted Host Call
                    // owns one additional pinned reference until completion.
                    self.values.retain(value)?;
                }
                let base_references = consumed_references.max(1);
                if base_references > 1 {
                    for _ in 0..(base_references - 1) {
                        self.values.release(value)?;
                    }
                }
            }
        }
        for (port, value) in consumed_values.iter().copied().enumerate() {
            let Some(value) = value else {
                continue;
            };
            if !outputs.iter().flatten().any(|output| *output == value)
                && host_request.map(|request| request.2.value) != Some(value)
                && !retained_inputs[port]
            {
                self.values.release(value)?;
            }
        }
        if let Some(value) = consumed_host_value {
            if !outputs.iter().flatten().any(|output| *output == value)
                && host_request.map(|request| request.2.value) != Some(value)
            {
                self.values.release(value)?;
            }
        }
        if let Some(value) = consumed_host_input {
            if consumed_host_value != Some(value)
                && host_request.map(|request| request.2.value) != Some(value)
            {
                self.values.release(value)?;
            }
        }

        if let (Some((request, call, input)), Some(binding)) = (host_request, admitted_host_request)
        {
            let slot = self
                .pending_host_calls
                .iter_mut()
                .find(|pending| pending.is_none())
                .ok_or(SchedulerError::HostCallCapacityExceeded)?;
            *slot = Some(PendingHostCall {
                request: HostCallRequest {
                    node: NodeId(as_u16(node)?),
                    request,
                    call,
                    input,
                },
                maximum_input_bytes: binding.maximum_input_bytes,
                maximum_output_bytes: binding.maximum_output_bytes,
                dispatched: false,
                cancellation_requested: false,
                cancellation_dispatched: false,
                completion: None,
            });
            self.last_host_request[node] = Some(request);
            self.signs.record(
                NodeId(as_u16(node)?),
                None,
                Some(request),
                KernelEventKind::HostCallRequested,
            )?;
        }

        for discard in discards.iter().copied().flatten() {
            self.values.release(discard)?;
        }

        for (port, value) in outputs.iter().copied().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let targets = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))?;
            let targets = targets.collect_targets::<ROUTE_TARGETS>()?;
            for target in targets.iter() {
                if self.cord_specs[usize::from(target.cord.0)].track
                    != AssignedConnectionTrack::Payload
                {
                    continue;
                }
                if let Some(superseded) = self.push(usize::from(target.cord.0), value)? {
                    self.values.release(superseded)?;
                }
                if let CordEndpoint::Local { node, .. } = target.sink {
                    self.ready[usize::from(node.0)] = true;
                }
                self.signs.record(
                    NodeId(as_u16(node)?),
                    Some(PortId(as_u16(port)?)),
                    None,
                    KernelEventKind::ValueRouted,
                )?;
                self.signs.observe_debug(DebugRuntimeEvent {
                    node: NodeId(as_u16(node)?),
                    port: Some(PortId(as_u16(port)?)),
                    cord: Some(target.cord),
                    kind: DebugEventKind::ValueSent,
                    type_identity: None,
                    value: Some(self.values.get(value)?),
                    fault_code: None,
                });
            }
        }
        Ok(())
    }

    fn preflight_step(
        &self,
        node: usize,
        staged: &StagedStep<PORTS>,
        retained_values: &[Option<ValueRef>; PORTS],
    ) -> Result<Option<HostCallBinding>, SchedulerError> {
        let consumed = &staged.consumed;
        let retained_inputs = &staged.retained_inputs;
        let outputs = &staged.outputs;
        let discards = &staged.discards;
        let consumed_host_completion = staged.consumed_host_completion;
        let host_request = staged.host_request;
        let host_cancellation = staged.host_cancellation;
        if host_request.is_some() && host_cancellation.is_some() {
            return Err(SchedulerError::InvalidHostCallAccess);
        }
        if consumed_host_completion && host_cancellation.is_some() {
            return Err(SchedulerError::InvalidHostCallAccess);
        }
        if retained_inputs.iter().enumerate().any(|(port, retained)| {
            *retained
                && !consumed[port]
                && host_request.map(|request| request.2.value)
                    != self.node_specs[node].input_cords[port]
                        .and_then(|cord| self.peek(usize::from(cord.0)).ok().flatten())
        }) {
            return Err(SchedulerError::InvalidPortAccess);
        }
        let completed_pending = self.pending_host_calls.iter().flatten().find(|pending| {
            usize::from(pending.request.node.0) == node && pending.completion.is_some()
        });
        let available_host_value = completed_pending
            .and_then(|pending| pending.completion)
            .and_then(|outcome| outcome.output)
            .map(|output| output.value);
        let consumed_host_value = if consumed_host_completion {
            completed_pending
                .ok_or(SchedulerError::InvalidHostCallAccess)?
                .completion
                .and_then(|outcome| outcome.output)
                .map(|output| output.value)
        } else {
            None
        };
        let node_id = NodeId(as_u16(node)?);
        if let Some(request) = host_cancellation {
            let pending = self
                .pending_host_calls
                .iter()
                .flatten()
                .find(|pending| {
                    pending.request.node == node_id && pending.request.request == request
                })
                .ok_or(SchedulerError::HostCallCancellationRejected)?;
            if !pending.dispatched {
                return Err(SchedulerError::HostCallCancellationUndispatched);
            }
            if pending.completion.is_some() {
                return Err(SchedulerError::HostCallCancellationRejected);
            }
            if pending.cancellation_requested {
                return Err(SchedulerError::HostCallCancellationDuplicate);
            }
        }
        let admitted_host_request = if let Some((request, call, input)) = host_request {
            if self.last_host_request[node].is_some_and(|last| request <= last)
                || self.pending_host_calls.iter().flatten().any(|pending| {
                    pending.request.node == node_id && pending.request.request == request
                })
            {
                return Err(SchedulerError::HostCallRequestDuplicate);
            }
            let pending_for_node = self
                .pending_host_calls
                .iter()
                .flatten()
                .any(|pending| usize::from(pending.request.node.0) == node);
            if pending_for_node && !consumed_host_completion {
                return Err(SchedulerError::InvalidHostCallAccess);
            }
            if !consumed_host_completion && self.pending_host_calls.iter().all(Option::is_some) {
                return Err(SchedulerError::HostCallCapacityExceeded);
            }
            self.values.get(input.value)?;
            let bindings = self
                .host_bindings
                .as_ref()
                .ok_or(SchedulerError::InvalidHostCallAccess)?;
            Some(bindings.admit_request(NodeId(as_u16(node)?), call, input)?)
        } else {
            None
        };
        for (port, value) in outputs.iter().copied().enumerate() {
            let Some(value) = value else {
                continue;
            };
            if available_host_value == Some(value) && !consumed_host_completion {
                return Err(SchedulerError::InvalidHostCallAccess);
            }
            if discards.iter().flatten().any(|discard| *discard == value) {
                return Err(SchedulerError::InvalidPortAccess);
            }
            self.values.get(value)?;
            for target in self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))?
            {
                let cord = usize::from(target.cord.0);
                let state = self.cords.get(cord).ok_or(SchedulerError::InvalidPlan)?;
                let spec = self
                    .cord_specs
                    .get(cord)
                    .ok_or(SchedulerError::InvalidPlan)?;
                if spec.track != AssignedConnectionTrack::Payload {
                    continue;
                }
                if state.len >= spec.item_capacity {
                    return Err(SchedulerError::QueueCapacityExceeded);
                }
                if value.byte_len > spec.maximum_value_bytes {
                    return Err(SchedulerError::SemanticValueBoundExceeded);
                }
                if value.byte_len > spec.byte_capacity.saturating_sub(state.queued_bytes) {
                    return Err(SchedulerError::QueueByteCapacityExceeded);
                }
            }
        }
        let mut handled = [None; PORTS];
        for value in outputs.iter().copied().flatten() {
            if handled.iter().flatten().any(|handled| *handled == value) {
                continue;
            }
            let consumed_references = consumed
                .iter()
                .copied()
                .enumerate()
                .filter(|(port, is_consumed)| {
                    *is_consumed
                        && self.node_specs[node].input_cords[*port]
                            .and_then(|cord| self.peek(usize::from(cord.0)).ok().flatten())
                            == Some(value)
                })
                .count()
                + usize::from(consumed_host_value == Some(value));
            let input_matches = self.node_specs[node]
                .input_cords
                .iter()
                .flatten()
                .filter(|cord| self.peek(usize::from(cord.0)).ok().flatten() == Some(value))
                .count();
            if input_matches > 0 && consumed_references == 0 {
                return Err(SchedulerError::InvalidPortAccess);
            }
            let base_references = consumed_references.max(1);
            let target_references =
                self.step_target_count(node, outputs, host_request, retained_values, value)?;
            let current = usize::from(self.values.reference_count(value)?);
            if current < consumed_references {
                return Err(SchedulerError::Storage(StorageError::StaleReference));
            }
            let additional = target_references.saturating_sub(base_references);
            if current
                .checked_add(additional)
                .filter(|count| *count <= usize::from(u16::MAX))
                .is_none()
            {
                return Err(SchedulerError::Storage(StorageError::ReferenceOverflow));
            }
            let slot = handled
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or(SchedulerError::InvalidPortAccess)?;
            *slot = Some(value);
        }
        self.preflight_host_input(
            node,
            staged,
            retained_values,
            available_host_value,
            consumed_host_value,
        )?;
        for discard in discards.iter().copied().flatten() {
            self.values.get(discard)?;
            if retained_values
                .iter()
                .flatten()
                .any(|retained| *retained == discard)
                || self.node_specs[node]
                    .input_cords
                    .iter()
                    .flatten()
                    .any(|cord| self.peek(usize::from(cord.0)).ok().flatten() == Some(discard))
            {
                return Err(SchedulerError::InvalidPortAccess);
            }
        }
        Ok(admitted_host_request)
    }

    fn commit_event_count(
        &self,
        node: usize,
        consumed: &[bool; PORTS],
        consumed_closed: &[bool; PORTS],
        outputs: &[Option<ValueRef>; PORTS],
    ) -> Result<usize, SchedulerError> {
        let consumed = consumed.iter().filter(|value| **value).count();
        let consumed_closed = consumed_closed.iter().filter(|value| **value).count();
        let mut routed = 0_usize;
        for (port, output) in outputs.iter().enumerate() {
            if output.is_some() {
                routed = routed
                    .checked_add(
                        self.routes
                            .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))?
                            .filter(|target| {
                                self.cord_specs[usize::from(target.cord.0)].track
                                    == AssignedConnectionTrack::Payload
                            })
                            .count(),
                    )
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
        }
        consumed
            .checked_add(consumed_closed)
            .and_then(|value| value.checked_add(routed))
            .ok_or(SchedulerError::InvalidPlan)
    }

    fn output_route_count(&self, node: usize) -> Result<usize, SchedulerError> {
        let mut count = 0_usize;
        for port in 0..PORTS {
            if let Ok(routes) = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))
            {
                count = count
                    .checked_add(routes.count())
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
        }
        Ok(count)
    }

    fn ensure_sign_capacity(&self, additional: usize) -> Result<(), SchedulerError> {
        let additional_items =
            u16::try_from(additional).map_err(|_| SchedulerError::InvalidPlan)?;
        self.signs
            .ensure_capacity(additional_items)
            .map_err(SchedulerError::Sign)
    }

    fn ensure_remote_sign_capacity(&self, additional: usize) -> Result<(), SchedulerError> {
        let additional = u16::try_from(additional).map_err(|_| SchedulerError::InvalidPlan)?;
        self.signs.ensure_remote_capacity(additional)?;
        Ok(())
    }

    fn step_target_count(
        &self,
        node: usize,
        outputs: &[Option<ValueRef>; PORTS],
        host_request: Option<(RequestId, HostCallId, BoundedValueRef)>,
        retained_values: &[Option<ValueRef>; PORTS],
        value: ValueRef,
    ) -> Result<usize, SchedulerError> {
        let mut count = 0_usize;
        for (port, output) in outputs.iter().copied().enumerate() {
            if output != Some(value) {
                continue;
            }
            count = count
                .checked_add(
                    self.routes
                        .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))?
                        .filter(|target| {
                            self.cord_specs[usize::from(target.cord.0)].track
                                == AssignedConnectionTrack::Payload
                        })
                        .count(),
                )
                .ok_or(SchedulerError::InvalidPlan)?;
        }
        if host_request.map(|request| request.2.value) == Some(value) {
            count = count.checked_add(1).ok_or(SchedulerError::InvalidPlan)?;
        }
        count = count
            .checked_add(
                retained_values
                    .iter()
                    .flatten()
                    .filter(|retained| **retained == value)
                    .count(),
            )
            .ok_or(SchedulerError::InvalidPlan)?;
        Ok(count)
    }

    fn close_outputs(&mut self, node: usize) -> Result<(), SchedulerError> {
        let mut remote_closures = 0_usize;
        let mut normal_close_targets = 0_usize;
        for port in 0..PORTS {
            let Ok(targets) = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))
            else {
                continue;
            };
            let targets = targets.collect_targets::<ROUTE_TARGETS>()?;
            remote_closures = remote_closures
                .checked_add(
                    targets
                        .iter()
                        .filter(|target| matches!(target.sink, CordEndpoint::Remote(_)))
                        .count(),
                )
                .ok_or(SchedulerError::InvalidPlan)?;
            for target in targets.iter() {
                let cord = usize::from(target.cord.0);
                let spec = self.cord_specs[cord];
                if spec.track != AssignedConnectionTrack::NormalClose {
                    continue;
                }
                let state = self.cords[cord];
                if state.len >= spec.item_capacity {
                    return Err(SchedulerError::QueueCapacityExceeded);
                }
                normal_close_targets = normal_close_targets
                    .checked_add(1)
                    .ok_or(SchedulerError::InvalidPlan)?;
            }
        }
        self.ensure_remote_sign_capacity(remote_closures)?;
        let normal_close = if normal_close_targets == 0 {
            None
        } else {
            let value = self.values.store(&[])?;
            for _ in 1..normal_close_targets {
                self.values.retain(value)?;
            }
            Some(value)
        };
        for port in 0..PORTS {
            let Ok(targets) = self
                .routes
                .route(NodeId(as_u16(node)?), PortId(as_u16(port)?))
            else {
                continue;
            };
            let targets = targets.collect_targets::<ROUTE_TARGETS>()?;
            for target in targets.iter() {
                let cord = usize::from(target.cord.0);
                if self.cord_specs[cord].track == AssignedConnectionTrack::NormalClose {
                    let value = normal_close.ok_or(SchedulerError::InvalidPlan)?;
                    if self.push(cord, value)?.is_some() {
                        return Err(SchedulerError::InvalidPlan);
                    }
                    self.signs.record(
                        NodeId(as_u16(node)?),
                        Some(PortId(as_u16(port)?)),
                        None,
                        KernelEventKind::ValueRouted,
                    )?;
                    self.signs.observe_debug(DebugRuntimeEvent {
                        node: NodeId(as_u16(node)?),
                        port: Some(PortId(as_u16(port)?)),
                        cord: Some(target.cord),
                        kind: DebugEventKind::ValueSent,
                        type_identity: None,
                        value: Some(&[]),
                        fault_code: None,
                    });
                }
                self.cords[cord].producer_closed = true;
                if let CordEndpoint::Local { node, .. } = target.sink {
                    self.ready[usize::from(node.0)] = true;
                } else {
                    let CordEndpoint::Remote(endpoint) = target.sink else {
                        unreachable!("remote output closure has remote sink")
                    };
                    self.signs.record_remote(
                        NodeId(as_u16(node)?),
                        PortId(as_u16(port)?),
                        KernelEventKind::RemoteOutputClosed,
                        crate::RemoteLifecycleIdentity {
                            endpoint,
                            cord: target.cord,
                            direction: crate::RemoteCordDirection::Egress,
                            sequence: self.cords[cord].next_remote_sequence,
                        },
                    )?;
                }
            }
        }
        Ok(())
    }

    fn abnormally_terminate_outputs(
        &mut self,
        node: usize,
        output: PortId,
        terminal: CanonicalValue,
    ) -> Result<bool, SchedulerError> {
        let mut abnormal_targets = 0_usize;
        let mut remote_closures = 0_usize;
        let Ok(targets) = self.routes.route(NodeId(as_u16(node)?), output) else {
            // A checked terminal output may deliberately end at this semantic
            // boundary. No Cord means the typed abnormal value terminates the
            // Play here; it is not an invalid-port side channel.
            return Ok(false);
        };
        for target in targets {
            let cord = usize::from(target.cord.0);
            let spec = self.cord_specs[cord];
            remote_closures = remote_closures
                .checked_add(usize::from(matches!(target.sink, CordEndpoint::Remote(_))))
                .ok_or(SchedulerError::InvalidPlan)?;
            if spec.track != AssignedConnectionTrack::AbnormalTerminal {
                continue;
            }
            let state = self.cords[cord];
            if state.len >= spec.item_capacity
                || spec.byte_capacity < terminal.as_slice().len() as u32
            {
                return Err(SchedulerError::QueueCapacityExceeded);
            }
            abnormal_targets = abnormal_targets
                .checked_add(1)
                .ok_or(SchedulerError::InvalidPlan)?;
        }
        if abnormal_targets == 0 {
            return Ok(false);
        }
        self.ensure_sign_capacity(abnormal_targets)?;
        self.ensure_remote_sign_capacity(remote_closures)?;
        let bytes = terminal.as_slice();
        let value = self.values.store(bytes)?;
        for _ in 1..abnormal_targets {
            self.values.retain(value)?;
        }
        let targets = self
            .routes
            .route(NodeId(as_u16(node)?), output)
            .map_err(|_| SchedulerError::InvalidPortAccess)?
            .collect_targets::<ROUTE_TARGETS>()?;
        for target in targets.iter() {
            let cord = usize::from(target.cord.0);
            let spec = self.cord_specs[cord];
            if spec.track == AssignedConnectionTrack::AbnormalTerminal {
                if self.push(cord, value)?.is_some() {
                    return Err(SchedulerError::InvalidPlan);
                }
                self.signs.record(
                    NodeId(as_u16(node)?),
                    Some(output),
                    None,
                    KernelEventKind::ValueRouted,
                )?;
                self.signs.observe_debug(DebugRuntimeEvent {
                    node: NodeId(as_u16(node)?),
                    port: Some(output),
                    cord: Some(target.cord),
                    kind: DebugEventKind::ValueSent,
                    type_identity: None,
                    value: Some(bytes),
                    fault_code: None,
                });
            }
            self.cords[cord].producer_closed = true;
            self.cords[cord].producer_abnormal =
                spec.track != AssignedConnectionTrack::AbnormalTerminal;
            self.cords[cord].abnormal_terminal =
                (spec.track != AssignedConnectionTrack::AbnormalTerminal).then_some(terminal);
            if let CordEndpoint::Local { node, .. } = target.sink {
                self.ready[usize::from(node.0)] = true;
            } else {
                let CordEndpoint::Remote(endpoint) = target.sink else {
                    unreachable!("remote output closure has remote sink")
                };
                self.signs.record_remote(
                    NodeId(as_u16(node)?),
                    output,
                    KernelEventKind::RemoteOutputClosed,
                    crate::RemoteLifecycleIdentity {
                        endpoint,
                        cord: target.cord,
                        direction: crate::RemoteCordDirection::Egress,
                        sequence: self.cords[cord].next_remote_sequence,
                    },
                )?;
            }
        }
        if self.unresolved_abnormal[node].is_some() {
            return Err(SchedulerError::InvalidPlan);
        }
        self.unresolved_abnormal[node] = Some(UnresolvedAbnormal {
            port: output,
            terminal,
        });
        Ok(true)
    }

    fn peek(&self, cord: usize) -> Result<Option<ValueRef>, SchedulerError> {
        let spec = *self
            .cord_specs
            .get(cord)
            .ok_or(SchedulerError::InvalidPlan)?;
        let state = *self.cords.get(cord).ok_or(SchedulerError::InvalidPlan)?;
        if state.len == 0 {
            return Ok(None);
        }
        let offset = state.head % spec.item_capacity;
        let slot = usize::from(spec.slot_start + offset);
        self.queue_slots
            .get(slot)
            .copied()
            .ok_or(SchedulerError::InvalidPlan)
    }

    fn pop(&mut self, cord: usize) -> Result<ValueRef, SchedulerError> {
        let spec = self.cord_specs[cord];
        let state = &mut self.cords[cord];
        if state.len == 0 {
            return Err(SchedulerError::InvalidPortAccess);
        }
        let offset = state.head % spec.item_capacity;
        let slot = usize::from(spec.slot_start + offset);
        let value = self.queue_slots[slot]
            .take()
            .ok_or(SchedulerError::InvalidPlan)?;
        state.head = (state.head + 1) % spec.item_capacity;
        state.len -= 1;
        state.queued_bytes -= value.byte_len;
        Ok(value)
    }

    fn push(&mut self, cord: usize, value: ValueRef) -> Result<Option<ValueRef>, SchedulerError> {
        let spec = self.cord_specs[cord];
        let state = &mut self.cords[cord];
        if state.len >= spec.item_capacity {
            if spec.pressure_policy != AssignedPressurePolicy::CoalesceLatest || state.len == 0 {
                return Err(SchedulerError::QueueCapacityExceeded);
            }
            let offset = (state.head + state.len - 1) % spec.item_capacity;
            let slot = usize::from(spec.slot_start + offset);
            let superseded = self.queue_slots[slot]
                .replace(value)
                .ok_or(SchedulerError::InvalidPlan)?;
            let remaining = spec
                .byte_capacity
                .saturating_sub(state.queued_bytes.saturating_sub(superseded.byte_len));
            if value.byte_len > remaining {
                self.queue_slots[slot] = Some(superseded);
                return Err(SchedulerError::QueueByteCapacityExceeded);
            }
            state.queued_bytes = state
                .queued_bytes
                .saturating_sub(superseded.byte_len)
                .saturating_add(value.byte_len);
            return Ok(Some(superseded));
        }
        let offset = (state.head + state.len) % spec.item_capacity;
        let slot = usize::from(spec.slot_start + offset);
        if self.queue_slots[slot].is_some() {
            return Err(SchedulerError::InvalidPlan);
        }
        self.queue_slots[slot] = Some(value);
        state.len += 1;
        state.queued_bytes += value.byte_len;
        Ok(None)
    }

    fn admission_maximum(&self, spec: CordSpec, state: &CordState) -> Result<u32, SchedulerError> {
        if state.len < spec.item_capacity {
            return Ok(spec.byte_capacity.saturating_sub(state.queued_bytes));
        }
        if spec.pressure_policy != AssignedPressurePolicy::CoalesceLatest || state.len == 0 {
            return Ok(0);
        }
        let offset = (state.head + state.len - 1) % spec.item_capacity;
        let slot = usize::from(spec.slot_start + offset);
        let newest = self.queue_slots[slot].ok_or(SchedulerError::InvalidPlan)?;
        Ok(spec
            .byte_capacity
            .saturating_sub(state.queued_bytes.saturating_sub(newest.byte_len)))
    }
}

fn validate_plan<
    const NODES: usize,
    const CORDS: usize,
    const PORTS: usize,
    const QUEUE_SLOTS: usize,
    const ROUTE_SLOTS: usize,
    const ROUTE_TARGETS: usize,
>(
    active_nodes: usize,
    active_cords: usize,
    nodes: &[NodeSpec<PORTS>; NODES],
    cords: &[CordSpec; CORDS],
    routes: &FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
) -> Result<(), SchedulerError> {
    for (node_index, node) in nodes[..active_nodes].iter().enumerate() {
        if node.maximum_step_fuel == 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        for (port, cord) in node.input_cords.iter().copied().enumerate() {
            let Some(cord) = cord else {
                continue;
            };
            let spec = cords
                .get(usize::from(cord.0))
                .filter(|_| usize::from(cord.0) < active_cords)
                .ok_or(SchedulerError::InvalidPlan)?;
            if spec.sink_local() != Some((NodeId(as_u16(node_index)?), PortId(as_u16(port)?))) {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for port in 0..PORTS {
            let Ok(targets) = routes.route(NodeId(as_u16(node_index)?), PortId(as_u16(port)?))
            else {
                continue;
            };
            let mut seen = [false; CORDS];
            for target in targets {
                let cord = usize::from(target.cord.0);
                let spec = cords
                    .get(cord)
                    .filter(|_| cord < active_cords)
                    .ok_or(SchedulerError::InvalidPlan)?;
                if seen[cord]
                    || spec.source_local()
                        != Some((NodeId(as_u16(node_index)?), PortId(as_u16(port)?)))
                    || spec.sink != target.sink
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                seen[cord] = true;
            }
        }
    }
    for (cord_index, cord) in cords[..active_cords].iter().copied().enumerate() {
        if usize::from(cord.cord.0) != cord_index
            || cord.item_capacity == 0
            || cord.byte_capacity == 0
            || cord.maximum_value_bytes == 0
            || cord.maximum_value_bytes > cord.byte_capacity
        {
            return Err(SchedulerError::InvalidPlan);
        }
        match (cord.source, cord.sink) {
            (
                CordEndpoint::Local {
                    node: source_node,
                    port: source_port,
                },
                CordEndpoint::Local {
                    node: sink_node,
                    port: sink_port,
                },
            ) => {
                if usize::from(source_node.0) >= active_nodes
                    || usize::from(sink_node.0) >= active_nodes
                    || usize::from(source_port.0) >= PORTS
                    || usize::from(sink_port.0) >= PORTS
                {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (
                CordEndpoint::Local {
                    node: source_node,
                    port: source_port,
                },
                CordEndpoint::Remote(_),
            ) => {
                if usize::from(source_node.0) >= active_nodes || usize::from(source_port.0) >= PORTS
                {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (
                CordEndpoint::Remote(_),
                CordEndpoint::Local {
                    node: sink_node,
                    port: sink_port,
                },
            ) => {
                if usize::from(sink_node.0) >= active_nodes || usize::from(sink_port.0) >= PORTS {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
            (CordEndpoint::Remote(_), CordEndpoint::Remote(_)) => {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        let end = usize::from(cord.slot_start)
            .checked_add(usize::from(cord.item_capacity))
            .ok_or(SchedulerError::InvalidPlan)?;
        if end > QUEUE_SLOTS {
            return Err(SchedulerError::InvalidPlan);
        }
        if let Some((sink_node, sink_port)) = cord.sink_local() {
            if nodes[usize::from(sink_node.0)].input_cords[usize::from(sink_port.0)]
                != Some(cord.cord)
            {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        if let Some((source_node, source_port)) = cord.source_local() {
            let routed = routes.route(source_node, source_port)?.any(|target| {
                target
                    == RouteTarget {
                        cord: cord.cord,
                        sink: cord.sink,
                    }
            });
            if !routed {
                return Err(SchedulerError::InvalidPlan);
            }
        }
        for other in &cords[..cord_index] {
            if cord
                .remote_endpoint()
                .zip(other.remote_endpoint())
                .is_some_and(|(left, right)| left == right)
            {
                return Err(SchedulerError::InvalidPlan);
            }
            let other_end = usize::from(other.slot_start) + usize::from(other.item_capacity);
            if usize::from(cord.slot_start) < other_end && usize::from(other.slot_start) < end {
                return Err(SchedulerError::InvalidPlan);
            }
        }
    }
    Ok(())
}

fn as_u16(value: usize) -> Result<u16, SchedulerError> {
    u16::try_from(value).map_err(|_| SchedulerError::InvalidPlan)
}

struct TargetBuffer<const CAPACITY: usize> {
    items: [Option<RouteTarget>; CAPACITY],
    len: usize,
}

impl<const CAPACITY: usize> TargetBuffer<CAPACITY> {
    fn iter(&self) -> impl Iterator<Item = RouteTarget> + '_ {
        self.items[..self.len].iter().copied().flatten()
    }
}

trait CollectTargets: Iterator<Item = RouteTarget> + Sized {
    fn collect_targets<const CAPACITY: usize>(
        self,
    ) -> Result<TargetBuffer<CAPACITY>, SchedulerError> {
        let mut buffer = TargetBuffer {
            items: [None; CAPACITY],
            len: 0,
        };
        for target in self {
            if buffer.len >= CAPACITY {
                return Err(SchedulerError::InvalidPlan);
            }
            buffer.items[buffer.len] = Some(target);
            buffer.len += 1;
        }
        Ok(buffer)
    }
}

impl<I: Iterator<Item = RouteTarget>> CollectTargets for I {}

#[cfg(test)]
mod tests;
