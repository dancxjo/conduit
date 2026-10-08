//! Mechanical selection of existing generic owners from sealed placements.
use super::{PreparedTopology, resources::PreparedIngress};
use alloc::{boxed::Box, format, string::String, vec::Vec};
use conduit_ai::operation_owners::*;
use conduit_composite::KernelOperationFactory;

pub(super) fn prepare(
    topology: &PreparedTopology<'_>,
    ingress: &PreparedIngress,
) -> Result<Vec<Box<dyn KernelOperationFactory>>, String> {
    let plan = &topology.plan;
    let resources = &ingress.resources;
    let initial = fixed_numeric::FixedNumericOperationFactory::for_plan_capacity64(plan, resources)
        .map_err(|error| format!("numeric owner admission: {error:?}"))?;
    let mut result: Vec<Box<dyn KernelOperationFactory>> = initial
        .into_iter()
        .map(|factory| Box::new(factory) as Box<dyn KernelOperationFactory>)
        .collect();
    macro_rules! owner {
        ($expression:expr) => {
            result.push(Box::new(
                $expression.map_err(|error| format!("numeric family admission: {error:?}"))?,
            ));
        };
    }
    owner!(
        fixed_numeric_u16_profile::U16ProfileOperationFactory::for_plan(
            plan,
            core::slice::from_ref(&topology.context.scalar)
        )
    );
    owner!(
        fixed_numeric_embedding_flow::FixedEmbeddingFlowOperationFactory::for_plan(plan, resources)
    );
    owner!(fixed_numeric_flow::FixedAffineFlowOperationFactory::for_plan(plan, resources));
    owner!(fixed_numeric_linear_flow::FixedLinearFlowOperationFactory::for_plan(plan, resources));
    owner!(fixed_numeric_pair_flow::FixedFlowPairOperationFactory::for_plan(plan));
    owner!(fixed_numeric_float_integer::FloatIntegerOperationFactory::for_plan(plan));
    owner!(native_profile::NativeProfileOperationFactory::for_plan(
        plan,
        &topology.context.native
    ));
    owner!(
        nominal_weakening::NominalWeakeningOperationFactory::for_plan(
            plan,
            &topology.context.weakening
        )
    );
    owner!(fixed_numeric_guard::FixedGuardOperationFactory::for_plan(
        plan,
        topology.context.guards.clone()
    ));
    owner!(
        closing_structured_pair::ClosingStructuredPairOperationFactory::for_plan(
            plan,
            topology.context.pairs.clone()
        )
    );
    Ok(result)
}
