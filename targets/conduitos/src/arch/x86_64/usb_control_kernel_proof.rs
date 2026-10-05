//! Emulator proof appliance: checked Source, one kernel, and actual selected DMA.
//! Descriptor/class interpretation remains outside this raw-transfer fixture.
//! The proof root admits finite local/remote Sign history for 64 complete calls.
//! Sustained raw ring reuse has a separate 64-transfer receipt.
use alloc::{vec, vec::Vec};
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::*;
use conduit_plan_lowering::lowering::lower_plan_fragment;
use core::fmt::Write;
use sha2::{Digest, Sha256};

use super::{
    UsbDevice,
    control_owner::{UsbControlHostCall, UsbControlSelection},
    device_dma_pointer,
};
use crate::arch::x86_64::{serial::early_write, xhci::XhciReady};
use crate::usb_base::{
    control_contract::{CONTROL_CALL, CONTROL_MAXIMUM_BYTES, ControlContract},
    control_factory::*,
    control_proof_plan::{self as planning, ControlProofSubject},
    control_request::ControlTransferRequest,
    control_result::PreparedControlResultEncoder,
};
use crate::{
    identity::{self, BootIdentities},
    sign_format::FixedText,
};

#[path = "usb_control_kernel_proof/kernel.rs"]
mod kernel;
#[path = "usb_control_kernel_proof/possession.rs"]
mod possession;

