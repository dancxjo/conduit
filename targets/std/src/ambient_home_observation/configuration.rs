//! Equipment inputs selected by the checked Host configuration.
use conduit_host_make::CheckedHostConfiguration;
use conduit_std_offers::{
    AMBIENT_CLOCK_SOURCE_IMPLEMENTATION, AMBIENT_HOME_SOURCE_BASE,
    AMBIENT_HOME_SOURCE_IMPLEMENTATION, AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION,
};
use std::net::SocketAddr;

pub struct CheckedAmbientHomeSource<'a> {
    pub implementation: &'a str,
    pub timezone: Option<&'a str>,
    pub weather_endpoint: Option<SocketAddr>,
    pub source_pool: &'a str,
    pub memory_pool: &'a str,
    pub acquire_nasa_photos: bool,
}

impl<'a> CheckedAmbientHomeSource<'a> {
    pub fn from_configuration(checked: &'a CheckedHostConfiguration) -> Result<Self, String> {
        let mut declarations = checked
            .configuration()
            .bases
            .iter()
            .filter(|base| base.kind == AMBIENT_HOME_SOURCE_BASE);
        let base = declarations
            .next()
            .ok_or("checked Ambient source Base is missing")?;
        let implementation = base.implementation.as_deref().unwrap_or_default();
        if declarations.next().is_some()
            || !matches!(
                implementation,
                AMBIENT_HOME_SOURCE_IMPLEMENTATION
                    | AMBIENT_CLOCK_SOURCE_IMPLEMENTATION
                    | AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION
            )
        {
            return Err("checked Ambient source Base is ambiguous or unsupported".into());
        }
        let initialization = base
            .initialization
            .as_ref()
            .ok_or("checked Ambient source initialization is missing")?;
        let text = |key: &str| {
            initialization
                .values
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("checked Ambient source requires {key}"))
        };
        let (timezone, weather_endpoint) =
            if implementation == AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION {
                (None, None)
            } else {
                let timezone = text("timezone")?;
                timezone
                    .parse::<chrono_tz::Tz>()
                    .map_err(|_| "checked Ambient source timezone is invalid")?;
                let weather_endpoint = text("weather_endpoint")?
                    .parse::<SocketAddr>()
                    .map_err(|_| "checked Ambient weather endpoint is invalid")?;
                (Some(timezone), Some(weather_endpoint))
            };
        let source_pool = text("source_pool")?;
        let memory_pool = text("memory_pool")?;
        if text("provider_access")? != "operator-admitted" {
            return Err("Ambient source requires explicit operator provider access".into());
        }
        let acquire_nasa_photos = match text("photo_source")? {
            "nasa-apod" => true,
            "disabled" => false,
            _ => return Err("checked Ambient photo source is unsupported".into()),
        };
        if implementation == AMBIENT_CLOCK_SOURCE_IMPLEMENTATION && acquire_nasa_photos {
            return Err("the standalone clock worker has no admitted photo acquisition".into());
        }
        if weather_endpoint.is_some_and(|address| address.port() == 0) || source_pool == memory_pool
        {
            return Err(
                "checked Ambient source requires a nonzero endpoint and distinct pools".into(),
            );
        }
        Ok(Self {
            implementation,
            timezone,
            weather_endpoint,
            source_pool,
            memory_pool,
            acquire_nasa_photos,
        })
    }
}

