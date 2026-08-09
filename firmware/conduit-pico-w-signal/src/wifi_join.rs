//! Ordinary generated network/join execution over the existing UsbCdc session.

use conduit_kernel::scheduler::{
    FixedScheduler, OperationDriver, RemoteIngressOutcome, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, FixedClueLog, FixedValueStore, HostOperationDisposition,
    HostOperationOutcome,
};
use conduit_wire::{
    SessionMachine, SessionMessage, SessionRole,
};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_24, PIN_25, PIN_29, PIO0};
use embassy_rp::Peri;
use embassy_time::{with_timeout, Duration};
use heapless::String as HString;
use static_cell::StaticCell;

use crate::network_image::{
    generated_cords, generated_host_bindings, generated_nodes, generated_remote_endpoint,
    generated_routes, network_join_layout, CORDS, HOST_BINDING_SLOTS, NODES, PENDING_REQUESTS,
    PORTS, QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS, RUNTIME_CLUE_BYTES, RUNTIME_CLUE_EVENTS,
};
use crate::network_receipts::NetworkAttachmentIdentity;
use crate::network_operations::NetworkOperation;
use crate::receipts::{RuntimeTranscriptIdentity, UsbCdc};
use crate::usb::PicoUsbCdcCarrier;
use crate::usb_link::{UsbLinkError, UsbLinkSession};
use crate::wifi_session::session_binding;

const JOIN_TIMEOUT: Duration = Duration::from_secs(30);
const ATTACHMENT_ID: &str = "r1/pico-network-attachment-1";
static NETWORK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

#[embassy_executor::task]
async fn network_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

type JoinScheduler = FixedScheduler<
    OperationDriver<NetworkOperation, PORTS>,
    FixedValueStore<QUEUE_SLOTS, { conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES as usize }>,
    FixedClueLog<RUNTIME_CLUE_EVENTS>,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

struct JoinKernel {
    scheduler: JoinScheduler,
    endpoint: conduit_kernel::RemoteEndpointId,
    cord: conduit_kernel::CordId,
    node: conduit_kernel::NodeId,
    operation: conduit_kernel::HostOperationId,
    clue_node: conduit_kernel::NodeId,
    clue_operation: conduit_kernel::HostOperationId,
}

