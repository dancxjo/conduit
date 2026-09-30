use crate::{
    BoundedActivationError, BoundedActivationHost, BoundedFoldActivationHost, BoundedFoldError,
    BoundedScanActivationHost, BoundedScanError, KernelCompositeDefinition,
    KernelCompositeDefinitionError, KernelCompositeHost, KernelOperationRegistry,
};
use conduit_core::{
    semantic_digest, verify_prepared_plan, Plan, PlanId, PlannedActivationEntry, PreparedPlan,
};

pub struct PreparedActivationChildPool {
    outer_plan_id: PlanId,
    activation_id: String,
    preparation_identity: [u8; 32],
    definition: KernelCompositeDefinition,
    ready: Vec<KernelCompositeHost>,
}

pub enum PreparedPlannedActivationComposite {
    Unary(BoundedActivationHost),
    Fold(BoundedFoldActivationHost),
    Scan(BoundedScanActivationHost),
}

#[derive(Debug)]
pub enum PlannedActivationCompositeError {
    Definition(KernelCompositeDefinitionError),
    MissingActivation,
    StaleOrSubstitutedHandoff,
    ChildPreparation(crate::KernelCompositeError),
    Unary(BoundedActivationError),
    Fold(BoundedFoldError),
    Scan(BoundedScanError),
}

impl PreparedActivationChildPool {
    /// The only registry-bearing entrance. Exact subordinate receipts are
    /// consumed to initialize the finite child pool before coordinator play.
    pub fn prepare_on_host(
        outer: &Plan,
        prepared: &PreparedPlan,
        activation_id: &str,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, PlannedActivationCompositeError> {
        let definition =
            KernelCompositeDefinition::from_planned_activation(outer, prepared, activation_id)
                .map_err(PlannedActivationCompositeError::Definition)?;
        let entry = activation(outer, activation_id)
            .ok_or(PlannedActivationCompositeError::MissingActivation)?;
        let maximum_items = usize::from(limits(entry).maximum_items);
        let mut ready = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            ready.push(
                KernelCompositeHost::prepare(definition.clone(), registry)
                    .map_err(PlannedActivationCompositeError::ChildPreparation)?,
            );
        }
        Ok(Self {
            outer_plan_id: outer.plan_id.clone(),
            activation_id: activation_id.into(),
            preparation_identity: preparation_identity(outer, prepared, activation_id)?,
            definition,
            ready,
        })
    }
}

impl PreparedPlannedActivationComposite {
    /// Consume a single-use host-prepared handoff. There is no registry lookup
    /// at this boundary or during play.
    pub fn prepare(
        outer: &Plan,
        prepared: &PreparedPlan,
        activation_id: &str,
        handoff: PreparedActivationChildPool,
    ) -> Result<Self, PlannedActivationCompositeError> {
        if handoff.outer_plan_id != outer.plan_id
            || handoff.activation_id != activation_id
            || handoff.preparation_identity != preparation_identity(outer, prepared, activation_id)?
        {
            return Err(PlannedActivationCompositeError::StaleOrSubstitutedHandoff);
        }
        let entry = activation(outer, activation_id)
            .ok_or(PlannedActivationCompositeError::MissingActivation)?;
        match entry {
            PlannedActivationEntry::Unary(value) => {
                BoundedActivationHost::prepare_planned_with_ready(
                    value,
                    &handoff.definition,
                    handoff.ready,
                )
                .map(Self::Unary)
                .map_err(PlannedActivationCompositeError::Unary)
            }
            PlannedActivationEntry::Fold(value) => BoundedFoldActivationHost::prepare_with_ready(
                value,
                handoff.definition,
                handoff.ready,
            )
            .map(Self::Fold)
            .map_err(PlannedActivationCompositeError::Fold),
            PlannedActivationEntry::Scan(value) => BoundedScanActivationHost::prepare_with_ready(
                value,
                handoff.definition,
                handoff.ready,
            )
            .map(Self::Scan)
            .map_err(PlannedActivationCompositeError::Scan),
        }
    }
}

fn activation<'a>(plan: &'a Plan, id: &str) -> Option<&'a PlannedActivationEntry> {
    plan.activations.iter().find(|entry| match entry {
        PlannedActivationEntry::Unary(value) => value.activation_id == id,
        PlannedActivationEntry::Fold(value) => value.activation_id == id,
        PlannedActivationEntry::Scan(value) => value.activation_id == id,
    })
}

fn limits(entry: &PlannedActivationEntry) -> conduit_core::PlannedActivationLimits {
    match entry {
        PlannedActivationEntry::Unary(value) => value.limits,
        PlannedActivationEntry::Fold(value) => value.limits,
        PlannedActivationEntry::Scan(value) => value.limits,
    }
}

fn preparation_identity(
    outer: &Plan,
    prepared: &PreparedPlan,
    activation_id: &str,
) -> Result<[u8; 32], PlannedActivationCompositeError> {
    if !verify_prepared_plan(prepared, outer) {
        return Err(PlannedActivationCompositeError::StaleOrSubstitutedHandoff);
    }
    let selected = match activation(outer, activation_id)
        .ok_or(PlannedActivationCompositeError::MissingActivation)?
    {
        PlannedActivationEntry::Unary(value) => value.selected_plan.as_ref(),
        PlannedActivationEntry::Fold(value) => value.selected_plan.as_ref(),
        PlannedActivationEntry::Scan(value) => value.selected_plan.as_ref(),
    };
    let mut exact = Vec::new();
    exact.extend_from_slice(outer.plan_id.as_str().as_bytes());
    exact.push(0);
    exact.extend_from_slice(activation_id.as_bytes());
    for (_, receipt) in prepared
        .subordinate_receipts()
        .iter()
        .filter(|(id, _)| id == activation_id)
    {
        exact.extend_from_slice(receipt.plan_id().as_str().as_bytes());
        exact.extend_from_slice(receipt.fragment_id().as_str().as_bytes());
        exact.extend_from_slice(receipt.host().host_id.as_str().as_bytes());
        exact.extend_from_slice(receipt.host().boot_id.as_str().as_bytes());
        exact.extend_from_slice(&receipt.host().offer_generation.0.to_le_bytes());
    }
    for placement in selected.fragments.iter().flat_map(|part| &part.placements) {
        exact.extend_from_slice(placement.placement_id.as_str().as_bytes());
        exact.extend_from_slice(placement.implementation_id.as_str().as_bytes());
        exact.extend_from_slice(placement.artifact_id.as_str().as_bytes());
    }
    Ok(semantic_digest(
        "conduit/prepared-activation-child-pool@1",
        &exact,
    ))
}
