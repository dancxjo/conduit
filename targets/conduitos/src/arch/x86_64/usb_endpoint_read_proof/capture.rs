//! Explicit eight-capture appliance driven by the sole controller and shared kernel.
use super::capture_preparation::{PreparedNativeCapture, prepare};
use super::*;
use crate::arch::x86_64::usb::{
    endpoint_read::window::EndpointReadWindowDma, endpoint_setup::ConfiguredInboundEndpoint,
};
use conduit_composite::AdmittedKernelCompositeHostRequest;
use conduit_kernel::NodeId;
use conduit_kernel::scheduler::RemoteIngressOutcome;

pub(super) fn run(
    controller: &mut XhciReady,
    device: UsbDevice,
    configured: ConfiguredInboundEndpoint,
    dma: EndpointReadWindowDma<'_, 8>,
    ids: &BootIdentities,
    base: &[u8; 32],
) -> Result<(), &'static str> {
    let device_dma =
        super::super::dma::device_dma_pointer(&device).map_err(|_| "usb-hid-capture-device")?;
    let mut capture = unsafe { prepare(controller, device, configured, dma, ids, base) }?;
    let (root_port, slot, dci) = capture.owner.target();
    let reads: [PortId; 8] = (0..8)
        .map(|index| {
            let name = alloc::format!("read{index}");
            capture
                .inputs
                .iter()
                .find(|port| port.port_id.as_str() == name)
                .map(|port| port.port_id.clone())
                .ok_or("usb-hid-capture-input")
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "usb-hid-capture-input")?;
    let begin = capture
        .inputs
        .iter()
        .find(|port| port.port_id.as_str() == "begin")
        .ok_or("usb-hid-capture-begin")?
        .port_id
        .clone();
    let observation = capture
        .outputs
        .iter()
        .position(|port| port.port_id.as_str() == "observation")
        .ok_or("usb-hid-capture-observation")?;
    let changes = capture
        .outputs
        .iter()
        .position(|port| port.port_id.as_str() == "changes")
        .ok_or("usb-hid-capture-changes")?;
    let ended = capture
        .outputs
        .iter()
        .position(|port| port.port_id.as_str() == "ended")
        .ok_or("usb-hid-capture-ended")?;
    if capture.outputs.len() != 3 {
        return Err("usb-hid-capture-outputs");
    }
    let mut buffers: Vec<_> = capture
        .outputs
        .iter()
        .map(|port| ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    let unit = ValuePayload {
        value_kind: kind_id("value/unit"),
        encoded: Vec::new(),
    };
    let device_hex = identity::hex(&capture.device_id);
    let mut pending = [None; 8];
    let mut nodes = [None; 8];
    let mut digest = Sha256::new();
    let mut wraps = 0;
    let mut cycle = 1;
    let mut ended_seen = false;
    // All preparation is complete before the production allocator forbids growth.
    crate::allocation::BOOT_ARENA.seal();
    capture
        .play
        .kernel_mut()
        .start()
        .map_err(|_| "usb-hid-capture-start")?;
    admit_unit(&mut capture, &begin, 0, &unit, true)?;
    for index in 0..8 {
        admit_unit(&mut capture, &reads[index], 0, &unit, false)?;
        let (node, admitted) = publish_call(&mut capture)?;
        if nodes[..index].contains(&Some(node)) {
            return Err("usb-hid-capture-node");
        }
        nodes[index] = Some(node);
        pending[index] = Some(admitted);
    }
    // Publish the complete window before the first emulator input/completion.
    if capture.owner.pending_count() != 8 {
        return Err("usb-hid-capture-window");
    }
    controller.ring_endpoint(slot, dci);
    for sequence in 0..128_u64 {
        let mut marker = FixedText::new();
        writeln!(marker, "CONDUIT_USB_ENDPOINT_READY {sequence}")
            .map_err(|_| "usb-hid-capture-marker")?;
        early_write(marker.as_bytes());
        let mut member = None;
        for _ in 0..4_000_000 {
            if controller.port_status(root_port) & 1 == 0 {
                return Err("usb-hid-capture-provider-lost-retained-dma");
            }
            if let Some(event) = controller.poll_event() {
                let completed = capture
                    .owner
                    .complete(event)
                    .map_err(|_| "usb-hid-capture-completion-retained-dma")?;
                let index = nodes
                    .iter()
                    .position(|node| *node == Some(completed.node))
                    .ok_or("usb-hid-capture-completion-node")?;
                let admitted = pending[index]
                    .take()
                    .ok_or("usb-hid-capture-completion-call")?;
                let kernel = capture.play.kernel_mut();
                if completed.ordinal != sequence
                    || kernel
                        .admitted_host_request_view(&admitted)
                        .map_err(|_| "usb-hid-capture-completion-call")?
                        .request
                        .request
                        != completed.request
                {
                    return Err("usb-hid-capture-completion-order");
                }
                kernel
                    .complete_host_call_bytes(&admitted, completed.encoded)
                    .map_err(|_| "usb-hid-capture-result")?;
                member = Some(index);
                break;
            }
            core::hint::spin_loop();
        }
        let member = member.ok_or("usb-hid-capture-timeout-retained-dma")?;
        let mut seen = [false; 3];
        for _ in 0..8192 {
            step_pure(&mut capture)?;
            for (index, port) in capture.outputs.iter().enumerate() {
                if let Some(actual) = capture
                    .play
                    .kernel_mut()
                    .output_into(&port.port_id, &mut buffers[index])
                    .map_err(|_| "usb-hid-capture-output")?
                {
                    if buffers[index].encoded.capacity() != 4096
                        || seen[index]
                        || (index != ended && actual != sequence)
                        || (index == ended && (sequence != 127 || actual != 0 || ended_seen))
                    {
                        return Err("usb-hid-capture-output-order");
                    }
                    if index == observation {
                        let value = validate_canonical_structured_value(&buffers[index].encoded)
                            .map_err(|_| "usb-hid-capture-observation")?;
                        if value
                            .record_field("ordinal")
                            .map_err(|_| "usb-hid-capture-observation")?
                            .ok_or("usb-hid-capture-observation")?
                            .primitive_bytes("value/u64")
                            .map_err(|_| "usb-hid-capture-observation")?
                            != sequence.to_le_bytes()
                        {
                            return Err("usb-hid-capture-wire-order");
                        }
                    }
                    capture
                        .play
                        .kernel_mut()
                        .complete_output(&port.port_id, actual)
                        .map_err(|_| "usb-hid-capture-ack")?;
                    seen[index] = true;
                    ended_seen |= index == ended;
                }
            }
            if seen[observation] && seen[changes] {
                break;
            }
        }
        if !seen[observation] || !seen[changes] {
            return Err("usb-hid-capture-delivery");
        }
        // Digest in declared port order, independent of internal scheduling.
        for (index, port) in capture
            .outputs
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != ended)
        {
            digest.update(sequence.to_le_bytes());
            digest.update(port.port_id.as_str().as_bytes());
            digest.update(&buffers[index].encoded);
        }
        let (_, next_cycle) = capture.owner.ring_position();
        if next_cycle != cycle {
            wraps += 1;
            cycle = next_cycle;
        }
        if sequence < 120 {
            let next = sequence + 8;
            admit_unit(&mut capture, &reads[member], next / 8, &unit, next >= 120)?;
            let (node, admitted) = publish_call(&mut capture)?;
            if Some(node) != nodes[member] {
                return Err("usb-hid-capture-rearm-node");
            }
            pending[member] = Some(admitted);
            controller.ring_endpoint(slot, dci);
            if capture.owner.pending_count() != 8 {
                return Err("usb-hid-capture-rearm-window");
            }
        }
    }
    let mut complete = false;
    for _ in 0..8192 {
        let status = step_pure(&mut capture)?;
        if let Some(sequence) = capture
            .play
            .kernel_mut()
            .output_into(&capture.outputs[ended].port_id, &mut buffers[ended])
            .map_err(|_| "usb-hid-capture-ended")?
        {
            if sequence != 0 || ended_seen {
                return Err("usb-hid-capture-ended-order");
            }
            capture
                .play
                .kernel_mut()
                .complete_output(&capture.outputs[ended].port_id, sequence)
                .map_err(|_| "usb-hid-capture-ended-ack")?;
            ended_seen = true;
        }
        if status == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    if !complete
        || !ended_seen
        || wraps != 2
        || capture.owner.pending_count() != 0
        || pending.iter().any(Option::is_some)
    {
        return Err("usb-hid-capture-drain");
    }
    let end = validate_canonical_structured_value(&buffers[ended].encoded)
        .map_err(|_| "usb-hid-capture-end-value")?;
    if end.variant_tag().map_err(|_| "usb-hid-capture-end-value")? != "closed" {
        return Err("usb-hid-capture-abnormal-end");
    }
    digest.update(0_u64.to_le_bytes());
    digest.update(capture.outputs[ended].port_id.as_str().as_bytes());
    digest.update(&buffers[ended].encoded);
    for (index, port) in capture.outputs.iter().enumerate() {
        if capture
            .play
            .kernel_mut()
            .output_terminal_into(&port.port_id, &mut buffers[index])
            .map_err(|_| "usb-hid-capture-terminal")?
            != Some(KernelCompositeTerminal::Normal)
        {
            return Err("usb-hid-capture-terminal");
        }
    }
    capture
        .owner
        .revoke_all()
        .map_err(|_| "usb-hid-capture-revoke")?;
    controller
        .disable_removed_slot(slot)
        .map_err(|_| "usb-hid-capture-stop-retained-dma")?;
    unsafe { capture.owner.release_stopped() }.map_err(|_| "usb-hid-capture-release")?;
    // This appliance owns the slot's sole configured data endpoint. Retire the
    // slot only after the controller ACK and every member's quiescent release.
    unsafe {
        (*device_dma).owner_slot = 0;
    }
    let digest: [u8; 32] = digest.finalize().into();
    let mut digest_text = FixedText::new();
    for byte in digest {
        write!(digest_text, "{byte:02x}").map_err(|_| "usb-hid-capture-digest")?;
    }
    let digest_hex =
        core::str::from_utf8(digest_text.as_bytes()).map_err(|_| "usb-hid-capture-digest")?;
    let mut sign = FixedText::new();
    writeln!(sign, "CONDUIT_USB_HID_ENDPOINT_SIGN {{\"schema\":\"conduit.conduitos.usb-hid-endpoint/v1\",\"proof_class\":\"freestanding-emulator\",\"source_document_id\":\"{}\",\"checked_plot_id\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"device_instance_id\":\"{}\",\"transfers\":128,\"cycle_transitions\":{},\"transcript_digest\":\"{}\",\"normal_close\":true,\"acknowledged_stop\":true,\"fixture_protocol\":true,\"allocation_sealed\":true,\"capture_buffers\":8,\"maximum_pending_transfers\":8}}", capture.identity.source_document_id.as_str(), capture.identity.checked_plot_id.as_str(), capture.identity.plan_id.as_str(), capture.identity.active_play_id.as_str(), device_hex, wraps, digest_hex).map_err(|_| "usb-hid-capture-sign")?;
    early_write(sign.as_bytes());
    Ok(())
}

fn admit_unit(
    capture: &mut PreparedNativeCapture<'_>,
    port: &PortId,
    sequence: u64,
    unit: &ValuePayload,
    close: bool,
) -> Result<(), &'static str> {
    if !matches!(
        capture
            .play
            .kernel_mut()
            .admit_input(port, sequence, unit)
            .map_err(|_| "usb-hid-capture-input")?,
        RemoteIngressOutcome::Accepted { .. }
    ) {
        return Err("usb-hid-capture-input-pressure");
    }
    if close {
        capture
            .play
            .kernel_mut()
            .close_input(port)
            .map_err(|_| "usb-hid-capture-close")?;
    }
    Ok(())
}

