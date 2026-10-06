//! Native eight-capture binding for the explicitly selected proof appliance.
//! Actual configuration establishes readiness; the proof Root issues possession.
use super::*;
use crate::arch::x86_64::usb::{
    endpoint_offer::ready_window_entry,
    endpoint_read::window::{
        EndpointReadWindow, EndpointReadWindowDma, EndpointReadWindowSelection,
    },
    endpoint_setup::ConfiguredInboundEndpoint,
};
use crate::usb_base::{hid_endpoint_proof_plan, hid_source_kernel::PreparedHidSourceKernel};

pub(super) struct PreparedNativeCapture<'a> {
    pub play: PreparedHidSourceKernel,
    pub owner: EndpointReadWindow<'a, 8>,
    pub plan: Plan,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub device_id: [u8; 32],
}

/// # Safety
/// Root owns the actual configured endpoint and coherent mapped DMA, retained
/// after every error until acknowledged hardware quiescence. This function
/// neither rings a doorbell nor consumes controller events.
pub(super) unsafe fn prepare<'a>(
    controller: &XhciReady,
    device: UsbDevice,
    configured: ConfiguredInboundEndpoint,
    dma: EndpointReadWindowDma<'a, 8>,
    ids: &BootIdentities,
    base: &[u8; 32],
) -> Result<PreparedNativeCapture<'a>, &'static str> {
    let contract = EndpointReadContract::prepare().map_err(|_| "usb-hid-capture-contract")?;
    let device_id = identity::derive_usb_device(
        &ids.boot,
        base,
        device.root_port,
        device.slot,
        device.attachment_epoch,
    );
    let base_hex = identity::hex(base);
    let device_hex = identity::hex(&device_id);
    let ready = ready_window_entry(
        &base_hex,
        &device_hex,
        &device,
        &configured,
        &dma,
        &contract,
    )?;
    let artifact = hid_endpoint_proof_plan::plan(
        &EndpointReadProofSubject {
            host_id: &identity::hex(&ids.host),
            boot_id: &identity::hex(&ids.boot),
            controller_base_id: &base_hex,
            device_instance_id: &device_hex,
            root_port: device.root_port,
            slot: device.slot,
            attachment_epoch: device.attachment_epoch,
            endpoint_dci: configured.dci,
            endpoint_epoch: configured.endpoint_epoch,
        },
        "usb-hid-keyboard-capture-window",
    )
    .map_err(|_| "usb-hid-capture-plan")?;
    let definition = artifact.artifact().definition();
    let plan = definition.internal_plan.clone();
    let inputs = definition.external_capability.inputs.clone();
    let outputs = definition.external_capability.outputs.clone();
    let fragment = &plan.fragments[0];
    let lowered = lower_plan_fragment(fragment).map_err(|_| "usb-hid-capture-lowering")?;
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let mut selections = Vec::with_capacity(8);
    for gear in fragment
        .placements
        .iter()
        .filter(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
    {
        let selected_base = gear.base.as_ref().ok_or("usb-hid-capture-base")?;
        if selections.len() == 8
            || selected_base.base_id != ready.base_id
            || selected_base.provider_instance_id != ready.provider_instance_id
            || selected_base.provider_generation != ready.provider_generation
            || selected_base.implementation_id != ready.implementation_id
            || selected_base.mechanism_family != ready.mechanism_family
            || selected_base.enforcement_class != ready.enforcement_class
            || gear.capability_id != ready.capabilities[0].capability_id
            || gear.resources.len() != 1
            || gear.resources[0].pool_id != ready.resources[0].pool_id
        {
            return Err("usb-hid-capture-readiness");
        }
        // Deterministic independent keys are confined to this proof appliance.
        // Ordinary Root installation must use its cryptographic issuer instead.
        let (table, handle, claim) = possession::issue_selected(
            &plan,
            &gear.placement_id,
            [71 + selections.len() as u8; 32],
        )?;
        selections.push(EndpointReadWindowSelection {
            table,
            handle,
            claim,
            selected: SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &gear.placement_id,
            },
        });
    }
    let selections = selections
        .try_into()
        .map_err(|_| "usb-hid-capture-members")?;
    let attachment = EndpointReadAttachment {
        slot: device.slot,
        generation: u64::from(device.attachment_epoch),
        endpoint_generation: configured.endpoint_epoch,
        endpoint_dci: configured.dci,
        maximum_data_bytes: 2048,
        resource_bytes: core::mem::size_of::<ProofDma>() as u64,
    };
    // Finish every fallible allocation before binding the retained native owner.
    let play =
        PreparedHidSourceKernel::prepare(artifact, planning::ENDPOINT_READ_PROOF_SIGN_STORAGE)
            .map_err(|_| "usb-hid-capture-kernel")?;
    let owner = unsafe {
        EndpointReadWindow::bind_selected(
            controller.maximum_ports(),
            device,
            attachment,
            configured,
            dma,
            selections,
            &contract,
        )
    }
    .map_err(|_| "usb-hid-capture-binding")?;
    Ok(PreparedNativeCapture {
        play,
        owner,
        plan,
        inputs,
        outputs,
        device_id,
    })
}
