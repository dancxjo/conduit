//! Installed local execution for exact checked pure expressions.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static FILTER_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::PURE_FILTER_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) use conduit_semantic_catalog::PureExpressionBack;

pub(crate) struct PureExpressionHost {
    inner: conduit_semantic_catalog::operation_owners::pure_expression_host::PreparedPureExpressionHost,
}
pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Result<Vec<Option<PureExpressionHost>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if matches!(
                placement.implementation_id.as_str(),
                conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION
                    | conduit_std_offers::PURE_FILTER_STD_IMPLEMENTATION
            ) {
                PureExpressionHost::from_placement(placement).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

impl PureExpressionHost {
    pub(crate) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let program = program_from_placement(placement)?;
        validate_placement(placement, &program)?;
        Ok(Self { inner: conduit_semantic_catalog::operation_owners::pure_expression_host::PreparedPureExpressionHost::prepare(&program, placement.inputs[0].temporal, placement.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION)? })
    }
    pub(super) fn execute(
        &mut self,
        input: &[u8],
    ) -> Result<&[u8], conduit_plot::PortableExpressionEvaluationRefusal> {
        self.inner.execute(input)
    }
    pub(crate) fn execute_filter(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_plot::PortableExpressionEvaluationRefusal> {
        self.inner.execute_filter(input)
    }
}
pub(crate) use conduit_semantic_catalog::operation_owners::pure_expression_host::program_from_placement;

fn validate_placement(
    placement: &PlannedGear,
    program: &conduit_plot::PortableExpressionProgram,
) -> Result<(), String> {
    let temporal = placement
        .inputs
        .first()
        .map(|port| port.temporal)
        .ok_or("pure expression input is absent")?;
    let offer =
        if placement.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
            conduit_std_offers::pure_filter_std_offer(program, temporal)
        } else {
            conduit_std_offers::pure_expression_std_offer(program, temporal)
        }
        .map_err(|error| format!("pure expression offer refusal: {error:?}"))?;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.inputs[0].temporal != placement.outputs[0].temporal
    {
        return Err("planned pure expression differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let program = program_from_placement(placement)?;
    validate_placement(placement, &program)?;
    Ok(BackBudget {
        value_items: 8,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(placement)?;
    if placement.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
        Ok(InstalledBack::PureFilter(
            conduit_semantic_catalog::StructuredSelectorBack::new(
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            ),
        ))
    } else {
        Ok(InstalledBack::PureExpression(PureExpressionBack::new(
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        )))
    }
}

pub(super) fn refusal_detail(refusal: &conduit_plot::PortableExpressionEvaluationRefusal) -> u16 {
    match refusal {
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidInput => 1,
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram => 2,
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidLiteral => 3,
        conduit_plot::PortableExpressionEvaluationRefusal::Arithmetic => 4,
        conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedSemanticCall(_) => 5,
        conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedType(_) => 6,
    }
}
