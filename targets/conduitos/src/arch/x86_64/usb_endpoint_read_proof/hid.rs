//! Explicit native keyboard Source appliance over the existing endpoint owner.
use super::*;
use crate::arch::x86_64::usb::endpoint_setup::ConfiguredInboundEndpoint;
use crate::usb_base::{
    hid_endpoint_proof_kernel::PreparedHidEndpointProofKernel, hid_endpoint_proof_plan,
};

pub(super) fn run(
    controller: &mut XhciReady,
    device: UsbDevice,
    configured: ConfiguredInboundEndpoint,
    dma: EndpointReceiveDma<'_>,
    ids: &BootIdentities,
    base: &[u8; 32],
) -> Result<(), &'static str> {
    let mouse = cfg!(feature = "usb-hid-mouse-proof");
    let entry = if mouse {
        "usb-hid-mouse-endpoint"
    } else {
        "usb-hid-keyboard-endpoint"
    };
    let slot = device.slot;
    let dci = configured.dci;
    let endpoint_epoch = configured.endpoint_epoch;
    let epoch = device.attachment_epoch;
    let device_id =
        identity::derive_usb_device(&ids.boot, base, device.root_port, device.slot, epoch);
    let artifact = hid_endpoint_proof_plan::plan(
        &EndpointReadProofSubject {
            host_id: &identity::hex(&ids.host),
            boot_id: &identity::hex(&ids.boot),
            controller_base_id: &identity::hex(base),
            device_instance_id: &identity::hex(&device_id),
            root_port: device.root_port,
            slot,
            attachment_epoch: epoch,
            endpoint_dci: dci,
            endpoint_epoch,
        },
        entry,
    )
    .map_err(|_| "usb-hid-native-plan")?;
    let plan = artifact.artifact().definition().internal_plan.clone();
    let inputs = artifact
        .artifact()
        .definition()
        .external_capability
        .inputs
        .clone();
    let outputs = artifact
        .artifact()
        .definition()
        .external_capability
        .outputs
        .clone();
    let mut buffers: Vec<_> = outputs
        .iter()
        .map(|port| ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    let mut play = PreparedHidEndpointProofKernel::prepare(
        artifact,
        planning::ENDPOINT_READ_PROOF_SIGN_STORAGE,
    )
    .map_err(|_| "usb-hid-native-kernel")?;
    let (table, handle, claim) = possession::issue(&plan)?;
    let fragment = &plan.fragments[0];
    let gear = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
        .ok_or("usb-hid-native-endpoint")?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| "usb-hid-native-lowering")?;
    let node = lowered
        .identity
        .node_for_placement(&gear.placement_id)
        .ok_or("usb-hid-native-node")?;
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let contract = EndpointReadContract::prepare().map_err(|_| "usb-hid-native-contract")?;
    let mut owner = unsafe {
        UsbEndpointReadHostCall::bind_selected(
            controller,
            device,
            EndpointReadAttachment {
                slot,
                generation: u64::from(epoch),
                endpoint_generation: endpoint_epoch,
                endpoint_dci: dci,
                maximum_data_bytes: 2048,
                resource_bytes: core::mem::size_of::<ProofDma>() as u64,
            },
            configured,
            dma,
            table,
            handle,
            claim,
            &contract,
            SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &gear.placement_id,
            },
        )
    }
    .map_err(|_| "usb-hid-native-binding")?;
    let read = inputs
        .iter()
        .find(|port| port.port_id.as_str() == "read")
        .ok_or("usb-hid-native-read")?;
    let begin = inputs.iter().find(|port| port.port_id.as_str() == "begin");
    if begin.is_some() == mouse {
        return Err("usb-hid-native-entry-inputs");
    }
    let device_hex = identity::hex(&device_id);
    let unit = ValuePayload {
        value_kind: read.value_kind.clone(),
        encoded: Vec::new(),
    };
    // Root has retained every owner, buffer and identity. From here through
    // transfer, drain and acknowledged stop, the production allocator refuses growth.
    crate::allocation::BOOT_ARENA.seal();
    play.kernel_mut()
        .start()
        .map_err(|_| "usb-hid-native-start")?;
    if let Some(begin) = begin {
        play.kernel_mut()
            .admit_input(&begin.port_id, 0, &unit)
            .map_err(|_| "usb-hid-native-begin")?;
        play.kernel_mut()
            .close_input(&begin.port_id)
            .map_err(|_| "usb-hid-native-begin")?;
    }
    let mut digest = Sha256::new();
    let mut wraps = 0;
    let mut cycle = 1;
    for sequence in 0..128_u64 {
        play.kernel_mut()
            .admit_input(&read.port_id, sequence, &unit)
            .map_err(|_| "usb-hid-native-read")?;
        if sequence == 127 {
            play.kernel_mut()
                .close_input(&read.port_id)
                .map_err(|_| "usb-hid-native-close")?;
        }
        let mut pending = None;
        for _ in 0..2048 {
            play.kernel_mut()
                .step()
                .map_err(|_| "usb-hid-native-step")?;
            if let Some(request) = play.kernel_mut().next_host_request()
                && !play
                    .service_pure_call(&request)
                    .map_err(|_| "usb-hid-native-pure")?
            {
                pending = Some(request);
                break;
            }
        }
        let request = pending.ok_or("usb-hid-native-call-budget")?;
        let kernel = play.kernel_mut();
        let obligation = kernel
            .host_request_obligation(&request)
            .map_err(|_| "usb-hid-native-obligation")?;
        let admitted = kernel
            .admit_host_request(
                &request,
                &obligation.host,
                &obligation.resources,
                &obligation.authorities,
            )
            .map_err(|_| "usb-hid-native-admission")?;
        let call = *kernel
            .admitted_host_request_view(&admitted)
            .map_err(|_| "usb-hid-native-call")?
            .request;
        owner
            .begin(
                call.node,
                call.call,
                call.request,
                kernel
                    .host_request_input(&admitted)
                    .map_err(|_| "usb-hid-native-input")?,
            )
            .map_err(|_| "usb-hid-native-submit")?;
        let mut marker = FixedText::new();
        writeln!(marker, "CONDUIT_USB_ENDPOINT_READY {sequence}")
            .map_err(|_| "usb-hid-native-marker")?;
        early_write(marker.as_bytes());
        let mut completed = false;
        for _ in 0..4_000_000 {
            if let Some(bytes) = owner.poll().map_err(|_| "usb-hid-native-completion")? {
                kernel
                    .complete_host_call_bytes(&admitted, bytes)
                    .map_err(|_| "usb-hid-native-result")?;
                completed = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !completed {
            return Err("usb-hid-native-timeout-retained-dma");
        }
        let mut seen = [false; 3];
        if outputs.len() != if mouse { 2 } else { 3 } {
            return Err("usb-hid-native-output-count");
        }
        let seen = &mut seen[..outputs.len()];
        for _ in 0..2048 {
            play.kernel_mut()
                .step()
                .map_err(|_| "usb-hid-native-output-step")?;
            if let Some(request) = play.kernel_mut().next_host_request()
                && !play
                    .service_pure_call(&request)
                    .map_err(|_| "usb-hid-native-pure")?
            {
                return Err("usb-hid-native-unexpected-call");
            }
            for (index, port) in outputs.iter().enumerate() {
                if let Some(actual) = play
                    .kernel_mut()
                    .output_into(&port.port_id, &mut buffers[index])
                    .map_err(|_| "usb-hid-native-output")?
                {
                    if actual != sequence
                        || seen[index]
                        || buffers[index].encoded.capacity() != 4096
                    {
                        return Err("usb-hid-native-order");
                    }
                    play.kernel_mut()
                        .complete_output(&port.port_id, actual)
                        .map_err(|_| "usb-hid-native-ack")?;
                    seen[index] = true;
                }
            }
            if seen.iter().all(|seen| *seen) {
                break;
            }
        }
        if !seen.iter().all(|seen| *seen) {
            return Err("usb-hid-native-delivery");
        }
        for (index, port) in outputs.iter().enumerate() {
            digest.update(sequence.to_le_bytes());
            digest.update(port.port_id.as_str().as_bytes());
            digest.update(&buffers[index].encoded);
        }
        let (_, next_cycle) = owner.ring_position();
        if next_cycle != cycle {
            wraps += 1;
            cycle = next_cycle;
        }
    }
    let mut complete = false;
    for _ in 0..2048 {
        let status = play
            .kernel_mut()
            .step()
            .map_err(|_| "usb-hid-native-drain")?;
        if let Some(request) = play.kernel_mut().next_host_request()
            && !play
                .service_pure_call(&request)
                .map_err(|_| "usb-hid-native-pure")?
        {
            return Err("usb-hid-native-drain-call");
        }
        if status == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    if !complete || wraps < 2 {
        return Err("usb-hid-native-drain");
    }
    for (index, port) in outputs.iter().enumerate() {
        if play
            .kernel_mut()
            .output_terminal_into(&port.port_id, &mut buffers[index])
            .map_err(|_| "usb-hid-native-terminal")?
            != Some(KernelCompositeTerminal::Normal)
        {
            return Err("usb-hid-native-terminal");
        }
    }
    owner
        .cancel(node, conduit_kernel::HostCallId(0))
        .map_err(|_| "usb-hid-native-stop")?;
    let digest: [u8; 32] = digest.finalize().into();
    let mut digest_text = FixedText::new();
    for byte in digest {
        write!(digest_text, "{byte:02x}").map_err(|_| "usb-hid-native-digest")?;
    }
    let digest_hex =
        core::str::from_utf8(digest_text.as_bytes()).map_err(|_| "usb-hid-native-digest")?;
    let mut sign = FixedText::new();
    writeln!(sign, "CONDUIT_USB_HID_ENDPOINT_SIGN {{\"schema\":\"conduit.conduitos.usb-hid-endpoint/v1\",\"proof_class\":\"freestanding-emulator\",\"source_document_id\":\"{}\",\"checked_plot_id\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"device_instance_id\":\"{}\",\"transfers\":128,\"cycle_transitions\":{},\"transcript_digest\":\"{}\",\"normal_close\":true,\"acknowledged_stop\":true,\"fixture_protocol\":true,\"allocation_sealed\":true}}", plan.source_document_id.as_str(), plan.checked_plot_id.as_str(), plan.plan_id.as_str(), active.active_play_id.as_str(), device_hex, wraps, digest_hex).map_err(|_| "usb-hid-native-sign")?;
    early_write(sign.as_bytes());
    Ok(())
}
