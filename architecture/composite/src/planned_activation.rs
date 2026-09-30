use crate::{
    BoundedActivationError, BoundedActivationHost, BoundedFoldActivationHost, BoundedFoldError,
    BoundedScanActivationHost, BoundedScanError, KernelCompositeDefinition,
    KernelCompositeDefinitionError, KernelOperationRegistry,
};
use conduit_core::{Plan, PlannedActivationEntry, PreparedPlan};

/// Allocation-prepared execution bridge for one exact activation entry.
///
/// Selection is by the sealed activation variant, never by Kind lookup. Each
/// coordinator pre-prepares its exact-N child kernel pool before this value is
/// returned, so play performs no preparation or pool growth.
pub enum PreparedPlannedActivationComposite {
    Unary(BoundedActivationHost),
    Fold(BoundedFoldActivationHost),
    Scan(BoundedScanActivationHost),
}

#[derive(Debug)]
pub enum PlannedActivationCompositeError {
    Definition(KernelCompositeDefinitionError),
    MissingActivation,
    Unary(BoundedActivationError),
    Fold(BoundedFoldError),
    Scan(BoundedScanError),
}

impl PreparedPlannedActivationComposite {
    pub fn prepare(
        outer: &Plan,
        prepared: &PreparedPlan,
        activation_id: &str,
        registry: &KernelOperationRegistry,
    ) -> Result<Self, PlannedActivationCompositeError> {
        let definition =
            KernelCompositeDefinition::from_planned_activation(outer, prepared, activation_id)
                .map_err(PlannedActivationCompositeError::Definition)?;
        let entry = outer
            .activations
            .iter()
            .find(|entry| match entry {
                PlannedActivationEntry::Unary(value) => value.activation_id == activation_id,
                PlannedActivationEntry::Fold(value) => value.activation_id == activation_id,
                PlannedActivationEntry::Scan(value) => value.activation_id == activation_id,
            })
            .ok_or(PlannedActivationCompositeError::MissingActivation)?;
        match entry {
            PlannedActivationEntry::Unary(value) => {
                BoundedActivationHost::prepare_planned(value, definition, registry)
                    .map(Self::Unary)
                    .map_err(PlannedActivationCompositeError::Unary)
            }
            PlannedActivationEntry::Fold(value) => {
                BoundedFoldActivationHost::prepare(value, definition, registry)
                    .map(Self::Fold)
                    .map_err(PlannedActivationCompositeError::Fold)
            }
            PlannedActivationEntry::Scan(value) => {
                BoundedScanActivationHost::prepare(value, definition, registry)
                    .map(Self::Scan)
                    .map_err(PlannedActivationCompositeError::Scan)
            }
        }
    }
}