pub fn run(
    controller: &mut XhciReady,
    device: UsbDevice,
    mapping: fn(u64) -> Option<u64>,
    ids: &BootIdentities,
    base: &[u8; 32],
) -> Result<UsbDevice, &'static str> {
    let contract = ControlContract::prepare().map_err(|_| "usb-control-proof-contract")?;
    let root_port = device.root_port;
    let slot = device.slot;
    let epoch = device.attachment_epoch;
    let device_id = identity::derive_usb_device(&ids.boot, base, root_port, slot, epoch);
    let plan = planning::plan(
        &contract,
        &ControlProofSubject {
            host_id: &identity::hex(&ids.host),
            boot_id: &identity::hex(&ids.boot),
            controller_base_id: &identity::hex(base),
            device_instance_id: &identity::hex(&device_id),
            root_port,
            slot,
            attachment_epoch: epoch,
        },
    )?;
    let (mut kernel, input_port, output_port) = kernel::kernel(&plan)?;
    let (table, handle, claim) = possession::issue(&plan)?;
    let fragment = &plan.fragments[0];
    let gear = &fragment.placements[0];
    let lowered = lower_plan_fragment(fragment).map_err(|_| "usb-control-proof-lowering")?;
    let node = lowered.nodes[0].node;
    let call = conduit_kernel::HostCallId(0);
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    if kernel.active_plays().get(&fragment.host_id) != Some(&active.active_play_id) {
        return Err("usb-control-proof-play-mismatch");
    }
    let dma = device_dma_pointer(&device).map_err(|_| "usb-control-proof-attachment")?;
    let (initial_enqueue, initial_cycle) = unsafe { (*dma).control_cursor.position() };
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: input(&contract)?,
    };
    // Previous acknowledged fixture transfers left the same descriptor in DMA.
    // Prepare exact typed expectations before Play; this does not parse USB fields.
    let raw = ControlTransferRequest::new([0x80, 6, 0, 1, 0, 0, 64, 0], &[], 256)
        .map_err(|_| "usb-control-proof-request")?;
    let mut expected_encoder = PreparedControlResultEncoder::new(&contract)
        .map_err(|_| "usb-control-proof-result-schema")?;
    let expected = expected_encoder
        .completed(&raw, 18, unsafe { &(&(*dma).descriptor)[..18] })
        .map_err(|_| "usb-control-proof-result-schema")?
        .to_vec();
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(CONTROL_MAXIMUM_BYTES as usize),
    };
    let capacity = output.encoded.capacity();
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
    .map_err(|_| "usb-control-proof-native-binding")?;
    let mut transcript = Sha256::new();
    let mut cycle = initial_cycle;
    let mut transitions = 0;
    kernel.start().map_err(|_| "usb-control-proof-start")?;
    for sequence in 0..u64::from(planning::CONTROL_PROOF_TRANSFERS) {
        match kernel.admit_input(&input_port.port_id, sequence, &input) {
            Ok(conduit_kernel::scheduler::RemoteIngressOutcome::Accepted {
                sequence: accepted,
            }) if accepted == sequence => {}
            Ok(_) => return Err("usb-control-proof-input-pressure"),
            Err(error) => {
                let mut refusal = FixedText::new();
                let _ = writeln!(
                    refusal,
                    "CONDUIT_USB_KERNEL_REFUSAL phase=input sequence={sequence} error={error:?}"
                );
                early_write(refusal.as_bytes());
                return Err("usb-control-proof-input");
            }
        }
        if sequence + 1 == u64::from(planning::CONTROL_PROOF_TRANSFERS) {
            kernel
                .close_input(&input_port.port_id)
                .map_err(|_| "usb-control-proof-close")?;
        }
        let mut dispatched = false;
        let mut delivered = false;
        for _ in 0..16 {
            kernel.step().map_err(|_| "usb-control-proof-step")?;
            if let Some(request) = kernel.next_host_request() {
                if dispatched {
                    return Err("usb-control-proof-duplicate-effect");
                }
                let obligation = kernel
                    .host_request_obligation(&request)
                    .map_err(|_| "usb-control-proof-obligation")?;
                let admitted = kernel
                    .admit_host_request(
                        &request,
                        &obligation.host,
                        &obligation.resources,
                        &obligation.authorities,
                    )
                    .map_err(|_| "usb-control-proof-dispatch")?;
                let invocation = *kernel
                    .admitted_host_request_view(&admitted)
                    .map_err(|_| "usb-control-proof-dispatch")?
                    .request;
                let bytes = kernel
                    .host_request_input(&admitted)
                    .map_err(|_| "usb-control-proof-dispatch")?;
                let result = owner
                    .execute(invocation.node, invocation.call, invocation.request, bytes)
                    .map_err(|_| "usb-control-proof-native-transfer")?;
                if result != expected {
                    return Err("usb-control-proof-native-result");
                }
                transcript.update(invocation.request.0.to_le_bytes());
                transcript.update(bytes);
                transcript.update(result);
                kernel
                    .complete_host_call_bytes(&admitted, result)
                    .map_err(|_| "usb-control-proof-completion")?;
                if kernel.complete_host_call_bytes(&admitted, result).is_ok() {
                    return Err("usb-control-proof-late-completion");
                }
                dispatched = true;
            }
            if let Some(actual_sequence) = kernel
                .output_into(&output_port.port_id, &mut output)
                .map_err(|_| "usb-control-proof-output")?
            {
                if !dispatched
                    || actual_sequence != sequence
                    || output.encoded != expected
                    || output.encoded.capacity() != capacity
                {
                    return Err("usb-control-proof-output-mismatch");
                }
                kernel
                    .complete_output(&output_port.port_id, actual_sequence)
                    .map_err(|_| "usb-control-proof-output-ack")?;
                delivered = true;
                break;
            }
        }
        if !delivered {
            return Err("usb-control-proof-step-budget");
        }
        let (_, next) = unsafe { (*dma).control_cursor.position() };
        if next != cycle {
            transitions += 1;
            cycle = next;
        }
    }
    let mut complete = false;
    for _ in 0..32 {
        if kernel.step().map_err(|_| "usb-control-proof-drain")? == KernelCompositeStatus::Complete
        {
            complete = true;
            break;
        }
        if kernel.next_host_request().is_some() {
            return Err("usb-control-proof-drain-effect");
        }
    }
    if !complete
        || transitions < 4
        || kernel
            .output_terminal_into(&output_port.port_id, &mut output)
            .map_err(|_| "usb-control-proof-terminal")?
            != Some(KernelCompositeTerminal::Normal)
    {
        return Err("usb-control-proof-not-drained");
    }
    let device = owner
        .release_completed(node, call)
        .map_err(|_| "usb-control-proof-release")?;
    let (final_enqueue, final_cycle) = unsafe { (*dma).control_cursor.position() };
    let digest: [u8; 32] = transcript.finalize().into();
    let mut sign = FixedText::new();
    writeln!(sign, "CONDUIT_USB_KERNEL_SIGN {{\"schema\":\"conduit.conduitos.usb-control-kernel/v2\",\"proof_class\":\"freestanding-emulator\",\"status\":\"completed\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"controller_base_id\":\"{}\",\"device_instance_id\":\"{}\",\"source_document_id\":\"{}\",\"checked_plot_id\":\"{}\",\"expanded_plot_id\":\"{}\",\"plan_id\":\"{}\",\"fragment_id\":\"{}\",\"active_play_id\":\"{}\",\"transcript_digest\":\"{}\",\"root_port\":{},\"slot\":{},\"attachment_epoch\":{},\"transfers\":64,\"short_transfers\":64,\"additional_local_sign_items\":2048,\"additional_remote_sign_items\":256,\"cycle_transitions\":{},\"initial_enqueue\":{},\"initial_cycle\":{},\"final_enqueue\":{},\"final_cycle\":{},\"output_capacity\":{},\"dma_bytes\":8192,\"maximum_in_flight\":1,\"normal_close\":true,\"fixture_protocol\":true}}",
        identity::hex(&ids.host), identity::hex(&ids.boot), identity::hex(base), identity::hex(&device_id),
        plan.source_document_id.as_str(), plan.checked_plot_id.as_str(), plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str(), fragment.fragment_id.as_str(), active.active_play_id.as_str(), identity::hex(&digest),
        root_port, slot, epoch, transitions, initial_enqueue, initial_cycle, final_enqueue, final_cycle, capacity)
        .map_err(|_| "usb-control-proof-sign-envelope")?;
    early_write(sign.as_bytes());
    Ok(device)
}

fn input(contract: &ControlContract) -> Result<Vec<u8>, &'static str> {
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        return Err("usb-control-proof-request-schema");
    };
    let ty = |name| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .map(|field| field.value_type().clone())
            .ok_or("usb-control-proof-request-schema")
    };
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "setup",
                StructuredInfoValue::leaf(ty("setup")?, vec![128, 6, 0, 1, 0, 0, 64, 0])
                    .map_err(|_| "usb-control-proof-request-schema")?,
            )
            .map_err(|_| "usb-control-proof-request-schema")?,
            StructuredFieldValue::new(
                "output",
                StructuredInfoValue::sequence(ty("output")?, vec![])
                    .map_err(|_| "usb-control-proof-request-schema")?,
            )
            .map_err(|_| "usb-control-proof-request-schema")?,
        ],
    )
    .map_err(|_| "usb-control-proof-request-schema")?
    .canonical_bytes()
    .map_err(|_| "usb-control-proof-request-schema")
}
