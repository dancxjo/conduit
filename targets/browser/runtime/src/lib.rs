use conduit_core::{
    bind_active_play, bind_presentation, bind_sign, kind_id, ArtifactId, BaseImplementationId,
    BootId, CapabilityId, CapabilityLimits, CapabilityOffer, HostAdvertisement, HostId,
    HostOperationContractId, HostProfileId, ImplementationId, OfferGeneration, PlacementId,
    PlanFragment, PlannerCapabilityOffer, PlannerLimits, PlannerProfileId, PresentationIdentity,
    SignIdentity, PROTOCOL_VERSION,
};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerError, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedSignLog,
    HostedValueStore, NodeId, Operation, OperationAction, OperationInput, PortId, RequestId,
    SignError, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment,
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_planner::{
    default_placements, plan_with_advertised_profile, PlanningOptions, BROWSER_PLANNER_PROFILE,
};
use conduit_signal::{
    decode_signal_bytes, encode_signal, parse_pulse_configuration, pulse_contract_revision,
    pulse_execution_profile, pulse_host_operation_requirements, pulse_outputs,
    pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_inputs, show_resource_requirements,
    signal_profile_catalog, signal_resource_offers, Signal, PULSE_KIND, SHOW_KIND,
    SIGNAL_ENCODED_LEN,
};
use std::{cell::RefCell, collections::BTreeMap};

pub mod browser_pointer;
#[cfg(feature = "creche-surface")]
mod creche;
mod device_base;
mod distributed;
mod distributed_toggle;
#[cfg(feature = "form-runner")]
mod form_runner;
pub mod human_media;
#[cfg_attr(not(feature = "form-runner"), allow(unused_imports, dead_code))]
mod installed_browser;
pub mod membership;
mod membership_abi;
pub mod presentation_nucleus;
#[cfg(any(
    feature = "form-runner",
    feature = "tour-surface",
    feature = "creche-surface"
))]
mod source_interaction;
mod structured_offers;
#[cfg(any(feature = "tour-surface", feature = "creche-surface"))]
mod syntax_projection;
mod text_lab_live;
pub mod text_lab_split;
mod webchat;
mod webrtc_session;

const FRAME_CAPACITY: usize = 4_096;
const MAXIMUM_RECEIPTS: usize = 16;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const ROUTE_SLOTS: usize = 4 * PORTS;
const EFFECT_NONE: i32 = 0;
const EFFECT_WAIT: i32 = 1;
const EFFECT_PRESENT: i32 = 2;
const STATUS_RUNNING: i32 = 0;
const STATUS_COMPLETE: i32 = 1;
const ERROR_NOT_STARTED: i32 = -1;
const ERROR_INVALID_HOST: i32 = -2;
const ERROR_START: i32 = -3;
const ERROR_NO_EFFECT: i32 = -4;
const ERROR_COMPLETION_SIZE: i32 = -5;
const ERROR_COMPLETION_IDENTITY: i32 = -6;
const ERROR_UNSUPPORTED_EFFECT: i32 = -7;
const ERROR_RECEIPT_CAPACITY: i32 = -8;
const ERROR_MALFORMED_FRAME: i32 = -9;
const ERROR_DUPLICATE_COMPLETION: i32 = -10;
const ERROR_CANCELLED: i32 = -11;
const ERROR_SIGN_EXHAUSTED: i32 = -12;
const ERROR_TERMINAL_FAILURE: i32 = -13;
const ERROR_CAPACITY_GROWTH: i32 = -14;
const ERROR_KERNEL: i32 = -15;

type BrowserScheduler = FixedScheduler<
    OperationDriver<SignalOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    2,
    1,
    PORTS,
    4,
    ROUTE_SLOTS,
    1,
    2,
    2,
>;

thread_local! {
    // Every WebAssembly instance owns a distinct linear memory and therefore a distinct pair of
    // gears. JavaScript instantiates this module twice; there is no page-global Rust runtime.
    static SESSION: RefCell<Option<BrowserSession>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; FRAME_CAPACITY]> = const { RefCell::new([0; FRAME_CAPACITY]) };
}

#[derive(Clone, Copy)]
enum PendingEffect {
    Wait {
        request: HostOperationRequest,
    },
    Present {
        request: HostOperationRequest,
        projection: usize,
    },
}

struct PreparedProjection {
    node: NodeId,
    signal: Signal,
    presentation: PresentationIdentity,
    sign: SignIdentity,
}

