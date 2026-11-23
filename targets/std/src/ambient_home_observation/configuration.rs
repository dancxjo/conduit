//! Equipment inputs selected by the checked Host configuration.
use conduit_host_make::CheckedHostConfiguration;
use conduit_std_offers::{AMBIENT_HOME_SOURCE_BASE, AMBIENT_HOME_SOURCE_IMPLEMENTATION};
use std::net::SocketAddr;

pub struct CheckedAmbientHomeSource<'a> {
    pub timezone: &'a str,
    pub weather_endpoint: SocketAddr,
    pub source_pool: &'a str,
    pub memory_pool: &'a str,
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
        if declarations.next().is_some()
            || base.implementation.as_deref() != Some(AMBIENT_HOME_SOURCE_IMPLEMENTATION)
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
        let timezone = text("timezone")?;
        timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| "checked Ambient source timezone is invalid")?;
        let weather_endpoint = text("weather_endpoint")?
            .parse::<SocketAddr>()
            .map_err(|_| "checked Ambient weather endpoint is invalid")?;
        let source_pool = text("source_pool")?;
        let memory_pool = text("memory_pool")?;
        if weather_endpoint.port() == 0 || source_pool == memory_pool {
            return Err(
                "checked Ambient source requires a nonzero endpoint and distinct pools".into(),
            );
        }
        Ok(Self {
            timezone,
            weather_endpoint,
            source_pool,
            memory_pool,
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
        F: FnOnce() -> Result<Option<crate::ambient_wallpaper::acquisition::PreparedDeck>, String>,
    {
        use conduit_core::*;
        use conduit_std_offers::{ambient_home_source_offer, AMBIENT_HOME_SOURCE_RESOURCE};
        use std::time::Duration;
        let parameters = CheckedAmbientHomeSource::from_configuration(checked)?;
        let base = image
            .manifest
            .base_selections
            .iter()
            .find(|base| {
                base.kind == AMBIENT_HOME_SOURCE_BASE
                    && base.driver == AMBIENT_HOME_SOURCE_IMPLEMENTATION
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
            kind: conduit_semantic_catalog::ambient_home_observation_kind()?,
            base: BaseProviderEntry {
                base_id: base.id.as_str().into(),
                provider_instance_id: format!(
                    "{}/{}/ambient-home-source",
                    config.host_id.as_str(),
                    config.boot_id.as_str()
                )
                .into(),
                provider_generation: config.offer_generation.0,
                implementation_id: AMBIENT_HOME_SOURCE_IMPLEMENTATION.into(),
                mechanism_family: AMBIENT_HOME_SOURCE_BASE.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![ambient_home_source_offer()?],
                resources: vec![
                    pool(parameters.source_pool, AMBIENT_HOME_SOURCE_RESOURCE)?,
                    pool(parameters.memory_pool, RUNTIME_MEMORY_RESOURCE_CLASS)?,
                ],
            },
            grant,
            resource_generation: ResourceGenerationId(format!(
                "{}/ambient-source/{}",
                config.boot_id.as_str(),
                config.offer_generation.0
            )),
            configuration: vec![ConfigurationEntry {
                key: "timezone".into(),
                value: ConfigurationValue::Text(parameters.timezone.into()),
            }],
            input_bytes: conduit_time::TICK_ENCODED_LEN,
            output_bytes: super::HOME_OBSERVATION_BYTES as u32,
            resource_bytes: super::HOME_OBSERVATION_BYTES as u64,
        };
        let budget = crate::NativeWorkerBudget {
            requested_stack_bytes: 2 * 1024 * 1024,
            maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
            capture_timeout: Duration::from_secs(3),
            cancellation_timeout: Duration::from_millis(250),
        };
        unsafe {
            Self::from_selected_timer_and_native_worker_image(
                config,
                image,
                catalog,
                checked,
                definition,
                budget,
                crate::ambient_wallpaper::acquisition::PREPARATION_BYTES,
                move || {
                    Ok(Box::new(
                        super::PreparedAmbientHomeObservation::prepare_with_optional_deck(
                            parameters.timezone,
                            parameters.weather_endpoint,
                            prepare_deck()?,
                        )?,
                    ))
                },
            )
        }
    }
}

/// Finite queue requirements for the connections owned by the Home source.
pub fn ambient_home_connection_limits(
    plot: &conduit_plot::ExpandedCanonicalPlot,
) -> std::collections::BTreeMap<
    conduit_planner::ConnectionEndpoints,
    conduit_planner::ConnectionQueueLimits,
> {
    let home_gear = |id| {
        plot.gears
            .iter()
            .any(|gear| &gear.gear_id == id && gear.kind_id.as_str() == AMBIENT_HOME_SOURCE_BASE)
    };
    plot.connections
        .iter()
        .filter_map(|connection| {
            let bytes = if home_gear(&connection.source_gear_id)
                && connection.source_port_id.as_str() == "observation"
            {
                super::HOME_OBSERVATION_BYTES as u32
            } else if home_gear(&connection.sink_gear_id)
                && connection.sink_port_id.as_str() == "tick"
            {
                conduit_time::TICK_ENCODED_LEN
            } else {
                return None;
            };
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
