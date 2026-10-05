//! Shared prepared computation owners; no protocol policy or kernel stepping.
use crate::{
    expression_host_call::{self, ExpressionHostCall},
    protocol_call_refusal::ProtocolCallRefusal,
    structured_selector_host_call::{self, SelectorHostCall},
};
use alloc::boxed::Box;
use conduit_core::{ActivePlayIdentity, PlanFragment, PlannedGear};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(crate) enum PureProtocolOwner {
    Expression(Box<ExpressionHostCall>),
    Selector(Box<SelectorHostCall>),
}
impl PureProtocolOwner {
    pub(crate) fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
        gear: &PlannedGear,
    ) -> Result<Option<Self>, ProtocolCallRefusal> {
        Ok(match gear.implementation_id.as_str() {
            expression_host_call::IMPLEMENTATION => Some(Self::Expression(Box::new(
                ExpressionHostCall::prepare(fragment, lowered, active, &gear.placement_id)
                    .map_err(ProtocolCallRefusal::Expression)?,
            ))),
            structured_selector_host_call::IMPLEMENTATION => Some(Self::Selector(Box::new(
                SelectorHostCall::prepare(fragment, lowered, active, &gear.placement_id)
                    .map_err(ProtocolCallRefusal::Selector)?,
            ))),
            _ => None,
        })
    }

    pub(crate) fn contract(&self) -> &'static str {
        match self {
            Self::Expression(_) => expression_host_call::CALL,
            Self::Selector(_) => structured_selector_host_call::CALL,
        }
    }

    pub(crate) fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<Option<&[u8]>, ProtocolCallRefusal> {
        match self {
            Self::Expression(owner) => owner
                .invoke(node, call, request, input)
                .map(Some)
                .map_err(ProtocolCallRefusal::Expression),
            Self::Selector(owner) => owner
                .invoke(node, call, request, input)
                .map_err(ProtocolCallRefusal::Selector),
        }
    }

    pub(crate) fn cancel(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), ProtocolCallRefusal> {
        match self {
            Self::Expression(owner) => owner
                .cancel(node, call)
                .map_err(ProtocolCallRefusal::Expression),
            Self::Selector(owner) => owner
                .cancel(node, call)
                .map_err(ProtocolCallRefusal::Selector),
        }
    }
}