fn publish_call(
    capture: &mut PreparedNativeCapture<'_>,
) -> Result<(NodeId, AdmittedKernelCompositeHostRequest), &'static str> {
    for _ in 0..8192 {
        capture
            .play
            .kernel_mut()
            .step()
            .map_err(|_| "usb-hid-capture-step")?;
        if let Some(request) = capture.play.kernel_mut().next_host_request()
            && !capture
                .play
                .service_pure_call(&request)
                .map_err(|_| "usb-hid-capture-pure")?
        {
            let kernel = capture.play.kernel_mut();
            let obligation = kernel
                .host_request_obligation(&request)
                .map_err(|_| "usb-hid-capture-obligation")?;
            let admitted = kernel
                .admit_host_request(
                    &request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .map_err(|_| "usb-hid-capture-admission")?;
            let call = *kernel
                .admitted_host_request_view(&admitted)
                .map_err(|_| "usb-hid-capture-call")?
                .request;
            capture
                .owner
                .begin(
                    call.node,
                    call.call,
                    call.request,
                    kernel
                        .host_request_input(&admitted)
                        .map_err(|_| "usb-hid-capture-call-input")?,
                )
                .map_err(|_| "usb-hid-capture-submit")?;
            return Ok((call.node, admitted));
        }
    }
    Err("usb-hid-capture-call-budget")
}

fn step_pure(
    capture: &mut PreparedNativeCapture<'_>,
) -> Result<KernelCompositeStatus, &'static str> {
    let status = capture
        .play
        .kernel_mut()
        .step()
        .map_err(|_| "usb-hid-capture-step")?;
    if let Some(request) = capture.play.kernel_mut().next_host_request()
        && !capture
            .play
            .service_pure_call(&request)
            .map_err(|_| "usb-hid-capture-pure")?
    {
        return Err("usb-hid-capture-unexpected-call");
    }
    Ok(status)
}
