//! Native realization of the existing pure `when` Kind, with no protocol policy.
use crate::expression_host_call::{
    ExpressionCallRefusal, ExpressionHostCall, ExpressionPreparation,
};
use alloc::{boxed::Box, format, string::String, vec, vec::Vec};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostCallId, HostedValueStore, NodeId, RequestId, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_plot::PortableExpressionProgram;
pub const IMPLEMENTATION: &str = "conduitos/kernel-pure-filter@1";
pub const PROFILE: &str = "conduitos/pure-filter-cooperative-bounded@1";
pub const ARTIFACT: &str = "conduit-plot/pure-filter@1";
pub const CALL: &str = "conduit.host/pure-filter@1";
pub fn offer(
    program: &PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, StructuredInfoRefusal> {
    let kind = conduit_semantic_catalog::pure_filter_contract(program, temporal)?;
    let id = kind.kind_id.clone();
    let bytes = program
        .maximum_prepared_input_bytes()
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    let mut limits = kind.limits.clone();
    limits.max_active_instances = limits.max_active_instances.min(16);
    limits.max_queue_bytes = limits.max_queue_bytes.min(bytes.max(1));
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: format!("conduitos/{}", id.as_str()).into(),
            execution_profile_id: PROFILE.into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: ARTIFACT.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: CALL.into(),
                target_kind: Some(id),
                maximum_in_flight: 1,
                maximum_input_bytes: bytes.max(1),
                maximum_output_bytes: bytes.max(1),
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .narrow_capacity(limits)
    .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?
    .build())
}
pub(crate) struct FilterHostCall {
    predicate: ExpressionHostCall,
    retained: Vec<u8>,
}
impl FilterHostCall {
    pub(crate) fn prepare(
        context: &ExpressionPreparation<'_>,
        placement: &PlacementId,
    ) -> Result<Self, ExpressionCallRefusal> {
        let predicate = context.prepare_filter(placement)?;
        let retained = Vec::with_capacity(predicate.maximum_input_bytes() as usize);
        Ok(Self {
            predicate,
            retained,
        })
    }
    pub(crate) fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<Option<&[u8]>, ExpressionCallRefusal> {
        let accepted = self.predicate.invoke(node, call, request, input)?;
        match accepted {
            [0] => Ok(None),
            [1] => {
                self.retained.clear();
                self.retained.extend_from_slice(input);
                Ok(Some(&self.retained))
            }
            _ => Err(ExpressionCallRefusal::InvalidProgram),
        }
    }
    pub(crate) fn cancel(
        &mut self,
        node: NodeId,
        call: HostCallId,
    ) -> Result<(), ExpressionCallRefusal> {
        self.predicate.cancel(node, call)
    }
}
pub struct FilterOperationFactory {
    implementation: ImplementationId,
}
impl Default for FilterOperationFactory {
    fn default() -> Self {
        Self {
            implementation: IMPLEMENTATION.into(),
        }
    }
}
impl KernelOperationFactory for FilterOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        let program = crate::expression_host_call::prepared_filter_program(gear)
            .map_err(|e| format!("{e:?}"))?;
        let maximum = program
            .maximum_prepared_input_bytes()
            .map_err(|e| format!("{e:?}"))?
            .max(1);
        Ok(KernelOperationBudget {
            value_items: 4,
            value_bytes: maximum * 4,
            maximum_value_bytes: maximum,
            host_requests: 1,
            sign_items: 32,
        })
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.budget(gear)?;
        // The existing nullable Host Call lifecycle already handles a dropped
        // value, normal EOF, pressure and cancellation in the shared kernel.
        Ok(Box::new(
            conduit_semantic_catalog::StructuredSelectorBack::new(
                gear.host_calls[0].maximum_input_bytes,
            ),
        ))
    }
}
