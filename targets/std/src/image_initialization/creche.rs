//! Initial Crèche resource correspondence and funded factory construction.
use super::*;
use conduit_body_make::deployment_runtime_catalog::creche_std_offer;

impl StdHost {
    /// Compose one independently prepared encounter resource before constructing
    /// the final IMAGE registry/ledger. The caller retains its original provider;
    /// this resource grants no Source, Make-input, toolchain or device authority.
    pub fn from_selected_timer_and_creche_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        resource: ResourceOffer,
    ) -> Result<Self, String> {
        Self::from_selected_initial_bases(
            config,
            image,
            catalog,
            InitialBases {
                creche: Some(resource),
                ..InitialBases::default()
            },
        )
    }

    /// Prepare one actual Crèche provider on the verified initial IMAGE counter,
    /// then bind that same counter into the final Host. The returned affine
    /// value retains the provider's original charge; no Source checker is needed.
    ///
    /// # Safety
    /// Root must authorize all selected inputs/effects and qualify the complete
    /// metadata factory peak and retained metadata (including cloned config and
    /// returned resource). The provider must be independently funded through
    /// this scope, and must actually own the exact returned encounter resource.
    /// Its shared stores and later effects need their own lifetime custody.
    /// Failed preparation must leave no live worker/effect. This does not admit
    /// Crèche execution or supply Make, Source, file or device authority.
    pub unsafe fn from_selected_timer_and_creche_factory_image<T>(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &conduit_host_make::CheckedHostConfiguration,
        preparation_bytes: usize,
        retained_bytes: usize,
        prepare: impl FnOnce(
            &crate::body_execution::ImageRootPreparation<'_>,
        ) -> Result<
            (ResourceOffer, crate::body_execution::ImagePreparedValue<T>),
            String,
        >,
    ) -> Result<(Self, crate::body_execution::ImagePreparedValue<T>), String> {
        use crate::body_execution::ImageRootPreparation;
        let memory = crate::kernel_preparation::StagedImageMemory::verified(
            &config, image, checked, catalog,
        )?;
        let prepared = {
            let mut reserve = |bytes| memory.reserve(bytes);
            let root = ImageRootPreparation::new(&mut reserve);
            // SAFETY: preserve Root's qualified metadata and provider obligations.
            unsafe {
                root.prepare_value(preparation_bytes, retained_bytes, || {
                    Ok((config.clone(), prepare(&root)?))
                })
            }?
        };
        let ((final_config, (resource, provider)), _metadata_memory) = prepared.into_parts();
        let host = Self::from_selected_initial_bases(
            final_config,
            image,
            catalog,
            InitialBases {
                creche: Some(resource),
                memory: Some(memory),
                ..InitialBases::default()
            },
        )?;
        host.verify_selected_image_prepared_value(&provider)?;
        Ok((host, provider))
    }
}

pub(super) fn prepare_creche_entry(
    config: &StdHostConfig,
    image: &HostImage,
    resource: &ResourceOffer,
) -> Result<BaseProviderEntry, String> {
    let mut bases = image
        .manifest
        .base_selections
        .iter()
        .filter(|b| b.kind == "creche/encounter");
    let base = bases
        .next()
        .ok_or("selected IMAGE lacks Creche encounter Base")?;
    let mut budgets = image
        .manifest
        .resource_budgets
        .iter()
        .filter(|b| b.class == CRECHE_RESOURCE);
    let budget = budgets
        .next()
        .ok_or("selected IMAGE lacks Creche encounter budget")?;
    if bases.next().is_some()
        || base.driver != CRECHE_IMPLEMENTATION
        || budgets.next().is_some()
        || budget.bytes == 0
        || budget.slots != 1
        || resource.class_id.as_str() != CRECHE_RESOURCE
        || resource.pool_id.as_str().trim().is_empty()
        || resource.pool_id.as_str() != budget.id
        || resource.capacity_units != 1
        || resource.compute.is_some()
        || resource.content.is_some()
        || image
            .manifest
            .resource_budgets
            .iter()
            .filter(|b| b.id == budget.id)
            .count()
            != 1
    {
        return Err("selected IMAGE requires one exact prepared Creche encounter resource".into());
    }
    Ok(BaseProviderEntry {
        base_id: base.id.as_str().into(),
        provider_instance_id: format!(
            "{}/{}/creche",
            config.host_id.as_str(),
            config.boot_id.as_str()
        )
        .into(),
        provider_generation: config.offer_generation.0,
        implementation_id: CRECHE_IMPLEMENTATION.into(),
        mechanism_family: "creche/encounter".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![creche_std_offer()],
        resources: vec![resource.clone()],
    })
}