impl JoinKernel {
    fn new() -> Result<Self, UsbLinkError> {
        crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::KernelStorage);
        let layout = network_join_layout().ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
        let remote = generated_remote_endpoint().ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
        let values = FixedValueStore::new(
            conduit_net::MAXIMUM_JOIN_INPUT_BYTES + conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES,
        )
            .map_err(UsbLinkError::Storage)?;
        let clue = FixedClueLog::new(RUNTIME_CLUE_BYTES).map_err(UsbLinkError::ClueStorage)?;
        let join = NetworkOperation::join(
            layout.join_input_port,
            layout.join_output_port,
            layout.join_operation,
        );
        let attachment_clue =
            NetworkOperation::attachment_clue(layout.clue_input_port, layout.clue_operation);
        let join = OperationDriver::new(join).map_err(UsbLinkError::Kernel)?;
        let attachment_clue =
            OperationDriver::new(attachment_clue).map_err(UsbLinkError::Kernel)?;
        let drivers = match (layout.join_node.0, layout.clue_node.0) {
            (0, 1) => [join, attachment_clue],
            (1, 0) => [attachment_clue, join],
            _ => return Err(UsbLinkError::InvalidGeneratedEndpoint),
        };
        let nodes = generated_nodes();
        let cords = generated_cords();
        crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::KernelRoutes);
        let routes = generated_routes().map_err(|_| UsbLinkError::InvalidGeneratedEndpoint)?;
        let host_bindings =
            generated_host_bindings().map_err(|_| UsbLinkError::InvalidGeneratedEndpoint)?;
        crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::KernelScheduler);
        let scheduler = JoinScheduler::new_with_host_operations(
            nodes,
            cords,
            routes,
            host_bindings,
            drivers,
            values,
            clue,
        )
        .map_err(UsbLinkError::Kernel)?;
        Ok(Self {
            scheduler,
            endpoint: remote.endpoint,
            cord: remote.cord,
            node: layout.join_node,
            operation: layout.join_operation,
            clue_node: layout.clue_node,
            clue_operation: layout.clue_operation,
        })
    }

    fn admit(&mut self, sequence: u64, payload: &[u8]) -> Result<RemoteIngressOutcome, UsbLinkError> {
        self.scheduler
            .admit_remote_input(self.endpoint, self.cord, sequence, payload)
            .map_err(UsbLinkError::Kernel)
    }

    async fn execute(
        &mut self,
        control: &mut cyw43::Control<'_>,
        stack: Stack<'static>,
        clue: &mut UsbCdc,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<(), UsbLinkError> {
        loop {
            if let Some(request) = self.scheduler.next_host_request() {
                if request.node == self.clue_node && request.operation == self.clue_operation {
                    let encoded = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(UsbLinkError::Kernel)?;
                    let attachment = conduit_net::decode_network_attachment(encoded)
                        .map_err(|_| UsbLinkError::InvalidNetworkJoin)?;
                    let expected = attachment_identity(runtime);
                    if attachment.attachment_id != expected.attachment_id
                        || attachment.host_id != expected.host_id
                        || attachment.boot_id != expected.boot_id
                        || attachment.interface_pool_id != expected.interface_pool_id
                        || attachment.generation != expected.generation
                    {
                        return Err(UsbLinkError::InvalidNetworkJoin);
                    }
                    clue.write_network_attachment(expected).await?;
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
                    self.scheduler.step().map_err(UsbLinkError::Kernel)?;
                    return Ok(());
                }
                if request.node != self.node || request.operation != self.operation {
                    return Err(UsbLinkError::InvalidGeneratedEndpoint);
                }
                let mut ssid = HString::<{ conduit_net::MAXIMUM_SSID_BYTES }>::new();
                let mut credential = [0_u8; conduit_net::MAXIMUM_CREDENTIAL_BYTES];
                let credential_len;
                {
                    let encoded = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(UsbLinkError::Kernel)?;
                    let decoded = conduit_net::decode_network_join_request(encoded)
                        .map_err(|_| UsbLinkError::InvalidNetworkJoin)?;
                    ssid.push_str(
                        core::str::from_utf8(decoded.ssid)
                            .map_err(|_| UsbLinkError::InvalidNetworkJoin)?,
                    )
                    .map_err(|_| UsbLinkError::InvalidNetworkJoin)?;
                    credential_len = decoded.credential.len();
                    credential[..credential_len].copy_from_slice(decoded.credential);
                }
                let join_result = with_timeout(
                    JOIN_TIMEOUT,
                    control.join(
                        ssid.as_str(),
                        cyw43::JoinOptions::new(&credential[..credential_len]),
                    ),
                )
                .await;
                ssid.clear();
                credential.fill(0);
                match join_result {
                    Ok(Ok(())) => {}
                    Ok(Err(_)) => return Err(UsbLinkError::NetworkJoinFailed),
                    Err(_) => return Err(UsbLinkError::NetworkConfigurationTimeout),
                }
                with_timeout(JOIN_TIMEOUT, stack.wait_config_up())
                    .await
                    .map_err(|_| UsbLinkError::NetworkConfigurationTimeout)?;
                let identity = attachment_identity(runtime);
                let mut attachment = [0_u8; conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES as usize];
                let attachment_len = conduit_net::encode_network_attachment(
                    conduit_net::NetworkAttachmentInfo {
                        attachment_id: identity.attachment_id,
                        host_id: identity.host_id,
                        boot_id: identity.boot_id,
                        interface_pool_id: identity.interface_pool_id,
                        generation: identity.generation,
                    },
                    &mut attachment,
                )
                .map_err(|_| UsbLinkError::InvalidNetworkJoin)?;
                let output = self
                    .scheduler
                    .store_host_value(&attachment[..attachment_len])
                    .map_err(UsbLinkError::Kernel)?;
                self.scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: Some(
                                BoundedValueRef::new(
                                    output,
                                    conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES,
                                )
                                .expect("encoded attachment fits the planned output bound"),
                            ),
                            failure: None,
                        },
                    )
                    .map_err(UsbLinkError::Kernel)?;
                self.scheduler.step().map_err(UsbLinkError::Kernel)?;
                continue;
            }
            match self.scheduler.step().map_err(UsbLinkError::Kernel)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => return Err(UsbLinkError::KernelIdle),
                SchedulerStatus::Complete => return Err(UsbLinkError::KernelCompletedEarly),
                SchedulerStatus::Cancelled => return Err(UsbLinkError::KernelCancelled),
            }
        }
    }

    fn finish(&mut self, final_sequence: u64) -> Result<(), UsbLinkError> {
        if final_sequence != 1 {
            return Err(UsbLinkError::InvalidNetworkJoin);
        }
        self.scheduler
            .close_remote_input(self.endpoint, self.cord)
            .map_err(UsbLinkError::Kernel)?;
        loop {
            match self.scheduler.step().map_err(UsbLinkError::Kernel)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => return Ok(()),
                SchedulerStatus::Idle => return Err(UsbLinkError::KernelIdle),
                SchedulerStatus::Cancelled => return Err(UsbLinkError::KernelCancelled),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    spawner: &Spawner,
    carrier: PicoUsbCdcCarrier,
    clue: &mut UsbCdc,
    panic_record: Option<crate::panic_recovery::PanicRecord>,
    pio0: Peri<'static, PIO0>,
    dma: Peri<'static, DMA_CH0>,
    pin23: Peri<'static, PIN_23>,
    pin24: Peri<'static, PIN_24>,
    pin25: Peri<'static, PIN_25>,
    pin29: Peri<'static, PIN_29>,
    fw: &'static aligned::Aligned<aligned::A4, [u8]>,
    nvram: &'static aligned::Aligned<aligned::A4, [u8]>,
    clm: &'static [u8],
    runtime: &RuntimeTranscriptIdentity,
) -> ! {
    let mut link = UsbLinkSession::new(carrier).unwrap();
    if let Some(record) = panic_record {
        if establish_usb(&mut link, clue, runtime).await.is_ok() {
            let _ = clue
                .write_network_failure(record.code(), attachment_identity(runtime))
                .await;
        }
        loop {
            crate::bootsel::wait_for_request(&mut link).await.ok();
        }
    }
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::RadioDriverStartup);
    let usb_startup = establish_usb(&mut link, clue, runtime);
    let radio_startup = crate::radio::init_cyw43_network(
        spawner, pio0, dma, pin23, pin24, pin25, pin29, fw, nvram, clm,
    );
    let (usb_result, radio_result) = join(usb_startup, radio_startup).await;
    if usb_result.is_err() {
        core::future::pending::<()>().await;
    }
    let (device, mut control) = match radio_result {
        Ok(radio) => radio,
        Err(error) => {
            let _ = clue
                .write_network_failure(error.code(), attachment_identity(runtime))
                .await;
            loop {
                crate::bootsel::wait_for_request(&mut link).await.ok();
            }
        }
    };
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::NetworkStackStartup);
    let (stack, runner) = embassy_net::new(
        device,
        Config::dhcpv4(Default::default()),
        NETWORK_RESOURCES.init(StackResources::new()),
        0x502,
    );
    spawner.spawn(network_task(runner).unwrap());
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::SessionBinding);
    if let Err(error) = run_session(&mut link, clue, &mut control, stack, runtime).await {
        let _ = clue
            .write_network_failure(error.code(), attachment_identity(runtime))
            .await;
        // This Play is terminal, but CDC 0 remains an exact-build BOOTSEL
        // recovery path across host disconnects.
        loop {
            crate::bootsel::wait_for_request(&mut link).await.ok();
        }
    }
    crate::bootsel::wait_for_request(&mut link).await.ok();
    loop {
        core::future::pending::<()>().await;
    }
}

