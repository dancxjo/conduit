//! Shared prepared computation owners; no protocol policy or kernel stepping.
use crate::{
    expression_host_call::{self, ExpressionHostCall, ExpressionPreparation},
    protocol_call_refusal::ProtocolCallRefusal,
    structured_selector_host_call::{self, SelectorHostCall},
};
use alloc::boxed::Box;
use conduit_core::{ActivePlayIdentity, PlanFragment, PlannedGear};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

/// One immutable expression-validation context per fragment preparation.
pub(crate) struct PureProtocolPreparation<'a> {
    fragment: &'a PlanFragment,
    lowered: &'a LoweredPlanFragment,
    active: &'a ActivePlayIdentity,
    expressions: Option<ExpressionPreparation<'a>>,
}
impl<'a> PureProtocolPreparation<'a> {
    pub(crate) fn new(
        fragment: &'a PlanFragment,
        lowered: &'a LoweredPlanFragment,
        active: &'a ActivePlayIdentity,
    ) -> Result<Self, ProtocolCallRefusal> {
        let expressions = fragment
            .placements
            .iter()
            .any(|gear| gear.implementation_id.as_str() == expression_host_call::IMPLEMENTATION)
            .then(|| ExpressionPreparation::new(fragment, lowered, active))
            .transpose()
            .map_err(ProtocolCallRefusal::Expression)?;
        Ok(Self {
            fragment,
            lowered,
            active,
            expressions,
        })
    }
    pub(crate) fn owners(
        &self,
    ) -> impl Iterator<Item = Result<(&'a PlannedGear, PureProtocolOwner), ProtocolCallRefusal>> + '_
    {
        self.fragment
            .placements
            .iter()
            .filter_map(|gear| match self.prepare(gear) {
                Ok(Some(owner)) => Some(Ok((gear, owner))),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
    }
    pub(crate) fn prepare(
        &self,
        gear: &PlannedGear,
    ) -> Result<Option<PureProtocolOwner>, ProtocolCallRefusal> {
        Ok(match gear.implementation_id.as_str() {
            expression_host_call::IMPLEMENTATION => {
                let owner = if let Some(context) = &self.expressions {
                    context.prepare(&gear.placement_id)
                } else {
                    ExpressionHostCall::prepare(
                        self.fragment,
                        self.lowered,
                        self.active,
                        &gear.placement_id,
                    )
                }
                .map_err(ProtocolCallRefusal::Expression)?;
                Some(PureProtocolOwner::Expression(Box::new(owner)))
            }
            structured_selector_host_call::IMPLEMENTATION => {
                Some(PureProtocolOwner::Selector(Box::new(
                    SelectorHostCall::prepare(
                        self.fragment,
                        self.lowered,
                        self.active,
                        &gear.placement_id,
                    )
                    .map_err(ProtocolCallRefusal::Selector)?,
                )))
            }
            _ => None,
        })
    }
}

pub(crate) enum PureProtocolOwner {
    Expression(Box<ExpressionHostCall>),
    Selector(Box<SelectorHostCall>),
}
impl PureProtocolOwner {
    #[cfg(test)]
    pub(crate) fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
        gear: &PlannedGear,
    ) -> Result<Option<Self>, ProtocolCallRefusal> {
        PureProtocolPreparation::new(fragment, lowered, active)?.prepare(gear)
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
