//! Exact adapters for kernel-issued calls; protocol scheduling remains in the graph.
use crate::{
    expression_host_call::{self, ExpressionCallRefusal, ExpressionHostCall},
    i2c_base::{
        I2cProvider,
        contract::{I2C_CALL, I2cContract},
        installation::{I2C_IMPLEMENTATION, ReadyI2cBase},
        owner::{I2cCallRefusal, I2cCallSelection, I2cHostCall},
    },
    structured_selector_host_call::{self, SelectorCallRefusal, SelectorHostCall},
};
use alloc::{boxed::Box, vec::Vec};
use conduit_composite::{KernelCompositeError, KernelCompositeHost, KernelCompositeHostRequest};
use conduit_core::*;
use conduit_kernel::{
    Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome, NodeId,
};
use conduit_plan_lowering::lowering::lower_plan_fragment;

#[derive(Debug)]
pub enum ProtocolCallRefusal {
    InvalidPlan,
    Unsupported,
    Kernel(KernelCompositeError),
    Expression(ExpressionCallRefusal),
    Selector(SelectorCallRefusal),
    I2c(I2cCallRefusal),
}

enum Owner<P> {
    Expression(Box<ExpressionHostCall>),
    Selector(Box<SelectorHostCall>),
    I2c(Box<I2cHostCall<P>>),
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
        use ProtocolCallRefusal as Refusal;
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err(Refusal::InvalidPlan);
        }
        let fragment = &plan.fragments[0];
        let lowered = lower_plan_fragment(fragment).map_err(|_| Refusal::InvalidPlan)?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let mut physical = fragment
            .placements
            .iter()
            .filter(|gear| gear.implementation_id.as_str() == I2C_IMPLEMENTATION);
        let i2c = physical.next().ok_or(Refusal::Unsupported)?;
        if physical.next().is_some()
            || fragment.placements.iter().any(|gear| {
                !matches!(
                    gear.implementation_id.as_str(),
                    I2C_IMPLEMENTATION
                        | expression_host_call::IMPLEMENTATION
                        | structured_selector_host_call::IMPLEMENTATION
                        | crate::flow_zip::IMPLEMENTATION
                        | crate::current_sample::IMPLEMENTATION
                )
            })
        {
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
            let owner = match gear.implementation_id.as_str() {
                expression_host_call::IMPLEMENTATION => Owner::Expression(Box::new(
                    ExpressionHostCall::prepare(fragment, &lowered, &active, &gear.placement_id)
                        .map_err(Refusal::Expression)?,
                )),
                structured_selector_host_call::IMPLEMENTATION => Owner::Selector(Box::new(
                    SelectorHostCall::prepare(fragment, &lowered, &active, &gear.placement_id)
                        .map_err(Refusal::Selector)?,
                )),
                _ => continue,
            };
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
        let contract = I2cContract::prepare().map_err(|_| Refusal::InvalidPlan)?;
        let owner = ready
            .bind_selected(
                table,
                handle,
                claim,
                I2cCallSelection {
                    contract: &contract,
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement: &i2c.placement_id,
                },
            )
            .map_err(Refusal::I2c)?;
        bindings.push(Binding {
            node: lowered
                .identity
                .node_for_placement(&i2c.placement_id)
                .ok_or(Refusal::InvalidPlan)?,
            resources: i2c.resources.clone(),
            authorities: i2c.authority.clone(),
            owner: Owner::I2c(Box::new(owner)),
        });
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            bindings,
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
            Owner::Expression(_) => expression_host_call::CALL,
            Owner::Selector(_) => structured_selector_host_call::CALL,
            Owner::I2c(_) => I2C_CALL,
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
        let result = match &mut binding.owner {
            Owner::Expression(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map(Some)
                .map_err(Refusal::Expression),
            Owner::Selector(owner) => owner
                .invoke(node, call.call, call.request, input)
                .map_err(Refusal::Selector),
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

    /// Revoke actual bus ownership as well as cancelling kernel and computation state.
    pub fn cancel(&mut self, kernel: &mut KernelCompositeHost) -> Result<(), ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(Refusal::InvalidPlan);
        }
        // Revoke the physics-facing owner first, even if kernel cancellation later refuses.
        for binding in &mut self.bindings {
            if let Owner::I2c(owner) = &mut binding.owner {
                owner
                    .revoke(binding.node, HostCallId(0))
                    .map_err(Refusal::I2c)?;
            }
        }
        let cancelled = kernel.cancel().map_err(Refusal::Kernel);
        for binding in &mut self.bindings {
            if let Owner::Expression(owner) = &mut binding.owner {
                owner
                    .cancel(binding.node, HostCallId(0))
                    .map_err(Refusal::Expression)?;
            }
        }
        for binding in &mut self.bindings {
            if let Owner::Selector(owner) = &mut binding.owner {
                owner
                    .cancel(binding.node, HostCallId(0))
                    .map_err(Refusal::Selector)?;
            }
        }
        cancelled
    }
}

impl ProtocolCallRefusal {
    pub fn failure(&self) -> Failure {
        use ExpressionCallRefusal as Expression;
        use I2cCallRefusal as I2c;
        use SelectorCallRefusal as Selector;
        let (code, detail) = match self {
            Self::InvalidPlan => (FailureCode::InvalidLifecycle, 1020),
            Self::Unsupported => (FailureCode::HostCallDenied, 1021),
            Self::Kernel(_) => (FailureCode::HostCallFailed, 1022),
            Self::Expression(
                Expression::WrongBinding | Expression::StaleRequest | Expression::InvalidProgram,
            ) => (FailureCode::InvalidLifecycle, 1023),
            Self::Expression(Expression::InvalidInput) => (FailureCode::InvalidInput, 1024),
            Self::Expression(Expression::SequenceExhausted) => {
                (FailureCode::IdentityCapacityExhausted, 1025)
            }
            Self::Expression(Expression::Cancelled) => (FailureCode::Cancelled, 1026),
            Self::Expression(Expression::Evaluation(_)) => (FailureCode::HostCallFailed, 1027),
            Self::Selector(
                Selector::WrongBinding | Selector::InvalidSelector | Selector::StaleRequest,
            ) => (FailureCode::InvalidLifecycle, 1040),
            Self::Selector(Selector::InvalidInput) => (FailureCode::InvalidInput, 1041),
            Self::Selector(Selector::SequenceExhausted) => {
                (FailureCode::IdentityCapacityExhausted, 1042)
            }
            Self::Selector(Selector::Cancelled) => (FailureCode::Cancelled, 1043),
            Self::Selector(Selector::Selection(_)) => (FailureCode::InvalidInput, 1044),
            Self::I2c(I2c::WrongBinding | I2c::StaleRequest) => {
                (FailureCode::InvalidLifecycle, 1030)
            }
            Self::I2c(I2c::Possession | I2c::Capability(_)) => (FailureCode::HostCallDenied, 1031),
            Self::I2c(I2c::SequenceExhausted) => (FailureCode::IdentityCapacityExhausted, 1032),
            Self::I2c(I2c::Decode(_)) => (FailureCode::InvalidInput, 1033),
            Self::I2c(I2c::Result(_) | I2c::Canonical(_)) => (FailureCode::HostCallFailed, 1034),
        };
        Failure { code, detail }
    }
}
