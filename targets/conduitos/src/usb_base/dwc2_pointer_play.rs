//! Checked mouse Source over the retained, admitted DWC2 endpoint owner.
//! Root owns hardware and carries typed values; plots own report interpretation.
use super::{
    dwc2_endpoint_read::*, endpoint_read_contract::*, endpoint_read_factory::*,
    endpoint_read_owner::*, hid_source_kernel::PreparedUsbSourceKernel,
    installation::UsbNativeIdentity,
};
use crate::{
    arch::dwc2::{Controller, RegisterIo},
    machine_membrane::selected_operation::SelectedOperationPlan,
    protocol_host_calls::ProtocolCallRefusal,
    protocol_source::*,
};
use alloc::{collections::BTreeMap, vec::Vec};
use conduit_composite::{
    AdmittedKernelCompositeHostRequest, KernelCompositeHost, KernelCompositeStatus,
};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;
pub(crate) enum PointerPlayRefusal {
    Binding,
    Source(ProtocolSourceRefusal),
    Issue(NativeProtocolIssueRefusal),
    Call(ProtocolCallRefusal),
    Native(Dwc2ReadRefusal),
}
pub(crate) struct PointerPlaySchemas {
    pub initialize: StructuredInfoType,
    pub context: StructuredInfoType,
    pub update: StructuredInfoType,
}
pub(crate) struct Dwc2PointerPlay<I> {
    run: PreparedUsbSourceKernel,
    native: Dwc2EndpointReadHost<I>,
    identity: PreparationHostIdentity,
    node: NodeId,
    resources: Vec<ResourceBinding>,
    authorities: Vec<AuthorityBinding>,
    pending: Option<AdmittedKernelCompositeHostRequest>,
}
impl<I: RegisterIo> Dwc2PointerPlay<I> {
    /// # Safety
    /// Root owns the actual configured endpoint and original controller returned
    /// after settled enumeration. Its fresh issuer covers exactly this native
    /// attachment/resource generation, including the endpoint's initial DATA0.
    pub(crate) unsafe fn prepare(
        controller: Controller<I>,
        route: RootEndpointRoute,
        identity: &UsbNativeIdentity,
        issuer: NativeProtocolIssuer,
    ) -> Result<(Self, PointerPlaySchemas), PointerPlayRefusal> {
        use PointerPlayRefusal as E;
        let package = usb_hid_absolute_package().map_err(E::Source)?;
        let entry = PreparedProtocolEntry::prepare_package(package, "usb-hid-absolute-device")
            .map_err(E::Source)?;
        let schemas = PointerPlaySchemas {
            initialize: entry
                .input_schema(&port_id("initialize"))
                .ok_or(E::Binding)?,
            context: entry.input_schema(&port_id("read")).ok_or(E::Binding)?,
            update: entry.output_schema(&port_id("update")).ok_or(E::Binding)?,
        };
        let contract = EndpointReadContract::prepare().map_err(|_| E::Binding)?;
        let mut host = HostAdvertisement {
            declared_name: None,
            protocol_version: PROTOCOL_VERSION,
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            offer_generation: OfferGeneration(1),
            profile: "conduitos/native-absolute-mouse@1".into(),
            bases: alloc::vec![],
            resources: alloc::vec![],
            capabilities: alloc::vec![],
            planner_capabilities: alloc::vec![],
        };
        identity
            .append_endpoint_to_advertisement(&mut host, &contract)
            .map_err(|_| E::Binding)?;
        entry.publish_pure_backs(&mut host).map_err(E::Source)?;
        let hosts = [host];
        let placements = entry.placements(&hosts).map_err(E::Source)?;
        let artifact = entry
            .plan(
                &hosts,
                &placements,
                &["conduit.base/local@1".into()],
                conduit_planner::PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: 4096,
                    authority_grants: core::slice::from_ref(issuer.grant()),
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .map_err(E::Source)?;
        let plan = &artifact.artifact().definition().internal_plan;
        let fragment = &plan.fragments[0];
        let gear = fragment
            .placements
            .iter()
            .find(|g| g.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
            .ok_or(E::Binding)?;
        let lowered = lower_plan_fragment(fragment).map_err(|_| E::Binding)?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let node = lowered
            .identity
            .node_for_placement(&gear.placement_id)
            .ok_or(E::Binding)?;
        let resources = gear.resources.clone();
        let authorities = gear.authority.clone();
        let preparation = PreparationHostIdentity {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
        };
        let possession = issuer
            .issue(plan, &active, &ENDPOINT_READ_IMPLEMENTATION.into(), 1)
            .map_err(E::Issue)?;
        let owner = unsafe {
            EndpointReadCallOwner::bind_admitted(
                possession.table,
                possession.handle,
                possession.claim,
                EndpointReadAttachment {
                    slot: route.slot,
                    generation: route.generation,
                    endpoint_generation: route.endpoint_generation,
                    endpoint_dci: route.endpoint.endpoint * 2 + 1,
                    maximum_data_bytes: u16::from(route.endpoint.maximum_packet),
                    resource_bytes: 64,
                },
                &contract,
                SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &gear.placement_id,
                },
            )
        }
        .map_err(|e| E::Native(Dwc2ReadRefusal::Ownership(e)))?;
        let native = unsafe { Dwc2EndpointReadHost::bind_owned(controller, owner, route) }
            .map_err(E::Native)?;
        let run = PreparedUsbSourceKernel::prepare(
            artifact,
            conduit_composite::KernelCompositeSignStorage {
                additional_local_items: 60000,
                additional_remote_items: 60000,
            },
        )
        .map_err(E::Call)?;
        Ok((
            Self {
                run,
                native,
                identity: preparation,
                node,
                resources,
                authorities,
                pending: None,
            },
            schemas,
        ))
    }
    pub(crate) fn kernel_mut(&mut self) -> &mut KernelCompositeHost {
        self.run.kernel_mut()
    }
    /// One normal kernel quantum and at most one native transfer. Unsettled
    /// refusal retains both the admitted kernel token and original native lease.
    pub(crate) fn step(&mut self) -> Result<KernelCompositeStatus, PointerPlayRefusal> {
        use PointerPlayRefusal as E;
        let status = self
            .run
            .kernel_mut()
            .step()
            .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
        if let Some(request) = self.run.kernel_mut().next_host_request() {
            if self.run.service_pure_call(&request).map_err(E::Call)? {
                return Ok(status);
            }
            if self.pending.is_some() {
                return Err(E::Binding);
            }
            let kernel = self.run.kernel_mut();
            let view = kernel
                .host_request_view(&request)
                .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
            if *view.child != self.identity.host_id
                || view.request.node != self.node
                || view.request.call != HostCallId(0)
                || kernel
                    .host_request_obligation(&request)
                    .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?
                    .requirement
                    .contract_id
                    .as_str()
                    != ENDPOINT_READ_CALL
            {
                return Err(E::Binding);
            }
            self.pending = Some(
                kernel
                    .admit_host_request(
                        &request,
                        &self.identity,
                        &self.resources,
                        &self.authorities,
                    )
                    .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?,
            );
            let token = self
                .pending
                .as_ref()
                .expect("original admitted call retained");
            let call = *kernel
                .admitted_host_request_view(token)
                .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?
                .request;
            let input = kernel
                .host_request_input(token)
                .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
            let output = self
                .native
                .execute(call.node, call.call, call.request, input)
                .map_err(E::Native)?;
            kernel
                .complete_host_call_bytes(token, output)
                .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
            self.pending = None;
        }
        Ok(status)
    }
}

#[cfg(test)]
mod tests;

impl core::fmt::Debug for PointerPlayRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Binding => f.write_str("Binding"),
            Self::Source(error) => f.debug_tuple("Source").field(error).finish(),
            Self::Issue(error) => f.debug_tuple("Issue").field(error).finish(),
            Self::Call(error) => f.debug_tuple("Call").field(error).finish(),
            Self::Native(error) => f.debug_tuple("Native").field(error).finish(),
        }
    }
}
