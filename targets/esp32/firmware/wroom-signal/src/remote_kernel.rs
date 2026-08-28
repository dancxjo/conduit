//! Generated remote-ingress execution through the shared Conduit scheduler.

use conduit_kernel::scheduler::{
    FixedScheduler, OperationDriver, RemoteIngressOutcome, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, CordId, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    FixedSignLog, FixedValueStore, HostOperationDisposition, HostOperationOutcome, Operation,
    OperationAction, OperationInput, PortId, RemoteEndpointId, RequestId, SignQuery, ValueStorage,
    remote_sign_storage_bytes,
};
use conduit_signal::{SIGNAL_ENCODED_LEN, Signal, decode_signal_bytes};

const PORTS: usize = 16;
const QUEUE_SLOTS: usize = 1;
const RUNTIME_SIGN_EVENTS: usize = 128;
const RUNTIME_SIGN_BYTES: u32 =
    (RUNTIME_SIGN_EVENTS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32;
const REMOTE_SIGN_ITEMS: u16 = 17;

type SinkScheduler = FixedScheduler<
    OperationDriver<ShowOperation, PORTS>,
    FixedValueStore<QUEUE_SLOTS, { SIGNAL_ENCODED_LEN as usize }>,
    FixedSignLog<RUNTIME_SIGN_EVENTS>,
    1,
    1,
    PORTS,
    QUEUE_SLOTS,
    0,
    0,
    1,
    1,
>;

struct ShowOperation {
    input_port: PortId,
    pending: Option<RequestId>,
    presented: usize,
}

impl Operation for ShowOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if port == self.input_port && self.pending.is_none() =>
            {
                let request = RequestId(self.presented as u32);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: conduit_kernel::HostOperationId(0),
                    input: BoundedValueRef::new(value, SIGNAL_ENCODED_LEN)
                        .expect("generated Signal value is exactly bounded"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.presented += 1;
                OperationAction::Await
            }
            OperationInput::Closed { port }
                if port == self.input_port && self.pending.is_none() =>
            {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 1,
            }),
        }
    }
}

pub struct Esp32RemoteSignalKernel {
    scheduler: SinkScheduler,
    presented: usize,
}

impl Esp32RemoteSignalKernel {
    pub fn new() -> Result<Self, &'static str> {
        let values = FixedValueStore::<QUEUE_SLOTS, { SIGNAL_ENCODED_LEN as usize }>::new(
            SIGNAL_ENCODED_LEN,
        )
        .map_err(|_| "kernel-value-storage")?;
        let remote_sign_bytes =
            remote_sign_storage_bytes(REMOTE_SIGN_ITEMS).ok_or("kernel-sign-budget")?;
        let sign = FixedSignLog::<RUNTIME_SIGN_EVENTS>::new_with_remote_storage(
            RUNTIME_SIGN_BYTES,
            REMOTE_SIGN_ITEMS,
            remote_sign_bytes,
        )
        .map_err(|_| "kernel-sign-storage")?;
        let driver = OperationDriver::new(ShowOperation {
            input_port: PortId(0),
            pending: None,
            presented: 0,
        })
        .map_err(|_| "kernel-driver")?;
        let mut routes = FixedRoutes::<0, 0>::new(PORTS as u16);
        routes.seal().map_err(|_| "kernel-routes")?;
        let mut host_bindings = FixedHostOperationBindings::<1>::new(1);
        host_bindings
            .install(
                conduit_kernel::NodeId(0),
                crate::generated::GENERATED_HOST_OPERATIONS[0].1,
            )
            .map_err(|_| "kernel-host-binding")?;
        host_bindings.seal().map_err(|_| "kernel-host-binding")?;
        let scheduler = SinkScheduler::new_with_host_operations(
            crate::generated::GENERATED_NODES,
            crate::generated::GENERATED_CORDS,
            routes,
            host_bindings,
            [driver],
            values,
            sign,
        )
        .map_err(|_| "kernel-scheduler")?;
        Ok(Self {
            scheduler,
            presented: 0,
        })
    }

    pub fn admit(
        &mut self,
        sequence: u64,
        payload: &[u8],
    ) -> Result<RemoteIngressOutcome, &'static str> {
        self.scheduler
            .admit_remote_input(RemoteEndpointId(0), CordId(0), sequence, payload)
            .map_err(|_| "kernel-ingress")
    }

    pub fn present_accepted(&mut self, expected_sequence: u64) -> Result<Signal, &'static str> {
        loop {
            if let Some(request) = self.scheduler.next_host_request() {
                if request.node != conduit_kernel::NodeId(0)
                    || request.operation != conduit_kernel::HostOperationId(0)
                {
                    return Err("kernel-host-request");
                }
                let signal = decode_signal_bytes(
                    self.scheduler
                        .host_value(request.input.value)
                        .map_err(|_| "kernel-host-value")?,
                )
                .map_err(|_| "kernel-signal")?;
                if signal.sequence != expected_sequence
                    || self.presented as u64 != expected_sequence
                {
                    return Err("kernel-sequence");
                }
                esp_println::println!(
                    "CONDUIT_ESP32_PRESENT sequence={} level={}",
                    signal.sequence,
                    signal.level
                );
                self.scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .map_err(|_| "kernel-host-completion")?;
                self.presented += 1;
                return Ok(signal);
            }
            match self.scheduler.step().map_err(|_| "kernel-step")? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => return Err("kernel-idle"),
                SchedulerStatus::Complete => return Err("kernel-completed-early"),
                SchedulerStatus::Cancelled => return Err("kernel-cancelled"),
            }
        }
    }

    pub fn close_and_complete(&mut self, final_sequence: u64) -> Result<(), &'static str> {
        if final_sequence != self.presented as u64 {
            return Err("kernel-final-sequence");
        }
        self.scheduler
            .close_remote_input(RemoteEndpointId(0), CordId(0))
            .map_err(|_| "kernel-close")?;
        loop {
            match self.scheduler.step().map_err(|_| "kernel-step")? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => break,
                SchedulerStatus::Idle => return Err("kernel-idle"),
                SchedulerStatus::Cancelled => return Err("kernel-cancelled"),
            }
        }
        if self.scheduler.values().used_items() != 0
            || self
                .scheduler
                .cord_usage(CordId(0))
                .map_err(|_| "kernel-cord")?
                != (0, 0)
            || !self
                .scheduler
                .signs()
                .contains_kind(conduit_kernel::KernelEventKind::RemoteInputClosed)
            || !self
                .scheduler
                .signs()
                .contains_kind(conduit_kernel::KernelEventKind::OperationCompleted)
        {
            return Err("kernel-terminal-invariant");
        }
        Ok(())
    }
}