impl crate::StdHost {
    /// Starts a selected Ambient worker over the checked provider endpoint and
    /// an independently prepared photograph source.
    ///
    /// # Safety
    /// The caller must own and authorize this exact grant and endpoint. The
    /// supplied deck factory must stay within the advertised preparation
    /// capacity and retire its temporary work on failure.
    pub unsafe fn from_selected_ambient_home_image<F>(
        config: crate::StdHostConfig,
        image: &conduit_host_make::HostImage,
        catalog: &conduit_host_make::MakeCatalog,
        checked: &CheckedHostConfiguration,
        grant: conduit_core::AuthorityGrant,
        prepare_deck: F,
    ) -> Result<Self, String>
    where
        F: FnOnce() -> Result<crate::ambient_wallpaper::observation::WallpaperPreparation, String>,
    {
        use crate::{image_initialization::InitialBases, kernel_preparation::StagedImageMemory};
        let memory = StagedImageMemory::verified(&config, image, checked, catalog)?;
        let worker = stage_ambient_worker(&config, image, checked, grant, prepare_deck, &memory)?;
        Self::from_selected_initial_bases(
            config.clone(),
            image,
            catalog,
            InitialBases {
                worker: Some(worker),
                memory: Some(memory),
                ..InitialBases::default()
            },
        )
    }
}

pub(crate) fn stage_ambient_worker<'a, F>(
    config: &crate::StdHostConfig,
    image: &conduit_host_make::HostImage,
    checked: &'a CheckedHostConfiguration,
    grant: conduit_core::AuthorityGrant,
    prepare_deck: F,
    memory: &crate::kernel_preparation::StagedImageMemory<'_>,
) -> Result<crate::selected_native_source::image_worker::PreparedImageWorker<'a>, String>
where
    F: FnOnce() -> Result<crate::ambient_wallpaper::observation::WallpaperPreparation, String> + 'a,
{
    use crate::selected_native_source::image_worker::{stage_native_worker, ImageWorkerFactory};
    use conduit_core::*;
    use conduit_std_offers::{
        ambient_clock_source_offer, ambient_home_source_offer, ambient_wallpaper_source_offer,
        AMBIENT_CLOCK_SOURCE_RESOURCE, AMBIENT_HOME_SOURCE_RESOURCE,
        AMBIENT_WALLPAPER_SOURCE_RESOURCE,
    };
    use std::time::Duration;
    let parameters = CheckedAmbientHomeSource::from_configuration(checked)?;
    let (kind, offer, source_resource, observation_bytes) = match parameters.implementation {
        AMBIENT_CLOCK_SOURCE_IMPLEMENTATION => (
            conduit_semantic_catalog::ambient_clock_kind()?,
            ambient_clock_source_offer()?,
            AMBIENT_CLOCK_SOURCE_RESOURCE,
            conduit_semantic_catalog::AMBIENT_CLOCK_OBSERVATION_BYTES,
        ),
        AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION => (
            conduit_semantic_catalog::resident_wallpaper_kind()?,
            ambient_wallpaper_source_offer()?,
            AMBIENT_WALLPAPER_SOURCE_RESOURCE,
            conduit_semantic_catalog::RESIDENT_WALLPAPER_BYTES,
        ),
        _ => (
            conduit_semantic_catalog::ambient_home_observation_kind()?,
            ambient_home_source_offer()?,
            AMBIENT_HOME_SOURCE_RESOURCE,
            super::HOME_OBSERVATION_BYTES as u32,
        ),
    };
    let base = image
        .manifest
        .base_selections
        .iter()
        .find(|base| {
            base.kind == AMBIENT_HOME_SOURCE_BASE && base.driver == parameters.implementation
        })
        .ok_or("Ambient worker Base is absent from the IMAGE")?;
    let pool = |id: &str, class: &str| {
        image
            .manifest
            .resource_budgets
            .iter()
            .find(|pool| pool.id == id && pool.class == class)
            .map(|pool| resource_offer(&pool.id, &pool.class, pool.slots))
            .ok_or_else(|| format!("Ambient worker lacks the checked pool {id}"))
    };
    let definition = crate::NativeSourceDefinition {
        kind,
        base: BaseProviderEntry {
            base_id: base.id.as_str().into(),
            provider_instance_id: format!(
                "{}/{}/ambient-home-source",
                config.host_id.as_str(),
                config.boot_id.as_str()
            )
            .into(),
            provider_generation: config.offer_generation.0,
            implementation_id: parameters.implementation.into(),
            mechanism_family: AMBIENT_HOME_SOURCE_BASE.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![
                pool(parameters.source_pool, source_resource)?,
                pool(parameters.memory_pool, RUNTIME_MEMORY_RESOURCE_CLASS)?,
            ],
        },
        grant,
        resource_generation: ResourceGenerationId(format!(
            "{}/ambient-source/{}",
            config.boot_id.as_str(),
            config.offer_generation.0
        )),
        configuration: parameters
            .timezone
            .into_iter()
            .map(|timezone| ConfigurationEntry {
                key: "timezone".into(),
                value: ConfigurationValue::Text(timezone.into()),
            })
            .collect(),
        input_bytes: conduit_time::TICK_ENCODED_LEN,
        output_bytes: observation_bytes,
        resource_bytes: u64::from(observation_bytes),
    };
    let budget = crate::NativeWorkerBudget {
        requested_stack_bytes: 2 * 1024 * 1024,
        maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
        capture_timeout: Duration::from_secs(3),
        cancellation_timeout: Duration::from_millis(250),
    };
    let factory = ImageWorkerFactory {
        budget,
        provider_preparation_bytes: crate::ambient_wallpaper::acquisition::PREPARATION_BYTES,
        prepare: Box::new(move || {
            let Some((timezone, endpoint)) = parameters.timezone.zip(parameters.weather_endpoint)
            else {
                return Ok(Box::new(
                    crate::ambient_wallpaper::observation::PreparedWallpaperObservation::prepare_from_outcome(
                        prepare_deck()?,
                    )?,
                ));
            };
            if parameters.implementation == AMBIENT_CLOCK_SOURCE_IMPLEMENTATION {
                return Ok(Box::new(crate::ambient_clock::PreparedLiveClock::prepare(
                    timezone, endpoint,
                )?));
            }
            Ok(Box::new(
                super::PreparedAmbientHomeObservation::prepare_with_wallpaper(
                    timezone,
                    endpoint,
                    prepare_deck()?,
                )?,
            ))
        }),
    };
    stage_native_worker(image, checked, definition, factory, memory)
}

