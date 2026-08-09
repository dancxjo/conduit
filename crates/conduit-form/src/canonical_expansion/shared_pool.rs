use crate::{
    hash_string, CanonicalExpansionDiagnostic, CanonicalStartupValue, CheckedCanonicalForm,
    CheckedGear, ExpandedSharedPool,
};
use std::collections::BTreeMap;

pub(super) fn bind_pool_environment(
    form: &CheckedCanonicalForm,
    environment: &BTreeMap<String, CanonicalStartupValue>,
    path: &[String],
) -> Result<BTreeMap<String, CanonicalStartupValue>, CanonicalExpansionDiagnostic> {
    let mut scoped = environment.clone();
    for pool in &form.pools {
        let pool_id = conduit_core::SharedPoolId::from(format!("{}/{}", path.join("/"), pool.name));
        if scoped
            .insert(
                pool.name.clone(),
                CanonicalStartupValue::PoolReference(pool_id),
            )
            .is_some()
        {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-050",
                format!(
                    "pool '{}' conflicts with an existing startup binding",
                    pool.name
                ),
            ));
        }
    }
    Ok(scoped)
}

pub(super) fn expanded_pool_declarations(
    form: &CheckedCanonicalForm,
    path: &[String],
) -> Vec<ExpandedSharedPool> {
    form.pools
        .iter()
        .map(|pool| {
            let pool_id =
                conduit_core::SharedPoolId::from(format!("{}/{}", path.join("/"), pool.name));
            ExpandedSharedPool {
                declaration_id: conduit_core::PoolDeclarationId::from(hash_string(&format!(
                    "pool-declaration:{}:{}",
                    form.checked_form_id.as_str(),
                    pool_id.as_str()
                ))),
                pool_id,
                member_face: pool.member_face.clone(),
                maximum_members: pool.maximum_members,
                consumers: Vec::new(),
            }
        })
        .collect()
}

pub(super) fn seal_pool_consumers(
    shared_pools: &mut [ExpandedSharedPool],
    gears: &[CheckedGear],
) -> Result<(), CanonicalExpansionDiagnostic> {
    shared_pools.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    for pool in shared_pools.iter_mut() {
        pool.consumers = gears
            .iter()
            .filter(|gear| gear.pool_references.contains(&pool.pool_id))
            .map(|gear| gear.gear_id.clone())
            .collect();
        pool.consumers.sort();
        if pool.consumers.is_empty() {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-051",
                format!(
                    "shared pool '{}' has no explicit consumer binding",
                    pool.pool_id.as_str()
                ),
            ));
        }
    }
    if let Some(reference) = gears
        .iter()
        .flat_map(|gear| &gear.pool_references)
        .find(|reference| !shared_pools.iter().any(|pool| &pool.pool_id == *reference))
    {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-052",
            format!(
                "pool reference '{}' has no exact expanded declaration",
                reference.as_str()
            ),
        ));
    }
    Ok(())
}
