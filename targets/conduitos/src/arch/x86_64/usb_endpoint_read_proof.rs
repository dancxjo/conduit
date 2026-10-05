//! Explicit raw endpoint proof appliance; no report decoding or class policy.
#![allow(dead_code)] // Invoked only by the explicitly selected raw endpoint proof appliance.
use super::{
    UsbDevice,
    endpoint_read::{EndpointReceiveDma, UsbEndpointReadHostCall},
    endpoint_setup::{InboundEndpointParameters, configure_inbound},
};
use crate::{
    arch::x86_64::{serial::early_write, xhci::XhciReady},
    identity::{self, BootIdentities},
    machine_membrane::selected_operation::SelectedOperationPlan,
    sign_format::FixedText,
    usb_base::{
        endpoint_read_contract::*,
        endpoint_read_factory::*,
        endpoint_read_owner::EndpointReadAttachment,
        endpoint_read_proof_plan::{self as planning, EndpointReadProofSubject},
        endpoint_ring::EndpointRingCursor,
    },
};
use alloc::{vec, vec::Vec};
use conduit_composite::{KernelCompositeStatus, KernelCompositeTerminal};
use conduit_core::*;
use conduit_plan_lowering::lowering::lower_plan_fragment;
use core::fmt::Write;
use sha2::{Digest, Sha256};
#[path = "usb_endpoint_read_proof/hid.rs"]
mod hid;
#[path = "usb_endpoint_read_proof/kernel.rs"]
mod kernel;
#[path = "usb_endpoint_read_proof/possession.rs"]
mod possession;

#[repr(C, align(4096))]
struct ProofDma {
    ring: [[u32; 4]; 64],
    buffer: [u8; 2048],
    input: [u8; 2112],
    cursor: Option<EndpointRingCursor>,
}
static mut PROOF_DMA: ProofDma = ProofDma {
    ring: [[0; 4]; 64],
    buffer: [0; 2048],
    input: [0; 2112],
    cursor: None,
};

pub fn run_appliance(
    controller: &mut XhciReady,
    device: UsbDevice,
    mapping: fn(u64) -> Option<u64>,
    ids: &BootIdentities,
    base: &[u8; 32],
) -> Result<(), &'static str> {
    let endpoint = device.endpoints[0];
    if device.endpoint_count != 1
        || endpoint.address != 0x81
        || endpoint.transfer_type != 3
        || endpoint.maximum_packet_size != 8
    {
        return Err("usb-endpoint-proof-fixture-attachment");
    }
    let parameters = InboundEndpointParameters {
        address: endpoint.address,
        transfer_type: endpoint.transfer_type,
        packet_field: endpoint.maximum_packet_size,
        interval: endpoint.interval,
    };
    run(controller, device, mapping, ids, base, parameters)
}