struct PreparedKernel {
    scheduler: BrowserScheduler,
    pulse_node: NodeId,
    show_node: NodeId,
    signal_values: usize,
    projections: Vec<PreparedProjection>,
    identity: KernelExecutionIdentityMap,
    active_play_id: conduit_core::ActivePlayId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CapacitySeal {
    value: (usize, usize),
    sign: usize,
    driver_values: usize,
    identity: (usize, usize, usize),
    projections: usize,
}

struct BrowserSession {
    scheduler: BrowserScheduler,
    fragment: PlanFragment,
    active_play_id: conduit_core::ActivePlayId,
    identity: KernelExecutionIdentityMap,
    pulse_node: NodeId,
    show_node: NodeId,
    projections: Vec<PreparedProjection>,
    next_projection: usize,
    current: Option<PendingEffect>,
    output: [u8; FRAME_CAPACITY],
    output_len: usize,
    expected_completion: [u8; FRAME_CAPACITY],
    expected_completion_len: usize,
    last_completion: [u8; FRAME_CAPACITY],
    last_completion_len: usize,
    receipts: usize,
    complete: bool,
    terminal_failure: bool,
    error: i32,
    seal: CapacitySeal,
}

enum SignalOperation {
    Pulse {
        values: Vec<ValueRef>,
        waits: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
    Show {
        expected: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
}

impl SignalOperation {
    fn pulse(values: Vec<ValueRef>, waits: Vec<ValueRef>) -> Self {
        Self::Pulse {
            values,
            waits,
            next: 0,
            pending: None,
        }
    }

    fn show(expected: Vec<ValueRef>) -> Self {
        Self::Show {
            expected,
            next: 0,
            pending: None,
        }
    }

    fn fail(code: FailureCode, detail: u16) -> OperationAction {
        OperationAction::Fail(Failure { code, detail })
    }

    fn allocation_capacity(&self) -> usize {
        match self {
            Self::Pulse { values, waits, .. } => values.capacity() + waits.capacity(),
            Self::Show { expected, .. } => expected.capacity(),
        }
    }
}

impl Operation for SignalOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Pulse { values, .. } => {
                values
                    .first()
                    .copied()
                    .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    })
            }
            Self::Show { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Pulse {
                    values,
                    next,
                    pending,
                    ..
                },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                values.get(*next).copied().map_or_else(
                    || Self::fail(FailureCode::InvalidLifecycle, 1),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            (
                Self::Show {
                    expected,
                    next,
                    pending,
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if pending.is_none() && expected.get(*next) == Some(&value) => {
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(FailureCode::InvalidLifecycle, 2);
                };
                let request = RequestId(0x8000_0000 | sequence);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, SIGNAL_ENCODED_LEN)
                        .expect("sealed signal value is exactly admitted"),
                }
            }
            (
                Self::Show { next, pending, .. },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                *next += 1;
                OperationAction::Await
            }
            (
                Self::Show {
                    expected,
                    next,
                    pending,
                },
                OperationInput::Closed { port: PortId(0) },
            ) if pending.is_none() && *next == expected.len() => OperationAction::Complete,
            (Self::Pulse { .. }, _) => Self::fail(FailureCode::InvalidLifecycle, 3),
            (Self::Show { .. }, _) => Self::fail(FailureCode::InvalidInput, 4),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Pulse {
                values,
                waits,
                next,
                pending,
            } => {
                *next += 1;
                if *next >= values.len() {
                    return OperationAction::Complete;
                }
                let Some(wait) = waits.get(*next - 1).copied() else {
                    return Self::fail(FailureCode::InvalidLifecycle, 5);
                };
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(FailureCode::InvalidLifecycle, 6);
                };
                let request = RequestId(sequence);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(wait, 8)
                        .expect("sealed wait value is exactly admitted"),
                }
            }
            Self::Show { .. } => OperationAction::Await,
        }
    }
}

impl BrowserSession {
    fn start(host_index: u32) -> Result<Self, i32> {
        Self::start_with_sign_limit(host_index, None)
    }