async fn establish_usb(
    link: &mut UsbLinkSession,
    clue: &mut UsbCdc,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), UsbLinkError> {
    let mut frame = [0_u8; 2048];
    link.wait_connection().await;
    loop {
        let raw = link.receive_raw_stream_frame(&mut frame).await?;
        if crate::bootsel::handle_request(link, raw).await? {
            continue;
        }
        if raw == b"CONDUIT_RAW_CDC0_PROBE" {
            link.send_raw_stream_frame(b"CONDUIT_RAW_CDC0_REPLY").await?;
            break;
        }
    }
    clue.wait_dtr().await;
    clue
        .write_boot_identity(
            crate::receipts::BootIdentity {
                firmware_build_id: crate::network_image::FIRMWARE_BUILD_ID,
                source_document_id: crate::network_image::SOURCE_DOCUMENT_ID,
                checked_form_id: crate::network_image::CHECKED_FORM_ID,
                expanded_form_id: crate::network_image::EXPANDED_FORM_ID,
                plan_id: crate::network_image::PLAN_ID,
                fragment_id: crate::network_image::FRAGMENT_ID,
                host_id: crate::network_image::HOST_ID,
                boot_id: crate::network_image::BOOT_ID,
                boot_clue_id: crate::network_image::BOOT_CLUE_ID,
            },
            runtime,
        )
        .await?;
    Ok(())
}

