//! Compose the real ambient provider through the existing initial IMAGE worker.
use super::PreparedAmbientObservation;
use crate::{
    ImmediateSourcePreparationBudget, NativeSourceDefinition, NativeSourceWorkerProvider,
    NativeWorkerBudget, StdHost, StdHostConfig,
};
use conduit_core::*;
use conduit_host_hosted::{AmbientObservationLifetime, CheckedAmbientInitialization};
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
use conduit_std_offers::*;
use std::time::Duration;

/// A descriptive definition plus a separately supplied current Root grant.
/// Producing this value does not authorize or initialize a platform effect.
pub fn definition(
    config: &StdHostConfig,
    image: &HostImage,
    init: &CheckedAmbientInitialization,
    grant: AuthorityGrant,
) -> Result<NativeSourceDefinition, String> {
    if image.manifest.source_configuration_id.as_deref() != Some(&init.configuration_id) {
        return Err("ambient initialization belongs to a different checked IMAGE".into());
    }
    let (kind, offer, family, implementation, authority, call, resource) = match init.lifetime {
        AmbientObservationLifetime::Finite(_) => (
            conduit_semantic_catalog::ambient_observe_kind()?,
            ambient_source_offer()?,
            AMBIENT_SOURCE_BASE_KIND,
            AMBIENT_SOURCE_IMPLEMENTATION,
            AMBIENT_SOURCE_AUTHORITY,
            AMBIENT_SOURCE_CALL,
            AMBIENT_SOURCE_RESOURCE,
        ),
        AmbientObservationLifetime::Resident => (
            conduit_semantic_catalog::ambient_resident_observe_kind()?,
            ambient_resident_source_offer()?,
            AMBIENT_RESIDENT_SOURCE_BASE_KIND,
            AMBIENT_RESIDENT_SOURCE_IMPLEMENTATION,
            AMBIENT_RESIDENT_SOURCE_AUTHORITY,
            AMBIENT_RESIDENT_SOURCE_CALL,
            AMBIENT_RESIDENT_SOURCE_RESOURCE,
        ),
    };
    let mut selected = image
        .manifest
        .base_selections
        .iter()
        .filter(|b| b.kind == family);
    let selected = match (selected.next(), selected.next()) {
        (Some(base), None) if base.driver == implementation => base,
        _ => return Err("ambient IMAGE must select one exact provider".into()),
    };
    if grant.host_id != config.host_id
        || grant.boot_id != config.boot_id
        || grant.capability_id != offer.capability_id
        || grant.subject_kind != kind.kind_id
        || grant.contract_id.as_str() != authority
        || grant.host_call_contract_id.as_str() != call
    {
        return Err("ambient current independent grant differs from its exact provider".into());
    }
    Ok(NativeSourceDefinition {
        kind,
        grant,
        base: BaseProviderEntry {
            base_id: selected.id.as_str().into(),
            provider_instance_id: format!(
                "{}/{}/ambient",
                config.host_id.as_str(),
                config.boot_id.as_str()
            )
            .into(),
            provider_generation: config.offer_generation.0,
            implementation_id: implementation.into(),
            mechanism_family: family.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![
                resource_offer(&init.source_pool, resource, 1),
                resource_offer(
                    &init.memory_pool,
                    RUNTIME_MEMORY_RESOURCE_CLASS,
                    init.memory_bytes,
                ),
            ],
        },
        resource_generation: ResourceGenerationId(format!(
            "{}/ambient/{}",
            config.boot_id.as_str(),
            config.offer_generation.0
        )),
        configuration: {
            let mut entries = vec![ConfigurationEntry {
                key: "timezone".into(),
                value: ConfigurationValue::Text(init.timezone.clone()),
            }];
            if let AmbientObservationLifetime::Finite(maximum) = init.lifetime {
                entries.push(ConfigurationEntry {
                    key: "maximum-updates".into(),
                    value: ConfigurationValue::U64(u64::from(maximum)),
                });
            }
            entries
        },
        input_bytes: 8,
        output_bytes: 8192,
        resource_bytes: 8192,
    })
}
impl StdHost {
    /// Initial composition only, with the original counter and exact selected
    /// equipment. `photos` acquires or loads the finite dataset INSIDE the
    /// already funded provider factory; it is never called from runtime read.
    ///
    /// # Safety
    /// Root independently authorizes the actual endpoint, images and current
    /// grant. Its metadata envelope must cover the factory/captures and all
    /// checked definition construction. The supplied photo preparation and
    /// complete provider must fit the checked provider peak and retained budget,
    /// quiesce temporaries before returning and have non-unwinding destruction.
    /// The original worker contract and >=250ms cancellation bound apply.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_selected_ambient_observation_image<F>(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        metadata_budget: ImmediateSourcePreparationBudget,
        grant: AuthorityGrant,
        photos: F,
    ) -> Result<Self, String>
    where
        F: FnOnce() -> Result<crate::ambient_wallpaper::acquisition::PreparedDeck, String>
            + 'static,
    {
        // SAFETY: the current Root supplies the original obligations above.
        unsafe {
            Self::from_selected_timer_and_worker_source_factory_image(
                config,
                image,
                catalog,
                checked,
                metadata_budget,
                None,
                #[cfg(target_os = "linux")]
                None,
                move |config, _scope| {
                    let init = CheckedAmbientInitialization::from_configuration(checked)?;
                    let source = definition(config, image, &init, grant)?;
                    let provider_budget = ImmediateSourcePreparationBudget {
                        preparation_bytes: init.preparation_bytes,
                        retained_bytes: init.retained_bytes,
                    };
                    Ok((
                        source,
                        provider_budget,
                        NativeWorkerBudget {
                            requested_stack_bytes: 2 * 1024 * 1024,
                            maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
                            capture_timeout: Duration::from_secs(3),
                            cancellation_timeout: Duration::from_millis(500),
                        },
                        move || {
                            let deck = photos()?;
                            let provider = match init.lifetime {
                                AmbientObservationLifetime::Finite(maximum) => {
                                    PreparedAmbientObservation::prepare(
                                        &init.timezone,
                                        init.weather_endpoint,
                                        deck,
                                        maximum,
                                    )?
                                }
                                AmbientObservationLifetime::Resident => {
                                    PreparedAmbientObservation::prepare_resident(
                                        &init.timezone,
                                        init.weather_endpoint,
                                        deck,
                                    )?
                                }
                            };
                            let actual = core::mem::size_of::<PreparedAmbientObservation>()
                                .checked_add(provider.prepared_heap_bytes())
                                .ok_or("ambient retained bytes overflow")?;
                            if actual > init.retained_bytes as usize {
                                return Err(
                                    "actual ambient provider exceeds admitted retained backing"
                                        .into(),
                                );
                            }
                            Ok(Box::new(provider) as Box<dyn NativeSourceWorkerProvider>)
                        },
                    ))
                },
            )
        }
    }
}