    fn start_with_sign_limit(
        host_index: u32,
        sign_item_override: Option<u16>,
    ) -> Result<Self, i32> {
        let (host_id, boot_id) = match host_index {
            0 => ("browser-host-a", "browser-boot-a"),
            1 => ("browser-host-b", "browser-boot-b"),
            _ => return Err(ERROR_INVALID_HOST),
        };
        let advertisement = build_advertisement(host_id, boot_id);
        let form = conduit_form::parse_with_startup(
            include_str!("../../../../proof/fixtures/forms/signal-demo.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .map_err(|_| ERROR_START)?;
        let hosts = [advertisement.clone()];
        let placements = default_placements(&form, &hosts).map_err(|_| ERROR_START)?;
        let base_overrides = BTreeMap::new();
        let mut planned = plan_with_advertised_profile(
            &advertisement,
            &PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
            &form,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &base_overrides,
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
                connection_byte_capacity: conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|_| ERROR_START)?;
        let fragment = planned.fragments.pop().ok_or(ERROR_START)?;
        let lowered = lower_plan_fragment(&fragment).map_err(|_| ERROR_START)?;
        let PreparedKernel {
            scheduler,
            pulse_node,
            show_node,
            signal_values,
            projections,
            identity,
            active_play_id,
        } = prepare_kernel(&advertisement, &fragment, &lowered, sign_item_override)?;
        let seal = CapacitySeal {
            value: scheduler.values().allocation_capacities(),
            sign: scheduler.signs().allocation_capacity(),
            driver_values: scheduler
                .drivers()
                .iter()
                .map(|driver| driver.operation().allocation_capacity())
                .sum(),
            identity: identity.allocation_capacities(),
            projections: projections.capacity(),
        };
        if signal_values != MAXIMUM_RECEIPTS || projections.len() != MAXIMUM_RECEIPTS {
            return Err(ERROR_START);
        }
        let mut session = Self {
            scheduler,
            fragment,
            active_play_id,
            identity,
            pulse_node,
            show_node,
            projections,
            next_projection: 0,
            current: None,
            output: [0; FRAME_CAPACITY],
            output_len: 0,
            expected_completion: [0; FRAME_CAPACITY],
            expected_completion_len: 0,
            last_completion: [0; FRAME_CAPACITY],
            last_completion_len: 0,
            receipts: 0,
            complete: false,
            terminal_failure: false,
            error: STATUS_RUNNING,
            seal,
        };
        session.advance()?;
        Ok(session)
    }

    fn capacity_seal(&self) -> CapacitySeal {
        CapacitySeal {
            value: self.scheduler.values().allocation_capacities(),
            sign: self.scheduler.signs().allocation_capacity(),
            driver_values: self
                .scheduler
                .drivers()
                .iter()
                .map(|driver| driver.operation().allocation_capacity())
                .sum(),
            identity: self.identity.allocation_capacities(),
            projections: self.projections.capacity(),
        }
    }

    fn require_stable_capacity(&mut self) -> Result<(), i32> {
        if self.capacity_seal() != self.seal {
            self.error = ERROR_CAPACITY_GROWTH;
            self.terminal_failure = true;
            return Err(ERROR_CAPACITY_GROWTH);
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<(), i32> {
        self.require_stable_capacity()?;
        self.output_len = 0;
        self.expected_completion_len = 0;
        self.current = None;
        loop {
            if let Some(request) = self.scheduler.next_host_request() {
                if let Err(code) = self.prepare_effect(request) {
                    return self.fail(code);
                }
                self.require_stable_capacity()?;
                return Ok(());
            }
            let status = match self.scheduler.step() {
                Ok(status) => status,
                Err(error) => return self.fail(map_scheduler_error(error)),
            };
            match status {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => {
                    if self.receipts != MAXIMUM_RECEIPTS
                        || self.next_projection != MAXIMUM_RECEIPTS
                        || self.scheduler.values().used_items() != 0
                    {
                        return self.fail(ERROR_TERMINAL_FAILURE);
                    }
                    self.complete = true;
                    self.require_stable_capacity()?;
                    return Ok(());
                }
                SchedulerStatus::Idle => return self.fail(ERROR_NO_EFFECT),
                SchedulerStatus::Cancelled => return self.fail(ERROR_TERMINAL_FAILURE),
            }
        }
    }

    fn prepare_effect(&mut self, request: HostOperationRequest) -> Result<(), i32> {
        let request_identity = self
            .identity
            .request(request.node, request.request)
            .ok_or(ERROR_COMPLETION_IDENTITY)?;
        if request_identity.operation != request.operation {
            return self.fail(ERROR_COMPLETION_IDENTITY);
        }
        let placement = self
            .fragment
            .placements
            .get(usize::from(request.node.0))
            .ok_or(ERROR_COMPLETION_IDENTITY)?;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(map_scheduler_error)?;
        let mut output = FrameWriter::new(&mut self.output);
        let mut expected = FrameWriter::new(&mut self.expected_completion);
        if request.node == self.pulse_node {
            let duration = input
                .try_into()
                .map(u64::from_le_bytes)
                .map_err(|_| ERROR_MALFORMED_FRAME)?;
            write_common_frame(
                &mut output,
                EFFECT_WAIT as u8,
                &self.fragment,
                &self.active_play_id,
                request,
                &request_identity.contract_id,
                &placement.placement_id,
            )?;
            output.u64(duration)?;
            write_common_frame(
                &mut expected,
                EFFECT_WAIT as u8,
                &self.fragment,
                &self.active_play_id,
                request,
                &request_identity.contract_id,
                &placement.placement_id,
            )?;
            self.current = Some(PendingEffect::Wait { request });
        } else if request.node == self.show_node {
            let projection = self
                .projections
                .get(self.next_projection)
                .ok_or(ERROR_RECEIPT_CAPACITY)?;
            let signal = decode_signal_bytes(input).map_err(|_| ERROR_MALFORMED_FRAME)?;
            if projection.node != request.node || projection.signal != signal {
                return self.fail(ERROR_COMPLETION_IDENTITY);
            }
            write_common_frame(
                &mut output,
                EFFECT_PRESENT as u8,
                &self.fragment,
                &self.active_play_id,
                request,
                &request_identity.contract_id,
                &placement.placement_id,
            )?;
            write_presentation_frame(&mut output, projection, input)?;
            write_common_frame(
                &mut expected,
                EFFECT_PRESENT as u8,
                &self.fragment,
                &self.active_play_id,
                request,
                &request_identity.contract_id,
                &placement.placement_id,
            )?;
            write_presentation_completion_frame(&mut expected, projection, input)?;
            self.current = Some(PendingEffect::Present {
                request,
                projection: self.next_projection,
            });
        } else {
            return self.fail(ERROR_UNSUPPORTED_EFFECT);
        }
        self.output_len = output.len();
        self.expected_completion_len = expected.len();
        Ok(())
    }

    fn complete_current(&mut self, completion: &[u8]) -> Result<(), i32> {
        if self.error < 0 {
            return Err(self.error);
        }
        if completion.len() == self.last_completion_len
            && self.last_completion_len != 0
            && completion == &self.last_completion[..self.last_completion_len]
        {
            self.error = ERROR_DUPLICATE_COMPLETION;
            self.terminal_failure = true;
            return Err(ERROR_DUPLICATE_COMPLETION);
        }
        let current = self.current.ok_or(ERROR_NO_EFFECT)?;
        if completion.len() != self.expected_completion_len + 1 {
            return self.fail(ERROR_MALFORMED_FRAME);
        }
        if completion[..self.expected_completion_len]
            != self.expected_completion[..self.expected_completion_len]
        {
            return self.fail(ERROR_COMPLETION_IDENTITY);
        }
        let success = match completion[self.expected_completion_len] {
            0 => false,
            1 => true,
            _ => return self.fail(ERROR_MALFORMED_FRAME),
        };
        self.last_completion[..completion.len()].copy_from_slice(completion);
        self.last_completion_len = completion.len();
        self.current = None;
        self.output_len = 0;
        self.expected_completion_len = 0;
        let request = match current {
            PendingEffect::Wait { request } | PendingEffect::Present { request, .. } => request,
        };
        let outcome = if success {
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            }
        } else {
            HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(Failure {
                    code: FailureCode::HostOperationFailed,
                    detail: 1,
                }),
            }
        };
        if let Err(error) =
            self.scheduler
                .complete_host_operation(request.node, request.request, outcome)
        {
            return self.fail(map_scheduler_error(error));
        }
        if let PendingEffect::Present { projection, .. } = current {
            if success {
                if projection != self.next_projection || self.receipts >= MAXIMUM_RECEIPTS {
                    return self.fail(ERROR_RECEIPT_CAPACITY);
                }
                self.receipts += 1;
                self.next_projection += 1;
            }
        }
        if !success {
            // Rust owns terminal truth: advance the kernel through the failed correlated host
            // completion and expose a terminal failure status rather than letting JS decide.
            loop {
                match self.scheduler.step() {
                    Ok(SchedulerStatus::Progress { .. }) => continue,
                    Ok(SchedulerStatus::Cancelled) | Err(SchedulerError::OperationFailed(_)) => {
                        return self.fail(ERROR_TERMINAL_FAILURE)
                    }
                    Ok(SchedulerStatus::Complete | SchedulerStatus::Idle) | Err(_) => {
                        return self.fail(ERROR_TERMINAL_FAILURE)
                    }
                }
            }
        }
        self.advance()
    }

    fn cancel(&mut self) -> Result<(), i32> {
        if self.complete || self.error < 0 {
            return Err(self.status());
        }
        self.scheduler.cancel().map_err(map_scheduler_error)?;
        self.current = None;
        self.output_len = 0;
        self.expected_completion_len = 0;
        self.error = ERROR_CANCELLED;
        self.terminal_failure = true;
        self.require_stable_capacity()?;
        Err(ERROR_CANCELLED)
    }

    fn fail<T>(&mut self, code: i32) -> Result<T, i32> {
        self.error = code;
        self.terminal_failure = true;
        Err(code)
    }

    fn status(&self) -> i32 {
        if self.error < 0 {
            self.error
        } else if self.complete && self.current.is_none() {
            STATUS_COMPLETE
        } else {
            STATUS_RUNNING
        }
    }

    fn effect_kind(&self) -> i32 {
        match self.current {
            Some(PendingEffect::Wait { .. }) => EFFECT_WAIT,
            Some(PendingEffect::Present { .. }) => EFFECT_PRESENT,
            None => EFFECT_NONE,
        }
    }
}

fn prepare_kernel(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    sign_item_override: Option<u16>,
) -> Result<PreparedKernel, i32> {
    if lowered.nodes.len() != 2
        || lowered.cords.len() != 1
        || lowered.cord_value_slots != 4
        || lowered
            .routes
            .iter()
            .map(|route| route.targets.len())
            .sum::<usize>()
            != 1
        || lowered.host_operations.len() != 2
    {
        return Err(ERROR_START);
    }
    let pulse_node = lowered
        .nodes
        .iter()
        .find(|node| {
            fragment.placements[usize::from(node.node.0)]
                .kind_id
                .as_str()
                == PULSE_KIND
        })
        .map(|node| node.node)
        .ok_or(ERROR_START)?;
    let show_node = lowered
        .nodes
        .iter()
        .find(|node| {
            fragment.placements[usize::from(node.node.0)]
                .kind_id
                .as_str()
                == SHOW_KIND
        })
        .map(|node| node.node)
        .ok_or(ERROR_START)?;
    if pulse_node == show_node {
        return Err(ERROR_START);
    }
    let configuration =
        parse_pulse_configuration(&fragment.placements[usize::from(pulse_node.0)].configuration)
            .map_err(|_| ERROR_START)?;
    let count = usize::try_from(configuration.count).map_err(|_| ERROR_START)?;
    let wait_count = count.saturating_sub(1);
    let item_capacity =
        u16::try_from(count.saturating_add(wait_count).max(1)).map_err(|_| ERROR_START)?;
    let byte_capacity = configuration
        .count
        .checked_mul(u64::from(SIGNAL_ENCODED_LEN))
        .and_then(|bytes| bytes.checked_add(u64::try_from(wait_count).ok()?.checked_mul(8)?))
        .and_then(|bytes| u32::try_from(bytes.max(1)).ok())
        .ok_or(ERROR_START)?;
    let mut values = HostedValueStore::new(item_capacity, SIGNAL_ENCODED_LEN, byte_capacity)
        .map_err(|_| ERROR_START)?;
    let mut signal_values = Vec::with_capacity(count);
    for sequence in 0..configuration.count {
        let payload = encode_signal(&Signal {
            sequence,
            level: if sequence.is_multiple_of(2) {
                configuration.initial_level
            } else {
                !configuration.initial_level
            },
        });
        signal_values.push(values.store(&payload.encoded).map_err(|_| ERROR_START)?);
    }
    let mut wait_values = Vec::with_capacity(wait_count);
    for _ in 0..wait_count {
        wait_values.push(
            values
                .store(&configuration.period_ms.to_le_bytes())
                .map_err(|_| ERROR_START)?,
        );
    }
    let mut routes = FixedRoutes::<ROUTE_SLOTS, 1>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| ERROR_START)?;
    }
    routes.seal().map_err(|_| ERROR_START)?;
    let mut host_bindings = FixedHostOperationBindings::<2>::new(1);
    for operation in &lowered.host_operations {
        host_bindings
            .install(operation.node, operation.binding)
            .map_err(|_| ERROR_START)?;
    }
    host_bindings.seal().map_err(|_| ERROR_START)?;
    let mut operations: [Option<SignalOperation>; 2] = core::array::from_fn(|_| None);
    operations[usize::from(pulse_node.0)] =
        Some(SignalOperation::pulse(signal_values.clone(), wait_values));
    operations[usize::from(show_node.0)] = Some(SignalOperation::show(signal_values.clone()));
    let drivers: [OperationDriver<SignalOperation, PORTS>; 2] = operations
        .map(|operation| {
            operation
                .ok_or(ERROR_START)
                .and_then(|operation| OperationDriver::new(operation).map_err(|_| ERROR_START))
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| ERROR_START)?;
    let sign_items = sign_item_override.unwrap_or_else(|| {
        let per_signal = 18_u16;
        u16::try_from(count)
            .unwrap_or(u16::MAX)
            .saturating_mul(per_signal)
            .saturating_add(64)
    });
    let sign_bytes = u32::from(sign_items)
        .checked_mul(u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>()).unwrap())
        .ok_or(ERROR_START)?;
    let sign = HostedSignLog::new(sign_items, sign_bytes.max(1)).map_err(|_| ERROR_START)?;
    let node_specs = lowered
        .node_specs
        .clone()
        .try_into()
        .map_err(|_| ERROR_START)?;
    let cord_specs = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| ERROR_START)?;
    let scheduler = BrowserScheduler::new_with_host_operations(
        node_specs,
        cord_specs,
        routes,
        host_bindings,
        drivers,
        values,
        sign,
    )
    .map_err(|_| ERROR_START)?;

    let active_play = bind_active_play(
        &fragment.plan_id,
        &advertisement.host_id,
        &advertisement.boot_id,
        0,
    );
    let request_capacity = count.checked_add(wait_count).ok_or(ERROR_START)?;
    let sign_capacity = count.checked_add(1).ok_or(ERROR_START)?;
    let mut identity = KernelExecutionIdentityMap::new(
        &lowered.identity,
        &active_play,
        request_capacity,
        count,
        sign_capacity,
    )
    .map_err(|_| ERROR_START)?;
    for sequence in 1..count {
        identity
            .bind_request(
                &lowered.identity,
                pulse_node,
                RequestId(u32::try_from(sequence).map_err(|_| ERROR_START)?),
                HostOperationId(0),
            )
            .map_err(|_| ERROR_START)?;
    }
    let show_placement = &fragment.placements[usize::from(show_node.0)];
    let mut projections = Vec::with_capacity(count);
    for sequence in 0..count {
        let request = RequestId(0x8000_0000 | u32::try_from(sequence).map_err(|_| ERROR_START)?);
        identity
            .bind_request(&lowered.identity, show_node, request, HostOperationId(0))
            .map_err(|_| ERROR_START)?;
        let sequence = u64::try_from(sequence).map_err(|_| ERROR_START)?;
        let signal = Signal {
            sequence,
            level: if sequence.is_multiple_of(2) {
                configuration.initial_level
            } else {
                !configuration.initial_level
            },
        };
        let presentation = bind_presentation(
            &active_play.active_play_id,
            &show_placement.placement_id,
            sequence,
        );
        let sign = bind_sign(
            &advertisement.host_id,
            &advertisement.boot_id,
            Some(&active_play.active_play_id),
            sequence,
        );
        identity
            .bind_presentation(&lowered.identity, show_node, request, &presentation)
            .map_err(|_| ERROR_START)?;
        identity
            .bind_sign(
                &sign,
                Some(show_node),
                Some(request),
                Some(&presentation.presentation_id),
            )
            .map_err(|_| ERROR_START)?;
        projections.push(PreparedProjection {
            node: show_node,
            signal,
            presentation,
            sign,
        });
    }
    let terminal = bind_sign(
        &advertisement.host_id,
        &advertisement.boot_id,
        Some(&active_play.active_play_id),
        u64::try_from(count).map_err(|_| ERROR_START)?,
    );
    identity
        .bind_sign(&terminal, None, None, None)
        .map_err(|_| ERROR_START)?;
    if identity.lengths() != (request_capacity, count, sign_capacity) {
        return Err(ERROR_START);
    }
    Ok(PreparedKernel {
        scheduler,
        pulse_node,
        show_node,
        signal_values: count,
        projections,
        identity,
        active_play_id: active_play.active_play_id,
    })
}

