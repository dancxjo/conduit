//! One ordinary control Play retains the original bus-scoped PIO owner.
//! The containing Root supplies reviewed wire routing and admitted possession;
//! USB descriptors, requests, class policy and retries remain in plots.
use super::{
    control_contract::*,
    control_owner::{ControlAttachment, ControlCallOwner},
    control_source_plan,
    dwc2_bus_route::BusControlReservation,
    dwc2_control::*,
    hid_source_kernel::PreparedUsbSourceKernel,
    installation::UsbNativeIdentity,
};
use crate::{
    arch::dwc2::{Controller, RegisterIo},
    machine_membrane::selected_operation::SelectedOperationPlan,
    protocol_host_calls::ProtocolCallRefusal,
    protocol_source::{NativeProtocolIssueRefusal, NativeProtocolIssuer, ProtocolSourcePackage},
};
use alloc::vec::Vec;
use conduit_composite::{
    AdmittedKernelCompositeHostRequest, KernelCompositeSignStorage, KernelCompositeStatus,
};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;
pub(crate) enum ControlPlayRefusal {
    Binding,
    Plan(control_source_plan::ControlSourcePrepareRefusal),
    Issue(NativeProtocolIssueRefusal),
    Protocol(ProtocolCallRefusal),
    Native(Dwc2ControlRefusal),
}
pub(crate) struct Dwc2ControlPlay<I> {
    run: PreparedUsbSourceKernel,
    native: Option<Dwc2ControlHost<I>>,
    identity: PreparationHostIdentity,
    resources: Vec<ResourceBinding>,
    authorities: Vec<AuthorityBinding>,
    node: NodeId,
    input_port: PortId,
    output_port: PortId,
    incoming: ValuePayload,
    result: ValuePayload,
    next_sequence: u64,
    working: bool,
    ready: bool,
    pending: Option<AdmittedKernelCompositeHostRequest>,
}
impl<I: RegisterIo> Dwc2ControlPlay<I> {
    /// # Safety
    /// Root retains this exact prepared controller, its full bus-wide control
    /// scope, actual transfer-memory reservation and electrical/address rights.
    /// `issuer` and `identity` describe those original native possessions.
    /// Per-device claims, Source recognition and copied metadata are insufficient.
    pub(crate) unsafe fn prepare_bus(
        controller: Controller<I>,
        route: RootControlRoute,
        reservation: BusControlReservation,
        identity: &UsbNativeIdentity,
        issuer: NativeProtocolIssuer,
    ) -> Result<Self, ControlPlayRefusal> {
        use ControlPlayRefusal as E;
        let package = ProtocolSourcePackage::compile(
            include_str!("../../plots/usb/control.conduit").into(),
            &[],
        )
        .map_err(|_| E::Binding)?;
        let artifact =
            control_source_plan::prepare(package, "usb-control-call", identity, issuer.grant())
                .map_err(E::Plan)?;
        let definition = artifact.artifact().definition();
        let plan = &definition.internal_plan;
        let fragment = &plan.fragments[0];
        let gear = fragment
            .placements
            .iter()
            .find(|g| {
                g.implementation_id.as_str() == super::control_factory::CONTROL_IMPLEMENTATION
            })
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
            .issue(
                plan,
                &active,
                &super::control_factory::CONTROL_IMPLEMENTATION.into(),
                1,
            )
            .map_err(E::Issue)?;
        // Supply the actual retained active identity; never infer a new Play
        // from a serialized request or use a descriptive grant as possession.
        let contract = ControlContract::prepare().map_err(|_| E::Binding)?;
        let input = definition.boundary.input_fronts[0].external_port.clone();
        let output = definition.boundary.output_fronts[0].external_port.clone();
        let selection = SelectedOperationPlan {
            fragment,
            lowered: &lowered,
            active: &active,
            placement_id: &gear.placement_id,
        };
        // SAFETY: Root owns the complete bus scope and the transfer reservation.
        let owner = unsafe {
            ControlCallOwner::bind_admitted(
                possession.table,
                possession.handle,
                possession.claim,
                ControlAttachment {
                    slot: route.slot,
                    generation: route.generation,
                    maximum_data_bytes: 256,
                    resource_bytes: 256,
                },
                &contract,
                selection,
            )
        }
        .map_err(|e| E::Native(Dwc2ControlRefusal::Ownership(e)))?;
        let native =
            unsafe { Dwc2ControlHost::bind_owned_bus(controller, owner, route, reservation) }
                .map_err(E::Native)?;
        let run = PreparedUsbSourceKernel::prepare_control(
            artifact,
            KernelCompositeSignStorage {
                additional_local_items: 8192,
                additional_remote_items: 8192,
            },
        )
        .map_err(E::Protocol)?;
        Ok(Self {
            run,
            native: Some(native),
            identity: preparation,
            resources,
            authorities,
            node,
            input_port: input.port_id,
            output_port: output.port_id,
            incoming: ValuePayload {
                value_kind: input.value_kind,
                encoded: Vec::with_capacity(CONTROL_MAXIMUM_BYTES as usize),
            },
            result: ValuePayload {
                value_kind: output.value_kind,
                encoded: Vec::with_capacity(CONTROL_MAXIMUM_BYTES as usize),
            },
            next_sequence: 0,
            working: false,
            ready: false,
            pending: None,
        })
    }
    pub(crate) fn start(&mut self) -> Result<(), ControlPlayRefusal> {
        self.run
            .kernel_mut()
            .start()
            .map(|_| ())
            .map_err(|e| ControlPlayRefusal::Protocol(ProtocolCallRefusal::Kernel(e)))
    }
    pub(crate) fn submit(
        &mut self,
        route: RootControlRoute,
        request: &[u8],
    ) -> Result<(), ControlPlayRefusal> {
        use ControlPlayRefusal as E;
        if self.working
            || self.ready
            || self.pending.is_some()
            || request.len() > CONTROL_MAXIMUM_BYTES as usize
        {
            return Err(E::Binding);
        }
        let next = self.next_sequence.checked_add(1).ok_or(E::Binding)?;
        self.native
            .as_mut()
            .ok_or(E::Binding)?
            .select_reserved_route(route)
            .map_err(E::Native)?;
        self.incoming.encoded.clear();
        self.incoming.encoded.extend_from_slice(request);
        let accepted = self
            .run
            .kernel_mut()
            .admit_input(&self.input_port, self.next_sequence, &self.incoming)
            .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?;
        if !matches!(
            accepted,
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ) {
            return Err(E::Binding);
        }
        self.next_sequence = next;
        self.working = true;
        Ok(())
    }
    /// One kernel quantum and at most one physical transfer. Any unsettled
    /// native refusal retains both the exact kernel token and original lease.
    pub(crate) fn step(&mut self) -> Result<(), ControlPlayRefusal> {
        use ControlPlayRefusal as E;
        if !self.working {
            return Ok(());
        }
        self.run
            .kernel_mut()
            .step()
            .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?;
        if let Some(request) = self.run.kernel_mut().next_host_request() {
            if self.pending.is_some() {
                return Err(E::Binding);
            }
            let kernel = self.run.kernel_mut();
            let view = kernel
                .host_request_view(&request)
                .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?;
            if *view.child != self.identity.host_id
                || view.request.node != self.node
                || view.request.call != HostCallId(0)
                || kernel
                    .host_request_obligation(&request)
                    .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?
                    .requirement
                    .contract_id
                    .as_str()
                    != CONTROL_CALL
            {
                return Err(E::Binding);
            }
            let admitted = kernel
                .admit_host_request(&request, &self.identity, &self.resources, &self.authorities)
                .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?;
            self.pending = Some(admitted);
            let token = self.pending.as_ref().expect("retained admitted call");
            let call = *kernel
                .admitted_host_request_view(token)
                .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?
                .request;
            let input = kernel
                .host_request_input(token)
                .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?;
            let output = self
                .native
                .as_mut()
                .ok_or(E::Binding)?
                .execute(call.node, call.call, call.request, input)
                .map_err(E::Native)?;
            kernel
                .complete_host_call_bytes(token, output)
                .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?;
            self.pending = None;
        }
        if let Some(sequence) = self
            .run
            .kernel_mut()
            .output_into(&self.output_port, &mut self.result)
            .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?
        {
            self.run
                .kernel_mut()
                .complete_output(&self.output_port, sequence)
                .map_err(|e| E::Protocol(ProtocolCallRefusal::Kernel(e)))?;
            self.working = false;
            self.ready = true;
        }
        Ok(())
    }
    pub(crate) fn result(&self) -> Option<&ValuePayload> {
        self.ready.then_some(&self.result)
    }
    pub(crate) fn acknowledge_result(&mut self) {
        self.ready = false;
    }
    #[allow(clippy::result_large_err)] // Preserve original custody on refusal.
    pub(crate) fn retire(mut self) -> Result<Controller<I>, (Self, ControlPlayRefusal)> {
        use ControlPlayRefusal as E;
        if self.working || self.ready || self.pending.is_some() {
            return Err((self, E::Binding));
        }
        if let Err(e) = self.run.kernel_mut().close_input(&self.input_port) {
            return Err((self, E::Protocol(ProtocolCallRefusal::Kernel(e))));
        }
        for _ in 0..16 {
            match self.run.kernel_mut().step() {
                Ok(KernelCompositeStatus::Complete) => {
                    let native = self.native.take().expect("original controller retained");
                    return match native.release_completed(self.node, HostCallId(0)) {
                        Ok(c) => Ok(c),
                        Err((native, e)) => {
                            self.native = Some(native);
                            Err((self, E::Native(e)))
                        }
                    };
                }
                Ok(_) => {}
                Err(e) => return Err((self, E::Protocol(ProtocolCallRefusal::Kernel(e)))),
            }
        }
        Err((self, E::Binding))
    }
}

#[cfg(test)]
mod tests;

impl core::fmt::Debug for ControlPlayRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Binding => f.write_str("Binding"),
            Self::Plan(error) => f.debug_tuple("Plan").field(error).finish(),
            Self::Issue(error) => f.debug_tuple("Issue").field(error).finish(),
            Self::Protocol(error) => f.debug_tuple("Protocol").field(error).finish(),
            Self::Native(error) => f.debug_tuple("Native").field(error).finish(),
        }
    }
}
