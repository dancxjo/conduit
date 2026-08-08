use crate::{PlacementChoice, PlacementChoices, PlannerError};
use conduit_core::{CapabilityId, HostAdvertisement};
use conduit_form::CheckedOperation;
use std::collections::BTreeMap;

pub(crate) fn default_placements_unvalidated(
    operations: &[CheckedOperation],
    realm: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    let host = realm
        .first()
        .ok_or_else(|| PlannerError::UnknownHost("realm is empty".to_string()))?;
    let mut by_operation = BTreeMap::new();
    let mut selected_counts = BTreeMap::<CapabilityId, u16>::new();
    for operation in operations {
        let mut candidates = host
            .capabilities
            .iter()
            .filter(|offer| offer.checked_face() == operation.checked_face())
            .filter(|offer| {
                selected_counts
                    .get(&offer.capability_id)
                    .copied()
                    .unwrap_or(0)
                    < offer.limits.max_active_instances
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|offer| {
            (
                offer.kind_id != operation.kind_id,
                offer.kind_contract_revision != operation.kind_contract_revision,
                offer.capability_id.clone(),
            )
        });
        let offer = candidates.first().copied().ok_or_else(|| {
            PlannerError::UnknownCapability(operation.kind_id.as_str().to_string())
        })?;
        *selected_counts
            .entry(offer.capability_id.clone())
            .or_default() += 1;
        by_operation.insert(
            operation.operation_id.clone(),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: offer.capability_id.clone(),
            },
        );
    }
    Ok(PlacementChoices { by_operation })
}
