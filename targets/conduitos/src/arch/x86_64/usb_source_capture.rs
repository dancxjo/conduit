//! Native binding of a retained HID Source kernel to Root-owned endpoint DMA.
//! The caller supplies admitted issuers; this path creates no fixture authority.
#![allow(dead_code)] // Product composition is being migrated onto this binding.
use super::{
    UsbDevice,
    endpoint_offer::ready_window_entry,
    endpoint_read::window::{
        EndpointReadWindow, EndpointReadWindowDma, EndpointReadWindowSelection,
    },
    endpoint_setup::ConfiguredInboundEndpoint,
};
use crate::{
    arch::x86_64::xhci::XhciReady,
    machine_membrane::selected_operation::SelectedOperationPlan,
    protocol_source::NativeProtocolIssuer,
    usb_base::{
        endpoint_read_contract::EndpointReadContract,
        endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
        endpoint_read_owner::EndpointReadAttachment, hid_source_kernel::PreparedHidSourceKernel,
    },
};
use alloc::vec::Vec;
use conduit_core::*;
use conduit_plan_lowering::lowering::lower_plan_fragment;

pub(super) struct PreparedSourceCapture<'a, const N: usize> {
    pub play: PreparedHidSourceKernel,
    pub owner: EndpointReadWindow<'a, N>,
}

/// Bind the actual retained Plan, configured endpoint and independently admitted
/// Root issuers. No transfer is published and no controller event is consumed.
///
/// # Safety
/// Root exclusively owns the actual configured endpoint and mapped coherent DMA.
/// Its issuers retain independently admitted authority for that resource. Root
/// must retain DMA after every failure until acknowledged hardware quiescence.
#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn bind<'a, const N: usize>(
    controller: &XhciReady,
    device: UsbDevice,
    configured: ConfiguredInboundEndpoint,
    dma: EndpointReadWindowDma<'a, N>,
    attachment: EndpointReadAttachment,
    base_id: &str,
    provider_instance_id: &str,
    mut play: PreparedHidSourceKernel,
    issuers: [NativeProtocolIssuer; N],
    work_units: u64,
) -> Result<PreparedSourceCapture<'a, N>, &'static str> {
    let contract = EndpointReadContract::prepare().map_err(|_| "usb-source-capture-contract")?;
    let ready = ready_window_entry(
        base_id,
        provider_instance_id,
        &device,
        &configured,
        &dma,
        &contract,
    )?;
    let plan = &play.kernel_mut().definition().internal_plan;
    if plan.fragments.len() != 1 {
        return Err("usb-source-capture-fragment");
    }
    let fragment = &plan.fragments[0];
    let lowered = lower_plan_fragment(fragment).map_err(|_| "usb-source-capture-lowering")?;
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let implementation = ImplementationId::from(ENDPOINT_READ_IMPLEMENTATION);
    let gears: Vec<_> = fragment
        .placements
        .iter()
        .filter(|gear| gear.implementation_id == implementation)
        .collect();
    if gears.len() != N {
        return Err("usb-source-capture-members");
    }
    let mut selections = Vec::with_capacity(N);
    for (gear, issuer) in gears.into_iter().zip(issuers) {
        let base = gear.base.as_ref().ok_or("usb-source-capture-base")?;
        if base.base_id != ready.base_id
            || base.provider_instance_id != ready.provider_instance_id
            || base.provider_generation != ready.provider_generation
            || base.implementation_id != ready.implementation_id
            || base.mechanism_family != ready.mechanism_family
            || base.enforcement_class != ready.enforcement_class
            || gear.capability_id != ready.capabilities[0].capability_id
            || gear.resources.len() != 1
            || gear.resources[0].pool_id != ready.resources[0].pool_id
        {
            return Err("usb-source-capture-readiness");
        }
        let possession = issuer
            .issue_selected(
                plan,
                &active,
                &implementation,
                &gear.placement_id,
                work_units,
            )
            .map_err(|_| "usb-source-capture-possession")?;
        selections.push(EndpointReadWindowSelection {
            table: possession.table,
            handle: possession.handle,
            claim: possession.claim,
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
        .map_err(|_| "usb-source-capture-members")?;
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
    .map_err(|_| "usb-source-capture-binding")?;
    Ok(PreparedSourceCapture { play, owner })
}
