//! Exact local Source execution without physical owners or invented possession.
use crate::{
    current_sample::CurrentSampleOperationFactory,
    expression_host_call::ExpressionOperationFactory, protocol_host_calls::ProtocolCallRefusal,
    protocol_operations::ProtocolOperations, pure_protocol_owner::PureProtocolOwner,
    structured_selector_host_call::SelectorOperationFactory,
};
use alloc::vec::Vec;
use conduit_composite::{
    KernelCompositeDefinition, KernelCompositeHost, KernelCompositeSignStorage,
    KernelCompositeStatus, KernelCompositeTerminal, KernelOperationRegistry,
};
use conduit_core::*;
use conduit_kernel::{FailureCode, HostCallDisposition, HostCallId, HostCallOutcome, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;

/// Owns the sole execution kernel and its already-prepared computation owners.
/// Physical calls, authority/resource cords, remote Lines and activation are
/// outside this local profile and refused before execution storage is prepared.
pub struct PreparedPureProtocolPlay {
    kernel: KernelCompositeHost,
    owners: Vec<(NodeId, PureProtocolOwner)>,
    identity: PreparationHostIdentity,
}
impl PreparedPureProtocolPlay {
    pub(crate) fn prepare(
        definition: KernelCompositeDefinition,
        operations: ProtocolOperations,
        storage: KernelCompositeSignStorage,
    ) -> Result<Self, ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        crate::protocol_play::validate_fore(&definition)?;
        let plan = &definition.internal_plan;
        let fragment = &plan.fragments[0];
        if !plan.activations.is_empty()
            || !plan.activation_preparations.is_empty()
            || !fragment.execution_fusions.is_empty()
            || fragment.connections.iter().any(|cord| {
                cord.track != ConnectionTrack::Payload
                    || cord.resource.is_some()
                    || cord.selected_line.is_some()
                    || !cord.admitted_lines.is_empty()
            })
            || fragment.placements.iter().any(|gear| {
                gear.base.is_some()
                    || !gear.resources.is_empty()
                    || !gear.authority.is_empty()
                    || !matches!(
                        gear.implementation_id.as_str(),
                        crate::expression_host_call::IMPLEMENTATION
                            | crate::structured_selector_host_call::IMPLEMENTATION
                            | crate::flow_zip::IMPLEMENTATION
                            | crate::flow_concat_finite::IMPLEMENTATION
                            | crate::flow_merge_finite::IMPLEMENTATION
                            | crate::seeded_state::IMPLEMENTATION
                            | crate::current_sample::IMPLEMENTATION
                    )
            })
        {
            return Err(Refusal::Unsupported);
        }
        operations
            .joins
            .validate_plan(plan)
            .map_err(|_| Refusal::InvalidPlan)?;
        operations
            .states
            .validate_plan(plan)
            .map_err(|_| Refusal::InvalidPlan)?;
        operations
            .concats
            .validate_plan(plan)
            .map_err(|_| Refusal::InvalidPlan)?;
        operations
            .merges
            .validate_plan(plan)
            .map_err(|_| Refusal::InvalidPlan)?;
        let lowered = lower_plan_fragment(fragment).map_err(|_| Refusal::InvalidPlan)?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let identity = PreparationHostIdentity {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
        };
        let mut owners = Vec::with_capacity(fragment.placements.len());
        for gear in &fragment.placements {
            if let Some(owner) = PureProtocolOwner::prepare(fragment, &lowered, &active, gear)? {
                let node = lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(Refusal::InvalidPlan)?;
                owners.push((node, owner));
            } else if !gear.host_calls.is_empty() {
                return Err(Refusal::Unsupported);
            }
        }
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(ExpressionOperationFactory::default())
            .map_err(|_| Refusal::InvalidPlan)?;
        registry
            .install(SelectorOperationFactory::default())
            .map_err(|_| Refusal::InvalidPlan)?;
        registry
            .install(operations.joins)
            .map_err(|_| Refusal::InvalidPlan)?;
        registry
            .install(operations.states)
            .map_err(|_| Refusal::InvalidPlan)?;
        registry
            .install(operations.concats)
            .map_err(|_| Refusal::InvalidPlan)?;
        registry
            .install(operations.merges)
            .map_err(|_| Refusal::InvalidPlan)?;
        registry
            .install(CurrentSampleOperationFactory::default())
            .map_err(|_| Refusal::InvalidPlan)?;
        let kernel = KernelCompositeHost::prepare_with_sign_storage(definition, &registry, storage)
            .map_err(Refusal::Kernel)?;
        Ok(Self {
            kernel,
            owners,
            identity,
        })
    }

    /// Immutable inspection cannot replace the kernel behind its retained owners.
    pub fn kernel(&self) -> &KernelCompositeHost {
        &self.kernel
    }

    pub fn start(&mut self) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .start()
            .map(|_| ())
            .map_err(ProtocolCallRefusal::Kernel)
    }

    /// Advance once, then service at most one kernel-issued computation call.
    pub fn step(&mut self) -> Result<KernelCompositeStatus, ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        let status = self.kernel.step().map_err(Refusal::Kernel)?;
        if let Some(request) = self.kernel.next_host_request() {
            let view = self
                .kernel
                .host_request_view(&request)
                .map_err(Refusal::Kernel)?;
            if *view.child != self.identity.host_id || view.request.call != HostCallId(0) {
                return Err(Refusal::InvalidPlan);
            }
            let node = view.request.node;
            let (_, owner) = self
                .owners
                .iter_mut()
                .find(|(actual, _)| *actual == node)
                .ok_or(Refusal::Unsupported)?;
            let obligation = self
                .kernel
                .host_request_obligation(&request)
                .map_err(Refusal::Kernel)?;
            if obligation.requirement.contract_id.as_str() != owner.contract() {
                return Err(Refusal::InvalidPlan);
            }
            let admitted = self
                .kernel
                .admit_host_request(&request, &self.identity, &[], &[])
                .map_err(Refusal::Kernel)?;
            let call = *self
                .kernel
                .admitted_host_request_view(&admitted)
                .map_err(Refusal::Kernel)?
                .request;
            let input = self
                .kernel
                .host_request_input(&admitted)
                .map_err(Refusal::Kernel)?;
            match owner.invoke(node, call.call, call.request, input) {
                Ok(Some(bytes)) => self
                    .kernel
                    .complete_host_call_bytes(&admitted, bytes)
                    .map_err(Refusal::Kernel)?,
                Ok(None) => self
                    .kernel
                    .complete_host_call(
                        &admitted,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .map_err(Refusal::Kernel)?,
                Err(error) => {
                    let failure = error.failure();
                    self.kernel
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
                    return Err(error);
                }
            }
        }
        Ok(status)
    }

    pub fn admit_input(
        &mut self,
        port: &PortId,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<conduit_kernel::scheduler::RemoteIngressOutcome, ProtocolCallRefusal> {
        self.kernel
            .admit_input(port, sequence, value)
            .map_err(ProtocolCallRefusal::Kernel)
    }
    pub fn close_input(&mut self, port: &PortId) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .close_input(port)
            .map_err(ProtocolCallRefusal::Kernel)
    }
    pub fn output_into(
        &mut self,
        port: &PortId,
        value: &mut ValuePayload,
    ) -> Result<Option<u64>, ProtocolCallRefusal> {
        self.kernel
            .output_into(port, value)
            .map_err(ProtocolCallRefusal::Kernel)
    }
    pub fn complete_output(
        &mut self,
        port: &PortId,
        sequence: u64,
    ) -> Result<(), ProtocolCallRefusal> {
        self.kernel
            .complete_output(port, sequence)
            .map_err(ProtocolCallRefusal::Kernel)
    }
    pub fn output_terminal_into(
        &mut self,
        port: &PortId,
        value: &mut ValuePayload,
    ) -> Result<Option<KernelCompositeTerminal>, ProtocolCallRefusal> {
        self.kernel
            .output_terminal_into(port, value)
            .map_err(ProtocolCallRefusal::Kernel)
    }
    pub fn cancel(&mut self) -> Result<(), ProtocolCallRefusal> {
        let cancelled = self.kernel.cancel().map_err(ProtocolCallRefusal::Kernel);
        for (node, owner) in &mut self.owners {
            owner.cancel(*node, HostCallId(0))?;
        }
        cancelled
    }
}