/// Only the explicitly selected proof appliance invokes this Root grant recipe.
/// Endpoint fields are fixture input, not discovery-derived authority.
pub(super) fn run(
    controller: &mut XhciReady,
    device: UsbDevice,
    mapping: fn(u64) -> Option<u64>,
    ids: &BootIdentities,
    base: &[u8; 32],
    parameters: InboundEndpointParameters,
) -> Result<(), &'static str> {
    let contract = EndpointReadContract::prepare().map_err(|_| "usb-endpoint-proof-contract")?;
    let root_port = device.root_port;
    let slot = device.slot;
    let epoch = device.attachment_epoch;
    let dci = ((parameters.address & 15) << 1) | 1;
    let endpoint_epoch = 1;
    let device_id = identity::derive_usb_device(&ids.boot, base, root_port, slot, epoch);
    let storage = core::ptr::addr_of_mut!(PROOF_DMA);
    let virtual_start = storage as u64;
    let physical = mapping(virtual_start).ok_or("usb-endpoint-proof-mapping")?;
    if physical & 4095 != 0 {
        return Err("usb-endpoint-proof-mapping");
    }
    for offset in (0..core::mem::size_of::<ProofDma>()).step_by(4096) {
        if mapping(virtual_start + offset as u64) != physical.checked_add(offset as u64) {
            return Err("usb-endpoint-proof-mapping");
        }
    }
    // Root retains this static allocation beyond all error returns. It cannot be
    // cleared/rebound after timeout, configuration uncertainty or pending DMA.
    let storage = unsafe { &mut *storage };
    if storage.cursor.is_some() {
        return Err("usb-endpoint-proof-already-owned");
    }
    storage.cursor = Some(EndpointRingCursor::new(64).map_err(|_| "usb-endpoint-proof-ring")?);
    let mut dma = EndpointReceiveDma {
        ring: &mut storage.ring,
        buffer: &mut storage.buffer,
        cursor: storage.cursor.as_mut().unwrap(),
        ring_physical: physical + core::mem::offset_of!(ProofDma, ring) as u64,
        buffer_physical: physical + core::mem::offset_of!(ProofDma, buffer) as u64,
    };
    let configured = unsafe {
        configure_inbound(
            controller,
            &device,
            parameters,
            endpoint_epoch,
            &mut storage.input,
            physical + core::mem::offset_of!(ProofDma, input) as u64,
            &mut dma,
        )
    }
    .map_err(|_| "usb-endpoint-proof-configuration")?;
    if cfg!(feature = "usb-hid-endpoint-proof") {
        return hid::run(controller, device, configured, dma, ids, base);
    }
    // Publish readiness and issue possession only after native configuration is acknowledged.
    let plan = planning::plan(
        &contract,
        &EndpointReadProofSubject {
            host_id: &identity::hex(&ids.host),
            boot_id: &identity::hex(&ids.boot),
            controller_base_id: &identity::hex(base),
            device_instance_id: &identity::hex(&device_id),
            root_port,
            slot,
            attachment_epoch: epoch,
            endpoint_dci: dci,
            endpoint_epoch,
        },
    )?;
    let (mut kernel, input_port, output_port) = kernel::kernel(&plan)?;
    let (table, handle, claim) = possession::issue(&plan)?;
    let fragment = &plan.fragments[0];
    let gear = &fragment.placements[0];
    let lowered = lower_plan_fragment(fragment).map_err(|_| "usb-endpoint-proof-lowering")?;
    let node = lowered.nodes[0].node;
    let call = conduit_kernel::HostCallId(0);
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    if kernel.active_plays().get(&fragment.host_id) != Some(&active.active_play_id) {
        return Err("usb-endpoint-proof-play");
    }
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
    .map_err(|_| "usb-endpoint-proof-binding")?;
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: input(&contract)?,
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let capacity = output.encoded.capacity();
    let mut transcript = Sha256::new();
    let mut wraps = 0;
    let mut cycle = 1;
    kernel.start().map_err(|_| "usb-endpoint-proof-start")?;
    for sequence in 0..u64::from(planning::ENDPOINT_READ_PROOF_TRANSFERS) {
        kernel
            .admit_input(&input_port.port_id, sequence, &input)
            .map_err(|_| "usb-endpoint-proof-input")?;
        if sequence + 1 == u64::from(planning::ENDPOINT_READ_PROOF_TRANSFERS) {
            kernel
                .close_input(&input_port.port_id)
                .map_err(|_| "usb-endpoint-proof-close")?;
        }
        let mut request = None;
        for _ in 0..16 {
            kernel.step().map_err(|_| "usb-endpoint-proof-step")?;
            if let Some(next) = kernel.next_host_request() {
                request = Some(next);
                break;
            }
        }
        let request = request.ok_or("usb-endpoint-proof-step-budget")?;
        let obligation = kernel
            .host_request_obligation(&request)
            .map_err(|_| "usb-endpoint-proof-obligation")?;
        let admitted = kernel
            .admit_host_request(
                &request,
                &obligation.host,
                &obligation.resources,
                &obligation.authorities,
            )
            .map_err(|_| "usb-endpoint-proof-admission")?;
        let invocation = *kernel
            .admitted_host_request_view(&admitted)
            .map_err(|_| "usb-endpoint-proof-invocation")?
            .request;
        owner
            .begin(
                invocation.node,
                invocation.call,
                invocation.request,
                kernel
                    .host_request_input(&admitted)
                    .map_err(|_| "usb-endpoint-proof-call")?,
            )
            .map_err(|_| "usb-endpoint-proof-submit")?;
        let mut marker = FixedText::new();
        writeln!(marker, "CONDUIT_USB_ENDPOINT_READY {sequence}")
            .map_err(|_| "usb-endpoint-proof-sign")?;
        early_write(marker.as_bytes());
        let mut completed = false;
        for _ in 0..4_000_000 {
            match owner.poll().map_err(|_| "usb-endpoint-proof-completion")? {
                Some(bytes) => {
                    transcript.update(sequence.to_le_bytes());
                    transcript.update((bytes.len() as u32).to_le_bytes());
                    transcript.update(bytes);
                    kernel
                        .complete_host_call_bytes(&admitted, bytes)
                        .map_err(|_| "usb-endpoint-proof-call-result")?;
                    completed = true;
                    break;
                }
                None => core::hint::spin_loop(),
            }
        }
        if !completed {
            return Err("usb-endpoint-proof-timeout-retained-dma");
        }
        let mut delivered = false;
        for _ in 0..16 {
            kernel
                .step()
                .map_err(|_| "usb-endpoint-proof-output-step")?;
            if let Some(actual_sequence) = kernel
                .output_into(&output_port.port_id, &mut output)
                .map_err(|_| "usb-endpoint-proof-output")?
            {
                if actual_sequence != sequence || output.encoded.capacity() != capacity {
                    return Err("usb-endpoint-proof-output-order");
                }
                let result = validate_canonical_structured_value(&output.encoded)
                    .map_err(|_| "usb-endpoint-proof-value")?;
                let frame = result
                    .variant_payload("completed")
                    .map_err(|_| "usb-endpoint-proof-value")?
                    .ok_or("usb-endpoint-proof-value")?;
                if frame
                    .record_field("wire")
                    .map_err(|_| "usb-endpoint-proof-value")?
                    .ok_or("usb-endpoint-proof-value")?
                    .primitive_bytes("value/bytes")
                    .map_err(|_| "usb-endpoint-proof-value")?
                    .len()
                    != 8
                {
                    return Err("usb-endpoint-proof-extent");
                }
                kernel
                    .complete_output(&output_port.port_id, sequence)
                    .map_err(|_| "usb-endpoint-proof-ack")?;
                delivered = true;
                break;
            }
        }
        if !delivered {
            return Err("usb-endpoint-proof-delivery");
        }
        let (_, next_cycle) = owner.ring_position();
        if next_cycle != cycle {
            wraps += 1;
            cycle = next_cycle;
        }
    }
    let mut complete = false;
    for _ in 0..32 {
        if kernel.step().map_err(|_| "usb-endpoint-proof-drain")? == KernelCompositeStatus::Complete
        {
            complete = true;
            break;
        }
    }
    if !complete
        || wraps < 2
        || kernel
            .output_terminal_into(&output_port.port_id, &mut output)
            .map_err(|_| "usb-endpoint-proof-terminal")?
            != Some(KernelCompositeTerminal::Normal)
    {
        return Err("usb-endpoint-proof-terminal");
    }
    owner
        .cancel(node, call)
        .map_err(|_| "usb-endpoint-proof-stop")?;
    let digest: [u8; 32] = transcript.finalize().into();
    let mut sign = FixedText::new();
    writeln!(sign, "CONDUIT_USB_ENDPOINT_SIGN {{\"schema\":\"conduit.conduitos.usb-endpoint-read/v1\",\"proof_class\":\"freestanding-emulator\",\"status\":\"completed\",\"source_document_id\":\"{}\",\"checked_plot_id\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"device_instance_id\":\"{}\",\"attachment_epoch\":{},\"endpoint_epoch\":{},\"endpoint_dci\":{},\"transfers\":128,\"cycle_transitions\":{},\"transcript_digest\":\"{}\",\"normal_close\":true,\"acknowledged_stop\":true,\"fixture_protocol\":true}}", plan.source_document_id.as_str(), plan.checked_plot_id.as_str(), plan.plan_id.as_str(), active.active_play_id.as_str(), identity::hex(&device_id), epoch, endpoint_epoch, dci, wraps, identity::hex(&digest)).map_err(|_| "usb-endpoint-proof-sign")?;
    early_write(sign.as_bytes());
    Ok(())
}