/// Finite queue requirements for connections owned by an ambient observation source.
pub fn ambient_home_connection_limits(
    plot: &conduit_plot::ExpandedCanonicalPlot,
) -> std::collections::BTreeMap<
    conduit_planner::ConnectionEndpoints,
    conduit_planner::ConnectionQueueLimits,
> {
    use conduit_semantic_catalog::{
        AMBIENT_CLOCK_KIND, AMBIENT_CLOCK_OBSERVATION_BYTES, AMBIENT_HOME_OBSERVE_KIND,
        RESIDENT_WALLPAPER_BYTES, RESIDENT_WALLPAPER_KIND,
    };
    let observation_bytes = |id| {
        let gear = plot.gears.iter().find(|gear| &gear.gear_id == id)?;
        match gear.kind_id.as_str() {
            AMBIENT_HOME_OBSERVE_KIND => Some(super::HOME_OBSERVATION_BYTES as u32),
            AMBIENT_CLOCK_KIND => Some(AMBIENT_CLOCK_OBSERVATION_BYTES),
            RESIDENT_WALLPAPER_KIND => Some(RESIDENT_WALLPAPER_BYTES),
            _ => None,
        }
    };
    plot.connections
        .iter()
        .filter_map(|connection| {
            let bytes = match connection.source_port_id.as_str() {
                "observation" => observation_bytes(&connection.source_gear_id),
                _ => None,
            }
            .or_else(|| {
                if connection.sink_port_id.as_str() == "tick" {
                    observation_bytes(&connection.sink_gear_id)
                        .map(|_| conduit_time::TICK_ENCODED_LEN)
                } else {
                    None
                }
            })?;
            Some((
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            ))
        })
        .collect()
}
