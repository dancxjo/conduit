//! Explicit checked projection-only Base. Reported data grants no acquisition.
use super::*;
use conduit_host_make::CheckedHostConfiguration;
use conduit_std_offers::{
    QUANTITY_OBSERVATION_FACE_BASE as BASE, QUANTITY_OBSERVATION_FACE_DRIVER as DRIVER,
    QUANTITY_OBSERVATION_FACE_IMPLEMENTATION as IMPLEMENTATION,
};

pub(super) fn prepare(
    config: &StdHostConfig,
    image: &HostImage,
    catalog: &MakeCatalog,
    checked: &CheckedHostConfiguration,
    initial: &InitialBases<'_>,
) -> Result<BaseProviderEntry, String> {
    conduit_host_make::verify_configuration_image(checked, image, catalog)?;
    let manifest = &image.manifest;
    if manifest.source_configuration_id.as_deref() != Some(checked.configuration_id())
        || manifest
            .implementations
            .iter()
            .filter(|id| id.as_str() == IMPLEMENTATION)
            .count()
            != 1
        || manifest
            .implementations
            .iter()
            .any(|id| conduit_std_offers::AmbientFaceProjection::for_implementation(id).is_some())
    {
        return Err(
            "quantity Face requires its exact checked Source and one projection family".into(),
        );
    }
    let mut selected = manifest
        .base_selections
        .iter()
        .filter(|base| base.kind == BASE);
    let base = selected
        .next()
        .filter(|base| base.driver == DRIVER)
        .ok_or("quantity Face requires its compiled projection Base")?;
    if selected.next().is_some()
        || !checked
            .resolved_bases()
            .iter()
            .any(|(kind, driver)| kind == BASE && driver == DRIVER)
    {
        return Err("quantity projection Base is ambiguous or differs from checked Source".into());
    }
    let mut declarations = checked
        .configuration()
        .bases
        .iter()
        .filter(|base| base.kind == BASE);
    let declaration = declarations
        .next()
        .filter(|_| declarations.next().is_none())
        .ok_or("quantity projection requires one checked Base declaration")?;
    let values = declaration
        .initialization
        .as_ref()
        .ok_or("quantity projection initialization missing")?
        .checked_values(
            catalog
                .initialization_contracts
                .get(DRIVER)
                .ok_or("quantity projection initialization contract missing")?,
        )?;
    let text = |key: &str| match values.get(key) {
        Some(ConfigurationValue::Text(value)) => Ok(value.as_str()),
        _ => Err("quantity projection pool name is not checked text".to_string()),
    };
    let face_pool = text("face_pool")?;
    let memory_pool = text("memory_pool")?;
    if face_pool == memory_pool || !valid_data_label(face_pool) || !valid_data_label(memory_pool) {
        return Err("quantity projection requires distinct exact named pools".into());
    }
    let offer = conduit_std_offers::quantity_observation_face_offer()?;
    let mut resources = Vec::with_capacity(offer.resource_requirements.len());
    for requirement in &offer.resource_requirements {
        let name = match requirement.class_id.as_str() {
            FACE_RESOURCE_CLASS => face_pool,
            RUNTIME_MEMORY_RESOURCE_CLASS => memory_pool,
            _ => return Err("quantity projection has an unsupported resource class".into()),
        };
        let mut pools = manifest
            .resource_budgets
            .iter()
            .filter(|pool| pool.id == name);
        let pool = pools
            .next()
            .ok_or("quantity projection named pool is missing")?;
        if pools.next().is_some()
            || initial.owns_native_pool(name)
            || pool.class != requirement.class_id.as_str()
            || pool.slots < requirement.units
            || pool.bytes < u64::from(requirement.units)
        {
            return Err("quantity projection pool is shared, ambiguous or undersized".into());
        }
        resources.push(resource_offer(&pool.id, &pool.class, pool.slots));
    }
    Ok(BaseProviderEntry {
        base_id: base.id.as_str().into(),
        provider_instance_id: format!(
            "{}/{}/quantity-observation-face",
            config.host_id.as_str(),
            config.boot_id.as_str()
        )
        .into(),
        provider_generation: config.offer_generation.0,
        implementation_id: DRIVER.into(),
        mechanism_family: BASE.into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![offer],
        resources,
    })
}

impl StdHost {
    /// Initialize one checked reported-quantity projection Base and its pools.
    /// This supplies no acquisition, recording or producer authority.
    pub fn from_selected_quantity_observation_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
    ) -> Result<Self, String> {
        bounded_image_preflight(&config, image)?;
        let mut initial = InitialBases::default();
        initial.quantity_face = Some(prepare(&config, image, catalog, checked, &initial)?);
        Self::from_selected_initial_bases(config, image, catalog, initial)
    }
}