fn input(contract: &EndpointReadContract) -> Result<Vec<u8>, &'static str> {
    StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "length",
                StructuredInfoValue::leaf(
                    StructuredInfoType::leaf(kind_id("value/u64"))
                        .map_err(|_| "usb-endpoint-proof-request")?,
                    8_u64.to_le_bytes().to_vec(),
                )
                .map_err(|_| "usb-endpoint-proof-request")?,
            )
            .map_err(|_| "usb-endpoint-proof-request")?,
        ],
    )
    .map_err(|_| "usb-endpoint-proof-request")?
    .canonical_bytes()
    .map_err(|_| "usb-endpoint-proof-request")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_root_recipe_prepares_the_same_checked_source_and_exact_endpoint_possession() {
        let contract = EndpointReadContract::prepare().unwrap();
        let mut subject = EndpointReadProofSubject {
            host_id: "fixture/host",
            boot_id: "fixture/boot",
            controller_base_id: "fixture/controller",
            device_instance_id: "fixture/device",
            root_port: 1,
            slot: 2,
            attachment_epoch: 3,
            endpoint_dci: 3,
            endpoint_epoch: 4,
        };
        let first = planning::plan(&contract, &subject).unwrap();
        assert!(verify_plan(&first));
        let (kernel, _, _) = kernel::kernel(&first).unwrap();
        let (_, _, claim) = possession::issue(&first).unwrap();
        assert_eq!(claim.plan_id, first.plan_id);
        assert_eq!(
            kernel.active_plays().get(&first.fragments[0].host_id),
            Some(&claim.active_play_id)
        );
        assert_eq!(claim.operation_contract_id.as_str(), ENDPOINT_READ_CALL);
        subject.endpoint_epoch += 1;
        let second = planning::plan(&contract, &subject).unwrap();
        assert_eq!(first.source_document_id, second.source_document_id);
        assert_eq!(first.checked_plot_id, second.checked_plot_id);
        assert_ne!(first.plan_id, second.plan_id);
        assert_ne!(
            first.fragments[0].placements[0].resources[0].pool_id,
            second.fragments[0].placements[0].resources[0].pool_id
        );
        subject.endpoint_epoch = 0;
        assert!(planning::plan(&contract, &subject).is_err());
        subject.endpoint_epoch = 1;
        subject.endpoint_dci = 4;
        assert!(planning::plan(&contract, &subject).is_err());
    }
}
