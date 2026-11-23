//! The checked IMAGE's semantic Home Face projection Base.
use super::*;
use conduit_std_offers::{
    resident_ambient_home_face_offer, AMBIENT_HOME_FACE_BASE, AMBIENT_HOME_FACE_DRIVER,
    AMBIENT_HOME_FACE_IMPLEMENTATION,
};

pub(super) fn prepare(
    config: &StdHostConfig,
    image: &HostImage,
    initial: &InitialBases<'_>,
) -> Result<Option<BaseProviderEntry>, String> {
    let manifest = &image.manifest;
    if !manifest
        .implementations
        .iter()
        .any(|id| id == AMBIENT_HOME_FACE_IMPLEMENTATION)
    {
        return Ok(None);
    }
    let mut bases = manifest.base_selections.iter().filter(|base| {
        base.kind == AMBIENT_HOME_FACE_BASE && base.driver == AMBIENT_HOME_FACE_DRIVER
    });
    let base = bases
        .next()
        .ok_or("Home Face requires its checked projection Base")?;
    if bases.next().is_some() || initial.worker.is_none() {
        return Err("Home Face requires one checked projection Base and an admitted worker".into());
    }
    let offer = resident_ambient_home_face_offer()?;
    let mut resources = Vec::with_capacity(offer.resource_requirements.len());
    for requirement in &offer.resource_requirements {
        let mut pools = manifest.resource_budgets.iter().filter(|pool| {
            pool.class == requirement.class_id.as_str() && !initial.owns_native_pool(&pool.id)
        });
        let pool = pools
            .next()
            .ok_or("Home Face requires its checked resource pool")?;
        if pools.next().is_some()
            || pool.slots < requirement.units
            || pool.bytes < u64::from(requirement.units)
        {
            return Err("Home Face resource pool is ambiguous or undersized".into());
        }
        resources.push(resource_offer(&pool.id, &pool.class, pool.slots));
    }
    Ok(Some(BaseProviderEntry {
        base_id: base.id.as_str().into(),
        provider_instance_id: format!(
            "{}/{}/ambient-home-face",
            config.host_id.as_str(),
            config.boot_id.as_str()
        )
        .into(),
        provider_generation: config.offer_generation.0,
        implementation_id: AMBIENT_HOME_FACE_DRIVER.into(),
        mechanism_family: AMBIENT_HOME_FACE_BASE.into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![offer],
        resources,
    }))
}