async fn run_session(
    link: &mut UsbLinkSession,
    clue: &mut UsbCdc,
    control: &mut cyw43::Control<'_>,
    stack: Stack<'static>,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<(), UsbLinkError> {
    let binding = session_binding(runtime)?;
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::SessionMachine);
    let mut machine = SessionMachine::new(binding.clone(), SessionRole::Sink)
        .map_err(UsbLinkError::Codec)?;
    crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::KernelStorage);
    let mut kernel = JoinKernel::new()?;
    let mut frame_buf = [0_u8; 2048];
    loop {
        let raw = link.receive_raw_stream_frame(&mut frame_buf).await?;
        if crate::bootsel::handle_request(link, raw).await? {
            continue;
        }
        if raw == conduit_net::R1_USB_NETWORK_SESSION_QUERY {
            link.send_raw_stream_frame(conduit_net::R1_USB_NETWORK_SESSION_READY).await?;
            crate::panic_recovery::set_phase(crate::panic_recovery::PanicPhase::SessionExecution);
            break;
        }
        return Err(UsbLinkError::InvalidNetworkJoin);
    }
    let hello = link.receive_frame(&mut frame_buf).await?;
    machine.admit_inbound(hello).map_err(UsbLinkError::Codec)?;
    let response = binding.hello_frame();
    machine.admit_outbound(response).map_err(UsbLinkError::Codec)?;
    link.send_frame(&response).await?;
    let ready = link.receive_frame(&mut frame_buf).await?;
    machine.admit_inbound(ready).map_err(UsbLinkError::Codec)?;
    let response = binding.frame(SessionMessage::Ready);
    machine.admit_outbound(response).map_err(UsbLinkError::Codec)?;
    link.send_frame(&response).await?;
    let offered = link.receive_frame(&mut frame_buf).await?;
    let (sequence, payload) = match offered.message {
        SessionMessage::Offered { sequence, payload } => (sequence, payload),
        _ => return Err(UsbLinkError::InvalidNetworkJoin),
    };
    machine.admit_inbound(offered).map_err(UsbLinkError::Codec)?;
    if !matches!(kernel.admit(sequence, payload)?, RemoteIngressOutcome::Accepted { .. }) {
        return Err(UsbLinkError::InvalidNetworkJoin);
    }
    let accepted = binding.frame(SessionMessage::Accepted { sequence });
    machine.admit_outbound(accepted).map_err(UsbLinkError::Codec)?;
    link.send_frame(&accepted).await?;
    kernel.execute(control, stack, clue, runtime).await?;
    let delivered = binding.frame(SessionMessage::Delivered { sequence });
    machine.admit_outbound(delivered).map_err(UsbLinkError::Codec)?;
    link.send_frame(&delivered).await?;
    let closed = link.receive_frame(&mut frame_buf).await?;
    let final_sequence = match closed.message {
        SessionMessage::InputClosed { final_sequence } => final_sequence,
        _ => return Err(UsbLinkError::InvalidNetworkJoin),
    };
    machine.admit_inbound(closed).map_err(UsbLinkError::Codec)?;
    kernel.finish(final_sequence)?;
    let terminal = link.receive_frame(&mut frame_buf).await?;
    if !matches!(
        terminal.message,
        SessionMessage::Terminal {
            disposition: conduit_wire::SessionTerminalDisposition::Completed,
            final_sequence: peer_final,
        } if peer_final == final_sequence
    ) {
        return Err(UsbLinkError::InvalidNetworkJoin);
    }
    machine.admit_inbound(terminal).map_err(UsbLinkError::Codec)?;
    let response = binding.frame(SessionMessage::Terminal {
        disposition: conduit_wire::SessionTerminalDisposition::Completed,
        final_sequence,
    });
    machine.admit_outbound(response).map_err(UsbLinkError::Codec)?;
    link.send_frame(&response).await?;
    if !machine.is_terminal() {
        return Err(UsbLinkError::InvalidNetworkJoin);
    }
    Ok(())
}

fn attachment_identity<'a>(runtime: &'a RuntimeTranscriptIdentity) -> NetworkAttachmentIdentity<'a> {
    NetworkAttachmentIdentity {
        firmware_build_id: crate::network_image::FIRMWARE_BUILD_ID,
        source_document_id: crate::network_image::SOURCE_DOCUMENT_ID,
        checked_form_id: crate::network_image::CHECKED_FORM_ID,
        expanded_form_id: crate::network_image::EXPANDED_FORM_ID,
        plan_id: crate::network_image::PLAN_ID,
        fragment_id: crate::network_image::FRAGMENT_ID,
        host_id: crate::network_image::HOST_ID,
        boot_id: runtime.boot_id(),
        active_play_id: runtime.active_play_id(),
        attachment_id: ATTACHMENT_ID,
        interface_pool_id: conduit_net::R1_WIFI_STATION_POOL_ID,
        generation: 1,
        clue_id: crate::network_image::ATTACHMENT_CLUE_ID,
    }
}