fn write_common_frame(
    writer: &mut FrameWriter<'_>,
    kind: u8,
    fragment: &PlanFragment,
    active_play_id: &conduit_core::ActivePlayId,
    request: HostOperationRequest,
    contract_id: &HostOperationContractId,
    placement_id: &PlacementId,
) -> Result<(), i32> {
    writer.byte(kind)?;
    writer.text(fragment.source_document_id.as_str())?;
    writer.text(fragment.checked_form_id.as_str())?;
    writer.text(fragment.expanded_form_id.as_str())?;
    writer.text(fragment.plan_id.as_str())?;
    writer.text(fragment.fragment_id.as_str())?;
    writer.text(fragment.host_id.as_str())?;
    writer.text(fragment.boot_id.as_str())?;
    writer.text(active_play_id.as_str())?;
    writer.u16(request.node.0)?;
    writer.u32(request.request.0)?;
    writer.u16(request.operation.0)?;
    writer.text(contract_id.as_str())?;
    writer.text(placement_id.as_str())
}

fn write_presentation_frame(
    writer: &mut FrameWriter<'_>,
    projection: &PreparedProjection,
    input: &[u8],
) -> Result<(), i32> {
    write_typed_presentation_frame(
        writer,
        projection,
        "presentation/signal",
        "value/signal",
        input,
    )
}

