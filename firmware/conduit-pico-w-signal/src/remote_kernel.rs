//! Generated remote-ingress execution through the one Conduit kernel.

use conduit_kernel::scheduler::{
    FixedScheduler, OperationDriver, RemoteIngressOutcome, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, SignQuery, Failure, FailureCode, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, Operation, OperationAction, OperationInput,
    PortId, RequestId, ValueStorage,
};
use conduit_signal::{decode_signal_bytes, Signal, SIGNAL_ENCODED_LEN, SIGNAL_ENCODED_LEN_USIZE};
use cyw43::Control;

use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::signal_execution_identity::SignalExecutionIdentity;
#[cfg(not(feature = "wifi-bootstrap"))]
use crate::signal_image::generated_remote_endpoint;
use crate::signal_image::{
    generated_cords, generated_host_bindings, generated_nodes, generated_routes,
    remote_signal_layout, CORDS, HOST_BINDING_SLOTS, NODES, PENDING_REQUESTS, PORTS, QUEUE_SLOTS,
    ROUTE_SLOTS, ROUTE_TARGETS, RUNTIME_SIGN_BYTES, RUNTIME_SIGN_EVENTS,
};
use crate::usb_link::{UsbLinkError, UsbLinkResult};

type SinkScheduler = FixedScheduler<
    OperationDriver<ShowOperation, PORTS>,
    FixedValueStore<QUEUE_SLOTS, SIGNAL_ENCODED_LEN_USIZE>,
    FixedSignLog<RUNTIME_SIGN_EVENTS>,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

struct ShowOperation {
    input_port: PortId,
    present_operation: conduit_kernel::HostOperationId,
    pending: Option<RequestId>,
    presented: usize,
}

impl ShowOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }
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
                    operation: self.present_operation,
                    input: BoundedValueRef::new(value, SIGNAL_ENCODED_LEN)
                        .expect("generated remote Signal is exactly bounded"),
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
            _ => Self::fail(1),
        }
    }
}

pub struct RemoteSignalKernel {
    scheduler: SinkScheduler,
    endpoint: conduit_kernel::RemoteEndpointId,
    cord: conduit_kernel::CordId,
    show_node: conduit_kernel::NodeId,
    present_operation: conduit_kernel::HostOperationId,
    presented: usize,
    closed: bool,
    identity: SignalExecutionIdentity,
}

impl RemoteSignalKernel {
    #[cfg(not(feature = "wifi-bootstrap"))]
    pub fn new(identity: SignalExecutionIdentity) -> UsbLinkResult<Self> {
        let remote = generated_remote_endpoint().ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
        Self::new_for_endpoint(identity, remote.endpoint, remote.cord)
    }

    pub fn new_for_endpoint(
        identity: SignalExecutionIdentity,
        endpoint: conduit_kernel::RemoteEndpointId,
        cord: conduit_kernel::CordId,
    ) -> UsbLinkResult<Self> {
        let layout = remote_signal_layout().ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
        let values = FixedValueStore::<QUEUE_SLOTS, SIGNAL_ENCODED_LEN_USIZE>::new(
            SIGNAL_ENCODED_LEN,
        )
        .map_err(UsbLinkError::Storage)?;
        let sign = FixedSignLog::<RUNTIME_SIGN_EVENTS>::new(RUNTIME_SIGN_BYTES)
            .map_err(UsbLinkError::SignStorage)?;
        let driver = OperationDriver::new(ShowOperation {
            input_port: layout.show_input_port,
            present_operation: layout.present_operation,
            pending: None,
            presented: 0,
        })
        .map_err(UsbLinkError::Kernel)?;
        let scheduler = SinkScheduler::new_with_host_operations(
            generated_nodes(),
            generated_cords(),
            generated_routes(),
            generated_host_bindings(),
            [driver],
            values,
            sign,
        )
        .map_err(UsbLinkError::Kernel)?;
        Ok(Self {
            scheduler,
            endpoint,
            cord,
            show_node: layout.show_node,
            present_operation: layout.present_operation,
            presented: 0,
            closed: false,
            identity,
        })
    }

    pub fn admit(&mut self, sequence: u64, payload: &[u8]) -> UsbLinkResult<RemoteIngressOutcome> {
        self.scheduler
            .admit_remote_input(self.endpoint, self.cord, sequence, payload)
            .map_err(UsbLinkError::Kernel)
    }

    pub async fn present_accepted(
        &mut self,
        expected_sequence: u64,
        control: &mut Control<'_>,
        sign: &mut UsbCdc,
        runtime: &RuntimeTranscriptIdentity,
    ) -> UsbLinkResult<Signal> {
        loop {
            if let Some(request) = self.scheduler.next_host_request() {
                if request.node != self.show_node || request.operation != self.present_operation {
                    return Err(UsbLinkError::InvalidGeneratedEndpoint);
                }
                let signal = decode_signal_bytes(
                    self.scheduler
                        .host_value(request.input.value)
                        .map_err(UsbLinkError::Kernel)?,
                )
                .map_err(|_| UsbLinkError::InvalidSignal)?;
                if signal.sequence != expected_sequence || self.presented as u64 != expected_sequence
                {
                    return Err(UsbLinkError::InvalidSignal);
                }
                let identity = self
                    .identity
                    .presentation(self.presented)
                    .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
                control.gpio_set(0, signal.level).await;
                sign
                    .write_receipt(
                        signal.sequence,
                        signal.level,
                        identity,
                        runtime,
                    )
                    .await?;
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
                    .map_err(UsbLinkError::Kernel)?;
                self.presented += 1;
                return Ok(signal);
            }
            match self.scheduler.step().map_err(UsbLinkError::Kernel)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => return Err(UsbLinkError::KernelIdle),
                SchedulerStatus::Complete => return Err(UsbLinkError::KernelCompletedEarly),
                SchedulerStatus::Cancelled => return Err(UsbLinkError::KernelCancelled),
            }
        }
    }

    pub fn close_and_complete(&mut self, final_sequence: u64) -> UsbLinkResult<()> {
        if final_sequence != self.presented as u64 {
            return Err(UsbLinkError::InvalidSignal);
        }
        self.scheduler
            .close_remote_input(self.endpoint, self.cord)
            .map_err(UsbLinkError::Kernel)?;
        self.closed = true;
        loop {
            match self.scheduler.step().map_err(UsbLinkError::Kernel)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => break,
                SchedulerStatus::Idle => return Err(UsbLinkError::KernelIdle),
                SchedulerStatus::Cancelled => return Err(UsbLinkError::KernelCancelled),
            }
        }
        if !self.closed
            || self.scheduler.values().used_items() != 0
            || self
                .scheduler
                .cord_usage(self.cord)
                .map_err(UsbLinkError::Kernel)?
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
            return Err(UsbLinkError::KernelTerminalInvariant);
        }
        Ok(())
    }

    pub fn cancel(&mut self) -> UsbLinkResult<()> {
        self.scheduler.cancel().map_err(UsbLinkError::Kernel)
    }
}
