//! Authored appliance construction material. This checks and expands meaning;
//! it creates no initialized provider, Host advertisement, authority or Plan.
use crate::publication::{PublicationChannel, PublicationContract};
use alloc::{
    format,
    string::{String, ToString},
};
use conduit_core::{ConfigurationValue, Kind, PortTemporal};
use conduit_plot::{
    self as p, CheckedCordStage, ExpandedCanonicalPlot, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

pub struct ApplianceSource {
    pub expanded: ExpandedCanonicalPlot,
    pub count: u64,
    pub period_ms: u64,
}

pub fn compile(count: u64, period_ms: u64) -> Result<ApplianceSource, String> {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_pulse_capture::PulseReadContract::prepare()
        .map_err(debug)?
        .install(&mut startup, &mut profiles)?;
    crate::observation::ObservationContract::prepare()
        .map_err(debug)?
        .install(&mut startup, &mut profiles)?;
    for kind in [
        conduit_time::tick_semantic_contract(),
        conduit_time::tick_trigger_semantic_contract(),
    ] {
        install(kind, &mut startup, &mut profiles)?;
    }
    for channel in PublicationChannel::ALL {
        PublicationContract::prepare(channel)
            .map_err(debug)?
            .install(&mut startup, &mut profiles)?;
    }
    let source = format!("{}\n{}\n{}\n{}\n{}\n{}\n{}\nplot appliance {{\n logger: environmental-appliance({count}, {period_ms})\n}}\n",
        include_str!("../../../../plots/device-protocols/pulse-types.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-capture.conduit"),
        include_str!("../../../../plots/device-protocols/dht11.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-read.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-observe.conduit"),
        include_str!("../../profiles/sampling.conduit"),
        include_str!("../../profiles/appliance.conduit"));
    let syntax = p::parse_syntax_document(&source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("appliance Source: {:?}", syntax.diagnostics));
    }
    let checked = p::check_syntax_document(&syntax, &startup).map_err(debug)?;
    for plot in &checked.plots {
        for cord in &plot.cords {
            for stage in &cord.stages {
                if let CheckedCordStage::StructuredSelector { selector, .. } = stage {
                    profiles
                        .insert(p::structured_selector_definition(
                            selector,
                            PortTemporal::Flow { closes: true },
                        ))
                        .map_err(debug)?;
                }
            }
        }
    }
    let expanded = p::expand_canonical_plot(&checked, "appliance", &profiles).map_err(debug)?;
    let tick = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_time::TICK_KIND)
        .ok_or_else(|| String::from("appliance has no authored sampling clock"))?;
    let cadence = conduit_time::parse_tick_configuration(&tick.configuration).map_err(debug)?;
    Ok(ApplianceSource {
        expanded,
        count: cadence.count,
        period_ms: cadence.period_ms,
    })
}
fn install(
    kind: Kind,
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: kind
            .startup_parameters
            .iter()
            .map(|parameter| StartupParameterSignature {
                name: parameter.name.clone(),
                value_type: "Count".into(),
                default: kind
                    .configuration
                    .iter()
                    .find(|field| field.key == parameter.name)
                    .and_then(|field| match field.default_value {
                        ConfigurationValue::U64(value) => Some(value.to_string()),
                        _ => None,
                    }),
            })
            .collect(),
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profiles.insert_kind(kind).map_err(debug)
}
fn debug(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}

impl ApplianceSource {
    /// Ordinary local planning over the Root's current initialized offers. The
    /// caller retains their providers and supplies exact Base and authority
    /// selections. An available capture capability without a grant must refuse.
    pub fn plan_local(
        &self,
        host: &conduit_core::HostAdvertisement,
        bases: &[conduit_core::BaseImplementationId],
        authority: &[conduit_core::AuthorityGrant],
    ) -> Result<conduit_core::Plan, conduit_planner::PlannerError> {
        use alloc::collections::BTreeMap;
        let hosts = [host.clone()];
        let placements = conduit_planner::default_expanded_placements(&self.expanded, &hosts)?;
        let connection_limits = self
            .expanded
            .connections
            .iter()
            .filter(|connection| {
                self.expanded.gears.iter().any(|gear| {
                    gear.gear_id == connection.sink_gear_id
                        && gear.kind_id.as_str() == PublicationChannel::Reading.kind_id()
                })
            })
            .map(|connection| {
                (
                    (
                        connection.source_gear_id.clone(),
                        connection.source_port_id.clone(),
                        connection.sink_gear_id.clone(),
                        connection.sink_port_id.clone(),
                    ),
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: PublicationChannel::Reading.maximum_bytes() as u32,
                    },
                )
            })
            .collect();
        conduit_planner::plan_expanded_canonical_with_connection_limits(
            &self.expanded,
            &hosts,
            &placements,
            bases,
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 1024,
                authority_grants: authority,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &connection_limits,
        )
    }
}