fn write_typed_presentation_frame(
    writer: &mut FrameWriter<'_>,
    projection: &PreparedProjection,
    presentation_kind: &str,
    value_kind: &str,
    input: &[u8],
) -> Result<(), i32> {
    writer.text(projection.presentation.presentation_id.as_str())?;
    writer.text(projection.sign.sign_id.as_str())?;
    writer.text(presentation_kind)?;
    writer.text(value_kind)?;
    writer.bytes(input)
}

fn write_presentation_completion_frame(
    writer: &mut FrameWriter<'_>,
    projection: &PreparedProjection,
    input: &[u8],
) -> Result<(), i32> {
    write_typed_presentation_completion_frame(writer, projection, "value/signal", input)
}

fn write_typed_presentation_completion_frame(
    writer: &mut FrameWriter<'_>,
    projection: &PreparedProjection,
    value_kind: &str,
    input: &[u8],
) -> Result<(), i32> {
    writer.text(projection.presentation.presentation_id.as_str())?;
    writer.text(projection.sign.sign_id.as_str())?;
    writer.text(value_kind)?;
    writer.bytes(input)
}

fn map_scheduler_error(error: SchedulerError) -> i32 {
    match error {
        SchedulerError::Sign(SignError::ItemCapacityExceeded | SignError::ByteCapacityExceeded) => {
            ERROR_SIGN_EXHAUSTED
        }
        SchedulerError::OperationFailed(_) | SchedulerError::Cancelled => ERROR_TERMINAL_FAILURE,
        _ => ERROR_KERNEL,
    }
}

