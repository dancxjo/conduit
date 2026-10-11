//! Exact adapters for kernel-issued calls; protocol scheduling remains in the graph.
use crate::{
    expression_host_call,
    i2c_base::{
        I2cProvider,
        contract::{I2C_CALL, I2cContract},
        installation::{I2C_IMPLEMENTATION, ReadyI2cBase},
        owner::{I2cCallSelection, I2cHostCall},
    },
    pure_protocol_owner::PureProtocolOwner,
    structured_selector_host_call,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_composite::{
    AdmittedKernelCompositeHostRequest, KernelCompositeHost, KernelCompositeHostRequest,
};
use conduit_core::*;
use conduit_kernel::{FailureCode, HostCallDisposition, HostCallId, HostCallOutcome, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;

pub use crate::protocol_call_refusal::ProtocolCallRefusal;

enum Owner<P> {
    Pure(PureProtocolOwner),
    I2c(Box<I2cHostCall<P>>),
    #[cfg(target_arch = "x86_64")]
    UsbControl(Box<crate::arch::NativeUsbControlCall>),
    #[cfg(target_arch = "x86_64")]
    UsbProtocolControl,
    #[cfg(target_arch = "x86_64")]
    UsbProtocolEndpoint,
}
struct Binding<P> {
    node: NodeId,
    resources: Vec<ResourceBinding>,
    authorities: Vec<AuthorityBinding>,
    owner: Owner<P>,
}
/// Admitted native owners and exact dispatch bindings prepared before Play.
pub struct PreparedProtocolCalls<P> {
    plan_id: PlanId,
    identity: PreparationHostIdentity,
    bindings: Vec<Binding<P>>,
    #[cfg(target_arch = "x86_64")]
    usb_protocol: Option<crate::arch::NativeUsbProtocolHost>,
    #[cfg(target_arch = "x86_64")]
    pending_usb: Option<AdmittedKernelCompositeHostRequest>,
    #[cfg(target_arch = "x86_64")]
    pending_usb_failed: bool,
}
impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub fn prepare(
        plan: &Plan,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_with_joins(
            plan,
            ready,
            table,
            handle,
            claim,
            &crate::flow_zip::FlowZipOperationFactory::default(),
        )
    }

    pub(crate) fn prepare_with_joins(
        plan: &Plan,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_with_operations(
            plan,
            ready,
            table,
            handle,
            claim,
            zip,
            &crate::seeded_state::SeededStateOperationFactory::default(),
        )
    }

    pub(crate) fn prepare_with_operations(
        plan: &Plan,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            plan,
            I2C_IMPLEMENTATION,
            zip,
            states,
            |fragment, lowered, active, placement| {
                let contract =
                    I2cContract::prepare().map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
                ready
                    .bind_selected(
                        table,
                        handle,
                        claim,
                        I2cCallSelection {
                            contract: &contract,
                            fragment,
                            lowered,
                            active,
                            placement,
                        },
                    )
                    .map(|owner| Owner::I2c(Box::new(owner)))
                    .map_err(ProtocolCallRefusal::I2c)
            },
        )
    }

    #[cfg(target_arch = "x86_64")]
    pub(crate) fn prepare_usb(
        plan: &Plan,
        ready: crate::arch::ReadyUsbControlBase,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            plan,
            crate::usb_base::control_factory::CONTROL_IMPLEMENTATION,
            zip,
            states,
            |fragment, lowered, active, placement| {
                let contract = crate::usb_base::control_contract::ControlContract::prepare()
                    .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
                ready
                    .bind_selected(
                        table,
                        handle,
                        claim,
                        crate::arch::UsbControlSelection {
                            contract: &contract,
                            fragment,
                            lowered,
                            active,
                            placement,
                        },
                    )
                    .map(|owner| Owner::UsbControl(Box::new(owner)))
                    .map_err(ProtocolCallRefusal::Usb)
            },
        )
    }

    #[cfg(target_arch = "x86_64")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_usb_protocol(
        plan: &Plan,
        ready: crate::arch::ReadyUsbProtocolHost,
        control_table: BaseCapabilityTable,
        control_handle: BaseCapabilityHandle,
        control_claim: BaseOperationClaim,
        endpoint_table: BaseCapabilityTable,
        endpoint_handle: BaseCapabilityHandle,
        endpoint_claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        use crate::usb_base::{
            control_factory::CONTROL_IMPLEMENTATION,
            endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
        };
        use ProtocolCallRefusal as Refusal;
        let (lowered, active, mut bindings) = Self::prepare_pure(
            plan,
            &[CONTROL_IMPLEMENTATION, ENDPOINT_READ_IMPLEMENTATION],
            zip,
            states,
        )?;
        let fragment = &plan.fragments[0];
        let control = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == CONTROL_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let endpoint = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let control_contract = crate::usb_base::control_contract::ControlContract::prepare()
            .map_err(|_| Refusal::InvalidPlan)?;
        let endpoint_contract =
            crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare()
                .map_err(|_| Refusal::InvalidPlan)?;
        let host = ready
            .bind_selected(
                control_table,
                control_handle,
                control_claim,
                crate::arch::UsbControlSelection {
                    contract: &control_contract,
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement: &control.placement_id,
                },
                endpoint_table,
                endpoint_handle,
                endpoint_claim,
                &endpoint_contract,
                crate::machine_membrane::selected_operation::SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &endpoint.placement_id,
                },
            )
            .map_err(Refusal::UsbProtocol)?;
        for (gear, owner) in [
            (control, Owner::UsbProtocolControl),
            (endpoint, Owner::UsbProtocolEndpoint),
        ] {
            bindings.push(Binding {
                node: lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(Refusal::InvalidPlan)?,
                resources: gear.resources.clone(),
                authorities: gear.authority.clone(),
                owner,
            });
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            bindings,
            usb_protocol: Some(host),
            pending_usb: None,
            #[cfg(target_arch = "x86_64")]
            pending_usb_failed: false,
        })
    }

    fn prepare_pure(
        plan: &Plan,
        implementations: &[&str],
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<
        (
            conduit_plan_lowering::lowering::LoweredPlanFragment,
            ActivePlayIdentity,
            Vec<Binding<P>>,
        ),
        ProtocolCallRefusal,
    > {
        use ProtocolCallRefusal as Refusal;
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err(Refusal::InvalidPlan);
        }
        states
            .validate_plan(plan)
            .map_err(|_| Refusal::InvalidPlan)?;
        let fragment = &plan.fragments[0];
        let lowered = lower_plan_fragment(fragment).map_err(|_| Refusal::InvalidPlan)?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        for implementation in implementations {
            if fragment
                .placements
                .iter()
                .filter(|gear| gear.implementation_id.as_str() == *implementation)
                .count()
                != 1
            {
                return Err(Refusal::Unsupported);
            }
        }
        if fragment.placements.iter().any(|gear| {
            !implementations.contains(&gear.implementation_id.as_str())
                && !matches!(
                    gear.implementation_id.as_str(),
                    expression_host_call::IMPLEMENTATION
                        | structured_selector_host_call::IMPLEMENTATION
                        | crate::flow_zip::IMPLEMENTATION
                        | crate::flow_concat_finite::IMPLEMENTATION
                        | crate::flow_merge_finite::IMPLEMENTATION
                        | crate::seeded_state::IMPLEMENTATION
                        | crate::current_sample::IMPLEMENTATION
                        | crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION
                )
        }) {
            return Err(Refusal::Unsupported);
        }
        // Validate pure typed storage before binding the physical owner.
        zip.validate_plan(plan).map_err(|_| Refusal::InvalidPlan)?;
        for gear in &fragment.placements {
            if gear.implementation_id.as_str() == crate::current_sample::IMPLEMENTATION {
                use conduit_composite::KernelOperationFactory;
                crate::current_sample::CurrentSampleOperationFactory::default()
                    .budget(gear)
                    .map_err(|_| Refusal::InvalidPlan)?;
            }
        }
        let mut bindings = Vec::with_capacity(fragment.placements.len());
        for gear in &fragment.placements {
            let Some(owner) = PureProtocolOwner::prepare(fragment, &lowered, &active, gear)? else {
                continue;
            };
            bindings.push(Binding {
                node: lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(Refusal::InvalidPlan)?,
                resources: gear.resources.clone(),
                authorities: gear.authority.clone(),
                owner: Owner::Pure(owner),
            });
        }
        Ok((lowered, active, bindings))
    }

    fn prepare_selected(
        plan: &Plan,
        implementation: &str,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
        bind: impl FnOnce(
            &PlanFragment,
            &conduit_plan_lowering::lowering::LoweredPlanFragment,
            &ActivePlayIdentity,
            &PlacementId,
        ) -> Result<Owner<P>, ProtocolCallRefusal>,
    ) -> Result<Self, ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        let (lowered, active, mut bindings) =
            Self::prepare_pure(plan, &[implementation], zip, states)?;
        let fragment = &plan.fragments[0];
        let i2c = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == implementation)
            .ok_or(Refusal::Unsupported)?;
        let owner = bind(fragment, &lowered, &active, &i2c.placement_id)?;
        bindings.push(Binding {
            node: lowered
                .identity
                .node_for_placement(&i2c.placement_id)
                .ok_or(Refusal::InvalidPlan)?,
            resources: i2c.resources.clone(),
            authorities: i2c.authority.clone(),
            owner,
        });
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            bindings,
            #[cfg(target_arch = "x86_64")]
            usb_protocol: None,
            #[cfg(target_arch = "x86_64")]
            pending_usb: None,
            #[cfg(target_arch = "x86_64")]
            pending_usb_failed: false,
        })
    }

    /// Service exactly one surfaced token. No request creation, retry or private stepping loop.
    pub fn dispatch(
        &mut self,
        kernel: &mut KernelCompositeHost,
        request: &KernelCompositeHostRequest,
    ) -> Result<(), ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(Refusal::InvalidPlan);
        }
        #[cfg(target_arch = "x86_64")]
        if self.pending_usb.is_some() {
            return Err(Refusal::Unsupported);
        }
        let view = kernel.host_request_view(request).map_err(Refusal::Kernel)?;
        if *view.child != self.identity.host_id || view.request.call != HostCallId(0) {
            return Err(Refusal::InvalidPlan);
        }
        let node = view.request.node;
        let binding = self
            .bindings
            .iter_mut()
            .find(|binding| binding.node == node)
            .ok_or(Refusal::Unsupported)?;
        let required = kernel
            .host_request_obligation(request)
            .map_err(Refusal::Kernel)?;
        let expected = match binding.owner {
            Owner::Pure(ref owner) => owner.contract(),
            Owner::I2c(_) => I2C_CALL,
            #[cfg(target_arch = "x86_64")]
            Owner::UsbControl(_) | Owner::UsbProtocolControl => {
                crate::usb_base::control_contract::CONTROL_CALL
            }
            #[cfg(target_arch = "x86_64")]
            Owner::UsbProtocolEndpoint => {
                crate::usb_base::endpoint_read_contract::ENDPOINT_READ_CALL
            }
        };
        if required.requirement.contract_id.as_str() != expected {
            return Err(Refusal::InvalidPlan);
        }
        let admitted = kernel
            .admit_host_request(
                request,
                &self.identity,
                &binding.resources,
                &binding.authorities,
            )
            .map_err(Refusal::Kernel)?;
        let call = *kernel
            .admitted_host_request_view(&admitted)
            .map_err(Refusal::Kernel)?
            .request;
        let input = kernel
            .host_request_input(&admitted)
            .map_err(Refusal::Kernel)?;
        #[cfg(target_arch = "x86_64")]
        if matches!(binding.owner, Owner::UsbProtocolEndpoint) {
            let result = self
                .usb_protocol
                .as_mut()
                .ok_or(Refusal::InvalidPlan)?
                .endpoint_begin(node, call.call, call.request, input)
                .map_err(Refusal::UsbProtocol);
            match result {
                Ok(()) => {
                    self.pending_usb = Some(admitted);
                    return Ok(());
                }
                Err(error) => {
                    Self::complete_failure(kernel, &admitted, &error)?;
                    return Err(error);
                }
            }
        }
        let result = match &mut binding.owner {
            #[cfg(target_arch = "x86_64")]
            Owner::UsbProtocolControl => self
                .usb_protocol
                .as_mut()
                .ok_or(Refusal::InvalidPlan)?
                .control_invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::UsbProtocol),
            #[cfg(target_arch = "x86_64")]
            Owner::UsbProtocolEndpoint => return Err(Refusal::InvalidPlan),
            #[cfg(target_arch = "x86_64")]
            Owner::UsbControl(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::Usb),
            Owner::Pure(owner) => owner.invoke(node, call.call, call.request, input),
            Owner::I2c(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::I2c),
        };
        match result {
            Ok(None) => kernel
                .complete_host_call(
                    &admitted,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .map_err(Refusal::Kernel),
            Ok(Some(bytes)) => kernel
                .complete_host_call_bytes(&admitted, bytes)
                .map_err(Refusal::Kernel),
            Err(error) => {
                let failure = error.failure();
                kernel
                    .complete_host_call(
                        &admitted,
                        HostCallOutcome {
                            disposition: if failure.code == FailureCode::HostCallDenied {
                                HostCallDisposition::Denied
                            } else {
                                HostCallDisposition::Failed
                            },
                            output: None,
                            failure: Some(failure),
                        },
                    )
                    .map_err(Refusal::Kernel)?;
                Err(error)
            }
        }
    }

    fn complete_failure(
        kernel: &mut KernelCompositeHost,
        admitted: &AdmittedKernelCompositeHostRequest,
        error: &ProtocolCallRefusal,
    ) -> Result<(), ProtocolCallRefusal> {
        let failure = error.failure();
        kernel
            .complete_host_call(
                admitted,
                HostCallOutcome {
                    disposition: if failure.code == FailureCode::HostCallDenied {
                        HostCallDisposition::Denied
                    } else {
                        HostCallDisposition::Failed
                    },
                    output: None,
                    failure: Some(failure),
                },
            )
            .map_err(ProtocolCallRefusal::Kernel)
    }

    /// Poll at most one native event for the original admitted endpoint token.
    pub fn poll_pending(
        &mut self,
        kernel: &mut KernelCompositeHost,
    ) -> Result<bool, ProtocolCallRefusal> {
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(admitted) = self.pending_usb {
            if self.pending_usb_failed {
                return Err(ProtocolCallRefusal::Unsupported);
            }
            let result = self
                .usb_protocol
                .as_mut()
                .ok_or(ProtocolCallRefusal::InvalidPlan)?
                .endpoint_poll()
                .map_err(ProtocolCallRefusal::UsbProtocol);
            match result {
                Ok(None) => {}
                Ok(Some(bytes)) => {
                    kernel
                        .complete_host_call_bytes(&admitted, bytes)
                        .map_err(ProtocolCallRefusal::Kernel)?;
                    self.pending_usb = None;
                }
                Err(error) => {
                    self.pending_usb_failed = true;
                    Self::complete_failure(kernel, &admitted, &error)?;
                    return Err(error);
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Revoke actual bus ownership as well as cancelling kernel and computation state.
    pub fn cancel(&mut self, kernel: &mut KernelCompositeHost) -> Result<(), ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(Refusal::InvalidPlan);
        }
        #[cfg(target_arch = "x86_64")]
        if let Some(host) = &mut self.usb_protocol {
            host.cancel().map_err(Refusal::UsbProtocol)?;
            self.pending_usb = None;
            self.pending_usb_failed = false;
        }
        // Revoke the physics-facing owner first, even if kernel cancellation later refuses.
        for binding in &mut self.bindings {
            #[cfg(target_arch = "x86_64")]
            if let Owner::UsbControl(owner) = &mut binding.owner {
                owner
                    .revoke(binding.node, HostCallId(0))
                    .map_err(Refusal::Usb)?;
            }
            if let Owner::I2c(owner) = &mut binding.owner {
                owner
                    .revoke(binding.node, HostCallId(0))
                    .map_err(Refusal::I2c)?;
            }
        }
        let cancelled = kernel.cancel().map_err(Refusal::Kernel);
        for binding in &mut self.bindings {
            if let Owner::Pure(owner) = &mut binding.owner {
                owner.cancel(binding.node, HostCallId(0))?;
            }
        }
        cancelled
    }
}
