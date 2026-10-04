//! Retain one admitted clock dispatch while the sole kernel owns its lifecycle.
use super::{contract::*, factory::ClockOperationFactory, installation::*, owner::*};
use crate::protocol_host_calls::ProtocolCallRefusal;
use alloc::vec::Vec;
use conduit_composite::*;
use conduit_core::*;
use conduit_kernel::{HostCallDisposition, HostCallId, HostCallOutcome, NodeId, RequestId};
use conduit_plan_lowering::lowering::lower_plan_fragment;

pub struct PreparedClockDispatch<P> {
    plan_id: PlanId,
    identity: PreparationHostIdentity,
    node: NodeId,
    resources: Vec<ResourceBinding>,
    authorities: Vec<AuthorityBinding>,
    owner: MonotonicClockHostCall<P>,
    pending: Option<(AdmittedKernelCompositeHostRequest, RequestId)>,
}
impl<P: MonotonicDeadlineProvider> PreparedClockDispatch<P> {
    pub fn prepare(
        plan: &Plan,
        ready: ReadyClockBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
    ) -> Result<Self, ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err(Refusal::InvalidPlan);
        }
        let fragment = &plan.fragments[0];
        let mut clocks = fragment
            .placements
            .iter()
            .filter(|gear| gear.implementation_id.as_str() == CLOCK_IMPLEMENTATION);
        let gear = clocks.next().ok_or(Refusal::Unsupported)?;
        if clocks.next().is_some() {
            return Err(Refusal::Unsupported);
        }
        ClockOperationFactory::prepare_contract()
            .map_err(|_| Refusal::InvalidPlan)?
            .budget(gear)
            .map_err(|_| Refusal::InvalidPlan)?;
        let lowered = lower_plan_fragment(fragment).map_err(|_| Refusal::InvalidPlan)?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let contract = MonotonicClockContract::prepare().map_err(|_| Refusal::InvalidPlan)?;
        let owner = ready
            .bind_selected(
                table,
                handle,
                claim,
                ClockCallSelection {
                    contract: &contract,
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement: &gear.placement_id,
                },
            )
            .map_err(Refusal::Clock)?;
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            node: lowered
                .identity
                .node_for_placement(&gear.placement_id)
                .ok_or(Refusal::InvalidPlan)?,
            resources: gear.resources.clone(),
            authorities: gear.authority.clone(),
            owner,
            pending: None,
        })
    }
    pub fn owns_request(
        &self,
        kernel: &KernelCompositeHost,
        request: &KernelCompositeHostRequest,
    ) -> Result<bool, ProtocolCallRefusal> {
        self.check_plan(kernel)?;
        let view = kernel
            .host_request_view(request)
            .map_err(ProtocolCallRefusal::Kernel)?;
        let obligation = kernel
            .host_request_obligation(request)
            .map_err(ProtocolCallRefusal::Kernel)?;
        let clock = obligation.requirement.contract_id.as_str() == CLOCK_CALL;
        if clock
            && (*view.child != self.identity.host_id
                || view.request.node != self.node
                || view.request.call != HostCallId(0))
        {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        Ok(clock)
    }
    pub fn dispatch(
        &mut self,
        kernel: &mut KernelCompositeHost,
        request: &KernelCompositeHostRequest,
    ) -> Result<(), ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        if self.pending.is_some() {
            return Err(Refusal::Clock(ClockCallRefusal::Pending));
        }
        if !self.owns_request(kernel, request)? {
            return Err(Refusal::Unsupported);
        }
        let admitted = kernel
            .admit_host_request(request, &self.identity, &self.resources, &self.authorities)
            .map_err(Refusal::Kernel)?;
        let call = *kernel
            .admitted_host_request_view(&admitted)
            .map_err(Refusal::Kernel)?
            .request;
        let input = kernel
            .host_request_input(&admitted)
            .map_err(Refusal::Kernel)?;
        if let Err(reason) = self.owner.start(call.node, call.call, call.request, input) {
            let error = Refusal::Clock(reason);
            complete_failure(kernel, &admitted, &error)?;
            return Err(error);
        }
        self.pending = Some((admitted, call.request));
        self.poll(kernel)
    }
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    /// One bounded provider poll. Pending means no Host Call completion is emitted.
    pub fn poll(&mut self, kernel: &mut KernelCompositeHost) -> Result<(), ProtocolCallRefusal> {
        self.check_plan(kernel)?;
        let Some((admitted, request)) = self.pending else {
            return Ok(());
        };
        // Validate the retained admission before any additional provider effect.
        let view = kernel
            .admitted_host_request_view(&admitted)
            .map_err(ProtocolCallRefusal::Kernel)?;
        if *view.child != self.identity.host_id
            || view.request.node != self.node
            || view.request.call != HostCallId(0)
            || view.request.request != request
        {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        match self.owner.poll(self.node, HostCallId(0), request) {
            Ok(None) => Ok(()),
            Ok(Some(bytes)) => {
                kernel
                    .complete_host_call_bytes(&admitted, bytes)
                    .map_err(ProtocolCallRefusal::Kernel)?;
                self.pending = None;
                Ok(())
            }
            Err(reason) => {
                let error = ProtocolCallRefusal::Clock(reason);
                complete_failure(kernel, &admitted, &error)?;
                self.pending = None;
                Err(error)
            }
        }
    }
    /// Quiesce the clock before the containing play cancels outstanding tokens.
    pub fn revoke(&mut self, kernel: &KernelCompositeHost) -> Result<(), ProtocolCallRefusal> {
        self.check_plan(kernel)?;
        self.owner
            .revoke(self.node, HostCallId(0))
            .map_err(ProtocolCallRefusal::Clock)?;
        self.pending = None;
        Ok(())
    }
    fn check_plan(&self, kernel: &KernelCompositeHost) -> Result<(), ProtocolCallRefusal> {
        let definition = kernel.definition();
        if definition.internal_plan.plan_id != self.plan_id
            || definition.host_id != self.identity.host_id
            || definition.boot_id != self.identity.boot_id
            || definition.offer_generation != self.identity.offer_generation
        {
            Err(ProtocolCallRefusal::InvalidPlan)
        } else {
            Ok(())
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
                disposition: if failure.code == conduit_kernel::FailureCode::HostCallDenied {
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

#[cfg(test)]
mod tests;
