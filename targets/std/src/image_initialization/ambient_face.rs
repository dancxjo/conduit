//! The checked IMAGE's semantic Home Face projection Base.
use super::*;
use conduit_std_offers::{
    AmbientFaceProjection, AMBIENT_CLOCK_SOURCE_IMPLEMENTATION, AMBIENT_HOME_FACE_BASE,
    AMBIENT_HOME_FACE_DRIVER, AMBIENT_HOME_SOURCE_IMPLEMENTATION,
    AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION,
};

pub(super) fn prepare(
    config: &StdHostConfig,
    image: &HostImage,
    initial: &InitialBases<'_>,
) -> Result<Option<BaseProviderEntry>, String> {
    let manifest = &image.manifest;
    let mut projections = manifest
        .implementations
        .iter()
        .filter_map(|id| AmbientFaceProjection::for_implementation(id));
    let Some(projection) = projections.next() else {
        return Ok(None);
    };
    let mut bases = manifest.base_selections.iter().filter(|base| {
        base.kind == AMBIENT_HOME_FACE_BASE && base.driver == AMBIENT_HOME_FACE_DRIVER
    });
    let base = bases
        .next()
        .ok_or("ambient Face requires its checked projection Base")?;
    let worker_id = match projection {
        AmbientFaceProjection::Home => AMBIENT_HOME_SOURCE_IMPLEMENTATION,
        AmbientFaceProjection::Clock => AMBIENT_CLOCK_SOURCE_IMPLEMENTATION,
        AmbientFaceProjection::Wallpaper => AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION,
    };
    if projections.next().is_some()
        || bases.next().is_some()
        || !initial
            .worker
            .as_ref()
            .is_some_and(|worker| worker.definition().base.implementation_id.as_str() == worker_id)
    {
        return Err(
            "ambient Face requires one checked projection and its exact admitted worker".into(),
        );
    }
    let offer = projection.offer()?;
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