struct FrameWriter<'a> {
    target: &'a mut [u8],
    offset: usize,
}

impl<'a> FrameWriter<'a> {
    fn new(target: &'a mut [u8]) -> Self {
        Self { target, offset: 0 }
    }

    fn len(&self) -> usize {
        self.offset
    }

    fn byte(&mut self, value: u8) -> Result<(), i32> {
        self.write(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), i32> {
        self.write(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), i32> {
        self.write(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), i32> {
        self.write(&value.to_le_bytes())
    }

    fn text(&mut self, value: &str) -> Result<(), i32> {
        self.bytes(value.as_bytes())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), i32> {
        let length = u16::try_from(value.len()).map_err(|_| ERROR_COMPLETION_SIZE)?;
        self.write(&length.to_le_bytes())?;
        self.write(value)
    }

    fn write(&mut self, value: &[u8]) -> Result<(), i32> {
        let end = self
            .offset
            .checked_add(value.len())
            .filter(|end| *end <= self.target.len())
            .ok_or(ERROR_COMPLETION_SIZE)?;
        self.target[self.offset..end].copy_from_slice(value);
        self.offset = end;
        Ok(())
    }
}

fn build_advertisement(host_id: &str, boot_id: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser-wasm-kernel"),
        resources: signal_resource_offers("browser/timer", "browser/dom", 16),
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
            limits: PlannerLimits {
                maximum_host_advertisements: 16,
                maximum_gears: 64,
                maximum_connections: 128,
                maximum_authority_grants: 64,
                maximum_protected_resource_grants: 64,
                maximum_line_offers: 128,
            },
        }],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: conduit_signal::pulse_face_startup_parameters(),
                shorthand: None,
                capability_id: CapabilityId::from("pulse-1"),
                kind_id: kind_id(PULSE_KIND),
                kind_contract_revision: pulse_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: pulse_execution_profile(),
                    implementation_id: ImplementationId::from("browser/kernel-pulse-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
                },
                inputs: vec![],
                outputs: pulse_outputs(),
                host_operations: pulse_host_operation_requirements(),
                resource_requirements: pulse_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("dom-show-1"),
                kind_id: kind_id(SHOW_KIND),
                kind_contract_revision: show_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from("browser/kernel-dom-show-signal-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
                },
                inputs: show_inputs(),
                outputs: vec![],
                host_operations: show_host_operation_requirements(),
                resource_requirements: show_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_start(host_index: u32) -> i32 {
    match BrowserSession::start(host_index) {
        Ok(session) => {
            SESSION.with(|slot| *slot.borrow_mut() = Some(session));
            STATUS_RUNNING
        }
        Err(code) => code,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_status() -> i32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(BrowserSession::status)
            .unwrap_or(ERROR_NOT_STARTED)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_effect_kind() -> i32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(BrowserSession::effect_kind)
            .unwrap_or(ERROR_NOT_STARTED)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_output_ptr() -> *const u8 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.output.as_ptr())
            .unwrap_or(std::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_output_len() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.output_len as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_input_capacity() -> u32 {
    FRAME_CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_complete(completion_len: u32) -> i32 {
    let completion_len = completion_len as usize;
    if completion_len > FRAME_CAPACITY {
        return ERROR_COMPLETION_SIZE;
    }
    INPUT.with(|input| {
        SESSION.with(|slot| {
            let input = input.borrow();
            let mut slot = slot.borrow_mut();
            let Some(session) = slot.as_mut() else {
                return ERROR_NOT_STARTED;
            };
            match session.complete_current(&input[..completion_len]) {
                Ok(()) => session.status(),
                Err(code) => code,
            }
        })
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_cancel() -> i32 {
    SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(session) = slot.as_mut() else {
            return ERROR_NOT_STARTED;
        };
        match session.cancel() {
            Ok(()) => session.status(),
            Err(code) => code,
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_receipt_count() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.receipts as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_capacity_stable() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| u32::from(session.capacity_seal() == session.seal))
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_terminal_failure() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| u32::from(session.terminal_failure))
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion(session: &BrowserSession, success: bool) -> Vec<u8> {
        let mut frame = session.expected_completion[..session.expected_completion_len].to_vec();
        frame.push(u8::from(success));
        frame
    }

    #[test]
    fn exact_kernel_completion_advances_and_wrong_identity_fails_closed() {
        let mut session = BrowserSession::start(0).expect("browser kernel session starts");
        while session.effect_kind() != EFFECT_PRESENT {
            let exact = completion(&session, true);
            session
                .complete_current(&exact)
                .expect("timer completion advances");
        }
        assert_eq!(session.effect_kind(), EFFECT_PRESENT);
        let exact = completion(&session, true);
        session
            .complete_current(&exact)
            .expect("exact frame advances");
        assert_eq!(session.receipts, 1);
        let mut changed = completion(&session, true);
        changed[1] ^= 1;
        assert_eq!(
            session.complete_current(&changed),
            Err(ERROR_COMPLETION_IDENTITY)
        );
        assert!(session.terminal_failure);
    }

    #[test]
    fn duplicate_completion_is_rejected_by_rust_runtime() {
        let mut session = BrowserSession::start(0).expect("browser kernel session starts");
        let exact = completion(&session, true);
        session
            .complete_current(&exact)
            .expect("first completion accepted");
        assert_eq!(
            session.complete_current(&exact),
            Err(ERROR_DUPLICATE_COMPLETION)
        );
    }

    #[test]
    fn cancellation_and_platform_failure_are_honest_terminal_states() {
        let mut cancelled = BrowserSession::start(0).expect("browser kernel session starts");
        assert_eq!(cancelled.cancel(), Err(ERROR_CANCELLED));
        assert_eq!(cancelled.status(), ERROR_CANCELLED);
        assert!(cancelled.terminal_failure);

        let mut failed = BrowserSession::start(1).expect("browser kernel session starts");
        let failure = completion(&failed, false);
        assert_eq!(
            failed.complete_current(&failure),
            Err(ERROR_TERMINAL_FAILURE)
        );
        assert_eq!(failed.status(), ERROR_TERMINAL_FAILURE);
        assert!(failed.terminal_failure);
    }

    #[test]
    fn sign_exhaustion_is_a_distinct_terminal_failure() {
        assert_eq!(
            BrowserSession::start_with_sign_limit(0, Some(1)).err(),
            Some(ERROR_SIGN_EXHAUSTED)
        );
    }

    #[test]
    fn capacities_are_sealed_before_play_start_and_never_grow() {
        let mut session = BrowserSession::start(0).expect("browser kernel session starts");
        while session.status() == STATUS_RUNNING {
            assert_eq!(session.capacity_seal(), session.seal);
            let exact = completion(&session, true);
            session
                .complete_current(&exact)
                .expect("completion advances");
        }
        assert_eq!(session.status(), STATUS_COMPLETE);
        assert_eq!(session.receipts, MAXIMUM_RECEIPTS);
        assert_eq!(session.capacity_seal(), session.seal);
    }

    #[test]
    fn exact_browser_fragment_uses_the_planned_item_and_byte_bounded_cord() {
        let session = BrowserSession::start(0).expect("browser kernel session starts");
        let lowered = lower_plan_fragment(&session.fragment).expect("exact fragment lowers");
        assert_eq!(lowered.cord_value_slots, 4);
        assert_eq!(lowered.cord_value_bytes, 64);
        assert_eq!(lowered.cords.len(), 1);
        assert_eq!(lowered.cords[0].spec.item_capacity, 4);
        assert_eq!(lowered.cords[0].spec.byte_capacity, 64);
        assert_eq!(session.scheduler.drivers().len(), 2);
        assert!(session.output_len <= FRAME_CAPACITY);
        assert!(session.expected_completion_len < FRAME_CAPACITY);
    }

    #[test]
    fn host_identity_is_bounded_to_the_two_page_instances() {
        let first = BrowserSession::start(0).expect("first browser host starts");
        let second = BrowserSession::start(1).expect("second browser host starts");
        assert_ne!(first.fragment.host_id, second.fragment.host_id);
        assert_ne!(first.fragment.boot_id, second.fragment.boot_id);
        assert_ne!(first.active_play_id, second.active_play_id);
        assert!(matches!(BrowserSession::start(2), Err(ERROR_INVALID_HOST)));
    }
}
