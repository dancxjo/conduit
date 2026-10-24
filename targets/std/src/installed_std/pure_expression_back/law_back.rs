//! Exact law-capsule Back identity; Source authority lives in the owning Host.
use super::*;
use conduit_plot::LawExpressionMaterial;

pub(super) fn is_law(placement: &PlannedGear) -> bool {
    matches!(
        placement.implementation_id.as_str(),
        conduit_std_offers::LAW_EXPRESSION_STD_IMPLEMENTATION
            | conduit_std_offers::LAW_FILTER_STD_IMPLEMENTATION
    )
}

fn material(placement: &PlannedGear) -> Result<LawExpressionMaterial, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("law expression requires one exact planned capsule".into());
    };
    let ("law-program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value)
    else {
        return Err("law expression planned capsule is malformed".into());
    };
    LawExpressionMaterial::from_canonical_hex(encoded)
        .map_err(|why| format!("planned law capsule encoding: {why:?}"))
}

pub(super) fn validate_placement(placement: &PlannedGear, expected: &[u8]) -> Result<(), String> {
    let material = material(placement)?;
    if material
        .canonical_bytes()
        .map_err(|why| format!("law capsule encoding: {why:?}"))?
        != expected
    {
        return Err("planned law capsule differs from independently retained Source".into());
    }
    let temporal = placement
        .inputs
        .first()
        .ok_or("law input missing")?
        .temporal;
    let filter =
        placement.implementation_id.as_str() == conduit_std_offers::LAW_FILTER_STD_IMPLEMENTATION;
    let offer = conduit_std_offers::law_expression_std_offer(&material, temporal, filter)
        .map_err(|why| format!("law expression offer: {why:?}"))?;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.semantic_contract != offer.semantic_contract
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || placement.realization_properties != offer.realization_properties
        || placement.base.is_some()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned law expression differs from installed realization".into());
    }
    Ok(())
}

pub(super) fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    // This validates only the complete immutable representation/realization.
    // Runtime host preparation additionally requires private verified custody.
    let bytes = material(placement)?
        .canonical_bytes()
        .map_err(|why| format!("law capsule: {why:?}"))?;
    validate_placement(placement, &bytes)?;
    Ok(BackBudget {
        value_items: 8,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

pub(super) fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(placement)?;
    if placement.kind_contract_revision.as_str() == conduit_plot::LAW_FILTER_REVISION {
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
