//! Source-owned class setup in the explicitly selected endpoint proof appliance.
//! This fixture Root grants one control operation; it offers no ordinary input.
use super::{
    UsbDevice,
    control_owner::{UsbControlHostCall, UsbControlSelection},
    kernel_proof::possession,
};
use crate::{
    arch::x86_64::{serial::early_write, xhci::XhciReady},
    identity::{self, BootIdentities},
    sign_format::FixedText,
    usb_base::{
        control_contract::{CONTROL_CALL, ControlContract},
        control_factory::CONTROL_IMPLEMENTATION,
        control_proof_plan::ControlProofSubject,
        device_probe_proof_kernel::PreparedDeviceProbeKernel,
    },
};
use alloc::{vec, vec::Vec};
use conduit_composite::{
    KernelCompositeSignStorage, KernelCompositeStatus, KernelCompositeTerminal,
};
use conduit_core::{ValuePayload, bind_active_play, validate_canonical_structured_value};
use conduit_kernel::HostCallId;
use conduit_plan_lowering::lowering::lower_plan_fragment;
use core::fmt::Write;
use sha2::{Digest, Sha256};

pub(super) fn run(
    controller: &mut XhciReady,
    device: UsbDevice,
    mapping: fn(u64) -> Option<u64>,
    ids: &BootIdentities,
    base: &[u8; 32],
) -> Result<UsbDevice, &'static str> {
    let interface = device.interfaces[0];
    if device.interface_count != 1
        || interface.class != 3
        || interface.subclass != 1
        || !matches!(interface.protocol, 1 | 2)
        || interface.alternate_setting != 0
    {
        return Err("usb-hid-boot-fixture-interface");
    }
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
    let mut proof = PreparedDeviceProbeKernel::prepare_hid_boot(
        &subject,
        KernelCompositeSignStorage {
            additional_local_items: 256,
            additional_remote_items: 32,
        },
    )
    .map_err(|_| "usb-hid-boot-preparation")?;
    let plan = proof.plan().clone();
    let fragment = &plan.fragments[0];
    let gear = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == CONTROL_IMPLEMENTATION)
        .ok_or("usb-hid-boot-selection")?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| "usb-hid-boot-lowering")?;
    let node = lowered
        .identity
        .node_for_placement(&gear.placement_id)
        .ok_or("usb-hid-boot-node")?;
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    if proof.kernel.active_plays().get(&fragment.host_id) != Some(&active.active_play_id) {
        return Err("usb-hid-boot-play");
    }
    let (table, handle, claim) = possession::issue_bounded(&plan, 1)?;
    let contract = ControlContract::prepare().map_err(|_| "usb-hid-boot-contract")?;
    let input_port = proof.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output_port = proof.kernel.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: vec![interface.number],
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
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
    .map_err(|_| "usb-hid-boot-native-owner")?;
    let mut digest = Sha256::new();
    let mut calls = 0;
    let mut seen = false;
    let mut complete = false;
    proof.kernel.start().map_err(|_| "usb-hid-boot-start")?;
    proof
        .kernel
        .admit_input(&input_port.port_id, 0, &input)
        .map_err(|_| "usb-hid-boot-input")?;
    proof
        .kernel
        .close_input(&input_port.port_id)
        .map_err(|_| "usb-hid-boot-close")?;
    for _ in 0..128 {
        let status = proof.kernel.step().map_err(|_| "usb-hid-boot-step")?;
        if let Some(request) = proof.kernel.next_host_request()
            && !proof
                .dispatch_pure(&request)
                .map_err(|_| "usb-hid-boot-pure-call")?
        {
            let obligation = proof
                .kernel
                .host_request_obligation(&request)
                .map_err(|_| "usb-hid-boot-obligation")?;
            if obligation.requirement.contract_id.as_str() != CONTROL_CALL || calls != 0 {
                return Err("usb-hid-boot-foreign-or-duplicate-call");
            }
            let admitted = proof
                .kernel
                .admit_host_request(
                    &request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .map_err(|_| "usb-hid-boot-admission")?;
            let call = *proof
                .kernel
                .admitted_host_request_view(&admitted)
                .map_err(|_| "usb-hid-boot-request")?
                .request;
            let bytes = proof
                .kernel
                .host_request_input(&admitted)
                .map_err(|_| "usb-hid-boot-request")?;
            digest.update(bytes);
            let result = owner
                .execute(call.node, call.call, call.request, bytes)
                .map_err(|_| "usb-hid-boot-native-call")?;
            digest.update(result);
            proof
                .kernel
                .complete_host_call_bytes(&admitted, result)
                .map_err(|_| "usb-hid-boot-completion")?;
            calls += 1;
        }
        if let Some(sequence) = proof
            .kernel
            .output_into(&output_port.port_id, &mut output)
            .map_err(|_| "usb-hid-boot-output")?
        {
            if seen
                || sequence != 0
                || validate_canonical_structured_value(&output.encoded)
                    .map_err(|_| "usb-hid-boot-result")?
                    .variant_payload("ready")
                    .map_err(|_| "usb-hid-boot-result")?
                    .is_none()
            {
                return Err("usb-hid-boot-not-ready");
            }
            proof
                .kernel
                .complete_output(&output_port.port_id, sequence)
                .map_err(|_| "usb-hid-boot-output-ack")?;
            seen = true;
        }
        if status == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    if !complete
        || !seen
        || calls != 1
        || proof
            .kernel
            .output_terminal_into(&output_port.port_id, &mut output)
            .map_err(|_| "usb-hid-boot-terminal")?
            != Some(KernelCompositeTerminal::Normal)
    {
        return Err("usb-hid-boot-not-drained");
    }
    let device = owner
        .release_completed(node, HostCallId(0))
        .map_err(|_| "usb-hid-boot-release")?;
    let digest: [u8; 32] = digest.finalize().into();
    let mut sign = FixedText::new();
    writeln!(sign, "CONDUIT_USB_HID_BOOT_SIGN {{\"schema\":\"conduit.conduitos.usb-hid-boot-selection/v1\",\"proof_class\":\"freestanding-emulator\",\"status\":\"ready\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"controller_base_id\":\"{}\",\"device_instance_id\":\"{}\",\"source_document_id\":\"{}\",\"checked_plot_id\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"attachment_epoch\":{},\"interface_number\":{},\"transfers\":1,\"transcript_digest\":\"{}\",\"normal_close\":true,\"quiescent_release\":true,\"fixture_appliance\":true,\"ordinary_class_offer\":false}}",
        host, boot, base_id, device_id, plan.source_document_id.as_str(), plan.checked_plot_id.as_str(),
        plan.plan_id.as_str(), active.active_play_id.as_str(), subject.attachment_epoch,
        interface.number, identity::hex(&digest)).map_err(|_| "usb-hid-boot-sign")?;
    early_write(sign.as_bytes());
    Ok(device)
}
