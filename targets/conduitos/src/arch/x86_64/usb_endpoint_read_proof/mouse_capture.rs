//! Explicit two-capture mouse appliance; report history remains in checked Source.
use super::capture_kernel::{admit_unit, publish_call, step_pure};
use super::capture_preparation::prepare;
use super::*;
use crate::arch::x86_64::usb::{
    endpoint_read::window::EndpointReadWindowDma, endpoint_setup::ConfiguredInboundEndpoint,
};

pub(super) fn run(
    controller: &mut XhciReady,
    device: UsbDevice,
    configured: ConfiguredInboundEndpoint,
    dma: EndpointReadWindowDma<'_, 2>,
    ids: &BootIdentities,
    base: &[u8; 32],
) -> Result<(), &'static str> {
    let device_dma =
        super::super::dma::device_dma_pointer(&device).map_err(|_| "usb-hid-capture-device")?;
    let decoder = prepare_sample_decoder()?;
    let mut sample_bytes = Vec::with_capacity(4096);
    let mut capture = unsafe { prepare(controller, device, configured, dma, ids, base) }?;
    let (root_port, slot, dci) = capture.owner.target();
    let reads: [PortId; 2] = (0..2)
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
    if capture.outputs.len() != 1 || capture.outputs[0].port_id.as_str() != "event" {
        return Err("usb-hid-mouse-capture-outputs");
    }
    let event_port = capture.outputs[0].port_id.clone();
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
    let mut pending = [None; 2];
    let mut nodes = [None; 2];
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
    for index in 0..2 {
        admit_unit(&mut capture, &reads[index], 0, &unit, false)?;
        let (node, admitted) = publish_call(&mut capture)?;
        if nodes[..index].contains(&Some(node)) {
            return Err("usb-hid-capture-node");
        }
        nodes[index] = Some(node);
        pending[index] = Some(admitted);
    }
    // Publish the complete window before the first emulator input/completion.
    if capture.owner.pending_count() != 2 {
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
        let mut delivered = false;
        for _ in 0..8192 {
            step_pure(&mut capture)?;
            if let Some(actual) = capture
                .play
                .kernel_mut()
                .output_into(&event_port, &mut buffers[0])
                .map_err(|_| "usb-hid-mouse-capture-output")?
            {
                if actual != sequence || buffers[0].encoded.capacity() != 4096 {
                    return Err("usb-hid-mouse-capture-output-order");
                }
                let value = validate_canonical_structured_value(&buffers[0].encoded)
                    .map_err(|_| "usb-hid-mouse-capture-event")?;
                let sample = value
                    .variant_payload("sample")
                    .map_err(|_| "usb-hid-mouse-capture-event")?
                    .ok_or("usb-hid-mouse-capture-event")?;
                if sample
                    .record_field("ordinal")
                    .map_err(|_| "usb-hid-mouse-capture-ordinal")?
                    .ok_or("usb-hid-mouse-capture-ordinal")?
                    .primitive_bytes("value/u64")
                    .map_err(|_| "usb-hid-mouse-capture-ordinal")?
                    != sequence.to_le_bytes()
                {
                    return Err("usb-hid-mouse-capture-ordinal");
                }
                let sample = sample
                    .record_field("sample")
                    .map_err(|_| "usb-hid-mouse-capture-sample")?
                    .ok_or("usb-hid-mouse-capture-sample")?;
                sample_bytes.clear();
                sample_bytes.extend_from_slice(sample.type_bytes());
                sample_bytes.extend_from_slice(sample.value_node());
                let normalized = decoder
                    .decode(&sample_bytes)
                    .map_err(|_| "usb-hid-mouse-capture-sample")?;
                if normalized.sequence != sequence + 1
                    || normalized.queue_capacity != 8
                    || normalized.coalesced != 0
                    || normalized.dropped != 0
                {
                    return Err("usb-hid-mouse-capture-history");
                }
                digest.update(sequence.to_le_bytes());
                digest.update(event_port.as_str().as_bytes());
                digest.update(&buffers[0].encoded);
                capture
                    .play
                    .kernel_mut()
                    .complete_output(&event_port, actual)
                    .map_err(|_| "usb-hid-mouse-capture-ack")?;
                delivered = true;
                break;
            }
        }
        if !delivered {
            return Err("usb-hid-mouse-capture-delivery");
        }
        let (_, next_cycle) = capture.owner.ring_position();
        if next_cycle != cycle {
            wraps += 1;
            cycle = next_cycle;
        }
        if sequence < 126 {
            let next = sequence + 2;
            admit_unit(&mut capture, &reads[member], next / 2, &unit, next >= 126)?;
            let (node, admitted) = publish_call(&mut capture)?;
            if Some(node) != nodes[member] {
                return Err("usb-hid-capture-rearm-node");
            }
            pending[member] = Some(admitted);
            controller.ring_endpoint(slot, dci);
            if capture.owner.pending_count() != 2 {
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
            .output_into(&event_port, &mut buffers[0])
            .map_err(|_| "usb-hid-capture-ended")?
        {
            if sequence != 128 || ended_seen {
                return Err("usb-hid-capture-ended-order");
            }
            capture
                .play
                .kernel_mut()
                .complete_output(&event_port, sequence)
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
    let final_event = validate_canonical_structured_value(&buffers[0].encoded)
        .map_err(|_| "usb-hid-capture-end-value")?;
    let end = final_event
        .variant_payload("ended")
        .map_err(|_| "usb-hid-capture-end-value")?
        .ok_or("usb-hid-capture-end-value")?;
    if end.variant_tag().map_err(|_| "usb-hid-capture-end-value")? != "closed" {
        return Err("usb-hid-capture-abnormal-end");
    }
    digest.update(128_u64.to_le_bytes());
    digest.update(event_port.as_str().as_bytes());
    digest.update(&buffers[0].encoded);
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
    writeln!(sign, "CONDUIT_USB_HID_ENDPOINT_SIGN {{\"schema\":\"conduit.conduitos.usb-hid-endpoint/v1\",\"proof_class\":\"freestanding-emulator\",\"source_document_id\":\"{}\",\"checked_plot_id\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"device_instance_id\":\"{}\",\"transfers\":128,\"cycle_transitions\":{},\"transcript_digest\":\"{}\",\"normal_close\":true,\"acknowledged_stop\":true,\"fixture_protocol\":true,\"allocation_sealed\":true,\"capture_buffers\":2,\"maximum_pending_transfers\":2}}", capture.identity.source_document_id.as_str(), capture.identity.checked_plot_id.as_str(), capture.identity.plan_id.as_str(), capture.identity.active_play_id.as_str(), device_hex, wraps, digest_hex).map_err(|_| "usb-hid-capture-sign")?;
    early_write(sign.as_bytes());
    Ok(())
}

fn prepare_sample_decoder()
-> Result<crate::source_pointer_sample::SourcePointerSampleDecoder, &'static str> {
    use crate::protocol_source::{PreparedProtocolEntry, usb_hid_mouse_order_package};
    let package = usb_hid_mouse_order_package().map_err(|_| "usb-hid-mouse-capture-package")?;
    let bytes = serde_json::to_vec(&package).map_err(|_| "usb-hid-mouse-capture-package")?;
    let entry = PreparedProtocolEntry::prepare(&bytes, "usb-hid-mouse-capture-window")
        .map_err(|_| "usb-hid-mouse-capture-source")?;
    let schema = entry
        .output_schema(&PortId::from("event"))
        .ok_or("usb-hid-mouse-capture-schema")?;
    let StructuredInfoTypeShape::Variant { cases, .. } = schema.shape() else {
        return Err("usb-hid-mouse-capture-schema");
    };
    let sample = cases
        .iter()
        .find(|case| case.tag() == "sample")
        .ok_or("usb-hid-mouse-capture-schema")?
        .payload_type();
    let StructuredInfoTypeShape::Record { fields, .. } = sample.shape() else {
        return Err("usb-hid-mouse-capture-schema");
    };
    let schema = fields
        .iter()
        .find(|field| field.name() == "sample")
        .ok_or("usb-hid-mouse-capture-schema")?
        .value_type();
    crate::source_pointer_sample::SourcePointerSampleDecoder::prepare(schema)
        .map_err(|_| "usb-hid-mouse-capture-schema")
}
