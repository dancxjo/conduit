//! Checked descriptor exchange over actual selected native control machinery.
//! The appliance retains legacy attachment setup; this is not class acceptance.
use super::{
    UsbDevice,
    control_owner::{UsbControlHostCall, UsbControlSelection},
    device_dma_pointer,
    kernel_proof::possession,
};
use crate::{
    arch::x86_64::{serial::early_write, xhci::XhciReady},
    identity::{self, BootIdentities},
    sign_format::FixedText,
    usb_base::{
        control_contract::{CONTROL_CALL, ControlContract},
        control_factory::CONTROL_IMPLEMENTATION,
        control_proof_plan::{CONTROL_PROOF_TRANSFERS, ControlProofSubject},
        device_probe_proof_kernel::PreparedDeviceProbeKernel,
    },
};
use alloc::{vec, vec::Vec};
use conduit_composite::{
    KernelCompositeSignStorage, KernelCompositeStatus, KernelCompositeTerminal,
};
use conduit_core::{ValuePayload, bind_active_play, validate_canonical_structured_value};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use core::fmt::Write;
use sha2::{Digest, Sha256};

pub fn run(
    controller: &mut XhciReady,
    device: UsbDevice,
    mapping: fn(u64) -> Option<u64>,
    ids: &BootIdentities,
    base: &[u8; 32],
) -> Result<UsbDevice, &'static str> {
    let host = identity::hex(&ids.host);
    let boot = identity::hex(&ids.boot);
    let base_id = identity::hex(base);
    let device_id = identity::hex(&identity::derive_usb_device(
        &ids.boot,
        base,
        device.root_port,
        device.slot,
        device.attachment_epoch,
    ));
    let subject = ControlProofSubject {
        host_id: &host,
        boot_id: &boot,
        controller_base_id: &base_id,
        device_instance_id: &device_id,
        root_port: device.root_port,
        slot: device.slot,
        attachment_epoch: device.attachment_epoch,
    };
    let mut proof = PreparedDeviceProbeKernel::prepare(
        &subject,
        KernelCompositeSignStorage {
            additional_local_items: 4096,
            additional_remote_items: 512,
        },
    )
    .map_err(|_| "usb-device-probe-preparation")?;
    let plan = proof.plan().clone();
    let fragment = &plan.fragments[0];
    let gear = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == CONTROL_IMPLEMENTATION)
        .ok_or("usb-device-probe-selection")?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| "usb-device-probe-lowering")?;
    let node = lowered
        .identity
        .placements
        .iter()
        .find(|(_, id)| id == &gear.placement_id)
        .ok_or("usb-device-probe-node")?
        .0;
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    if proof.kernel.active_plays().get(&fragment.host_id) != Some(&active.active_play_id) {
        return Err("usb-device-probe-play");
    }
    let (table, handle, claim) = possession::issue(&plan)?;
    let contract = ControlContract::prepare().map_err(|_| "usb-device-probe-contract")?;
    let input_port = proof.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let ports: Vec<_> = proof
        .kernel
        .definition()
        .boundary
        .output_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    if ports.len() != 2 {
        return Err("usb-device-probe-fronts");
    }
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: vec![],
    };
    let mut outputs: Vec<_> = ports
        .iter()
        .map(|port| ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: Vec::with_capacity(4096),
        })
        .collect();
    let dma = device_dma_pointer(&device).map_err(|_| "usb-device-probe-attachment")?;
    let initial = unsafe { (*dma).control_cursor.position() };
    let mut owner = unsafe {
        UsbControlHostCall::bind_selected(
            controller,
            device,
            mapping,
            table,
            handle,
            claim,
            UsbControlSelection {
                contract: &contract,
                fragment,
                lowered: &lowered,
                active: &active,
                placement: &gear.placement_id,
            },
        )
    }
    .map_err(|_| "usb-device-probe-native-owner")?;
    let mut digest = Sha256::new();
    let mut calls = 0_u16;
    let mut cycles = 0_u16;
    let mut last_cycle = initial.1;
    proof.kernel.start().map_err(|_| "usb-device-probe-start")?;
    for sequence in 0..u64::from(CONTROL_PROOF_TRANSFERS) {
        match proof
            .kernel
            .admit_input(&input_port.port_id, sequence, &input)
            .map_err(|_| "usb-device-probe-input")?
        {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: actual }
                if actual == sequence => {}
            _ => return Err("usb-device-probe-input-pressure"),
        }
        if sequence + 1 == u64::from(CONTROL_PROOF_TRANSFERS) {
            proof
                .kernel
                .close_input(&input_port.port_id)
                .map_err(|_| "usb-device-probe-close")?;
        }
        let mut seen = [false; 2];
        for _ in 0..128 {
            proof.kernel.step().map_err(|_| "usb-device-probe-step")?;
            if let Some(request) = proof.kernel.next_host_request() {
                if !proof
                    .dispatch_pure(&request)
                    .map_err(|_| "usb-device-probe-pure-call")?
                {
                    let obligation = proof
                        .kernel
                        .host_request_obligation(&request)
                        .map_err(|_| "usb-device-probe-obligation")?;
                    if obligation.requirement.contract_id.as_str() != CONTROL_CALL {
                        return Err("usb-device-probe-foreign-call");
                    }
                    let admitted = proof
                        .kernel
                        .admit_host_request(
                            &request,
                            &obligation.host,
                            &obligation.resources,
                            &obligation.authorities,
                        )
                        .map_err(|_| "usb-device-probe-admission")?;
                    let call = *proof
                        .kernel
                        .admitted_host_request_view(&admitted)
                        .map_err(|_| "usb-device-probe-request")?
                        .request;
                    let bytes = proof
                        .kernel
                        .host_request_input(&admitted)
                        .map_err(|_| "usb-device-probe-request")?;
                    digest.update(bytes);
                    let result = owner
                        .execute(call.node, call.call, call.request, bytes)
                        .map_err(|_| "usb-device-probe-native-call")?;
                    digest.update(result);
                    if let Err(error) = proof.kernel.complete_host_call_bytes(&admitted, result) {
                        let mut refusal = FixedText::new();
                        let _ = writeln!(
                            refusal,
                            "CONDUIT_USB_DEVICE_PROBE_REFUSAL phase=completion sequence={sequence} error={error:?}"
                        );
                        early_write(refusal.as_bytes());
                        return Err("usb-device-probe-completion");
                    }
                    calls += 1;
                    if u64::from(calls) != sequence + 1 {
                        return Err("usb-device-probe-duplicate-transfer");
                    }
                }
            }
            for (index, port) in ports.iter().enumerate() {
                if let Some(actual) = proof
                    .kernel
                    .output_into(&port.port_id, &mut outputs[index])
                    .map_err(|_| "usb-device-probe-output")?
                {
                    if actual != sequence
                        || seen[index]
                        || outputs[index].encoded.capacity() != 4096
                    {
                        return Err("usb-device-probe-output-bounds");
                    }
                    let tag = match port.port_id.as_str() {
                        "observed" => "frame",
                        "decoded" => "device",
                        _ => return Err("usb-device-probe-output-port"),
                    };
                    if validate_canonical_structured_value(&outputs[index].encoded)
                        .map_err(|_| "usb-device-probe-output-value")?
                        .variant_payload(tag)
                        .map_err(|_| "usb-device-probe-output-value")?
                        .is_none()
                    {
                        return Err("usb-device-probe-output-refusal");
                    }
                    digest.update(sequence.to_le_bytes());
                    digest.update(port.port_id.as_str().as_bytes());
                    digest.update(&outputs[index].encoded);
                    proof
                        .kernel
                        .complete_output(&port.port_id, actual)
                        .map_err(|_| "usb-device-probe-output-ack")?;
                    seen[index] = true;
                }
            }
            if seen == [true, true] {
                break;
            }
        }
        if seen != [true, true] {
            return Err("usb-device-probe-output-missing");
        }
        let next = unsafe { (*dma).control_cursor.position() }.1;
        if next != last_cycle {
            cycles += 1;
            last_cycle = next;
        }
    }
    let mut complete = false;
    for _ in 0..32 {
        if proof.kernel.step().map_err(|_| "usb-device-probe-drain")?
            == KernelCompositeStatus::Complete
        {
            complete = true;
            break;
        }
    }
    if !complete || cycles < 4 {
        return Err("usb-device-probe-terminal");
    }
    for (index, port) in ports.iter().enumerate() {
        if proof
            .kernel
            .output_terminal_into(&port.port_id, &mut outputs[index])
            .map_err(|_| "usb-device-probe-terminal")?
            != Some(KernelCompositeTerminal::Normal)
        {
            return Err("usb-device-probe-terminal");
        }
    }
    let final_position = unsafe { (*dma).control_cursor.position() };
    let device = owner
        .release_completed(node, conduit_kernel::HostCallId(0))
        .map_err(|_| "usb-device-probe-release")?;
    let mut sign = FixedText::new();
    writeln!(sign, "CONDUIT_USB_DEVICE_PROBE_SIGN {{\"schema\":\"conduit.conduitos.usb-device-probe/v1\",\"proof_class\":\"freestanding-emulator\",\"status\":\"completed\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"controller_base_id\":\"{}\",\"device_instance_id\":\"{}\",\"source_document_id\":\"{}\",\"checked_plot_id\":\"{}\",\"expanded_plot_id\":\"{}\",\"plan_id\":\"{}\",\"fragment_id\":\"{}\",\"active_play_id\":\"{}\",\"transcript_digest\":\"{:x}\",\"root_port\":{},\"slot\":{},\"attachment_epoch\":{},\"transfers\":{},\"decoded\":64,\"observed\":64,\"cycle_transitions\":{},\"initial_enqueue\":{},\"initial_cycle\":{},\"final_enqueue\":{},\"final_cycle\":{},\"additional_local_sign_items\":4096,\"additional_remote_sign_items\":512,\"output_capacity\":4096,\"dma_bytes\":8192,\"maximum_in_flight\":1,\"normal_close\":true,\"protocol_owned_by_source\":true,\"legacy_attachment_setup\":true,\"fixture_appliance\":true}}",
        host, boot, base_id, device_id, plan.source_document_id.as_str(), plan.checked_plot_id.as_str(), plan.expanded_plot_id.as_str(), plan.plan_id.as_str(), fragment.fragment_id.as_str(), active.active_play_id.as_str(), digest.finalize(), subject.root_port, subject.slot, subject.attachment_epoch, calls, cycles, initial.0, initial.1, final_position.0, final_position.1).map_err(|_| "usb-device-probe-sign")?;
    early_write(sign.as_bytes());
    Ok(device)
}
