//! Checked computational producer custody on an explicitly supplied current Host.
//! These Plans establish source selection, not input authority or producer Plays.
use conduit_core::{
    CapabilityOffer, ConfigurationValue, HostAdvertisement, Plan, PreparationHostIdentity,
};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlacementChoices, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, CheckedSyntaxDocument, ExpandedAuthoringPlot, PortableExpressionProgram,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    prepare_checked_parsec_map_transition, prepare_checked_source_plan, CheckedSourcePlanInputs,
    PreparedCheckedSourcePlan, PreparedParsecMapTransition, PARSEC_MAP_FRAME_BYTES,
    PARSEC_MAP_TRANSITION_SOURCE,
};
use std::collections::BTreeMap;

#[path = "parsec_input_policy_plan.rs"]
mod input_policy;
pub use input_policy::{required_parsec_input_policy_offer, ParsecInputPolicySourcePlan};

struct Producer {
    expanded: ExpandedAuthoringPlot,
    placements: PlacementChoices,
    bounds: BTreeMap<ForeBoundaryKey, ConnectionQueueLimits>,
    plan: Plan,
}
pub struct ParsecSourcePlans {
    checked: CheckedSyntaxDocument,
    hosts: [HostAdvertisement; 1],
    game: Producer,
    map: Producer,
}

fn checked() -> Result<CheckedSyntaxDocument, String> {
    check_syntax_document(
        &parse_syntax_document(PARSEC_MAP_TRANSITION_SOURCE),
        &StartupCatalog::new(),
    )
    .map_err(|error| format!("Parsec source check: {error:?}"))
}
fn expanded(checked: &CheckedSyntaxDocument, name: &str) -> Result<ExpandedAuthoringPlot, String> {
    expand_canonical_plot_for_authoring(checked, name, &ProfileCatalog::new())
        .map_err(|error| format!("Parsec source expansion: {error:?}"))
}
pub fn required_parsec_source_offers() -> Result<Vec<CapabilityOffer>, String> {
    let checked = checked()?;
    ["parsec/state-step", "parsec/map-transition"]
        .into_iter()
        .map(|name| {
            let expanded = expanded(&checked, name)?;
            let gear = &expanded.expanded.gears[0];
            let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
                return Err("Parsec producer has no exact program".into());
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded)
                .map_err(|error| format!("Parsec producer program: {error:?}"))?;
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .map_err(|error| format!("Parsec producer offer: {error:?}"))
        })
        .collect()
}

impl Producer {
    fn prepare(
        checked: &CheckedSyntaxDocument,
        hosts: &[HostAdvertisement; 1],
        name: &str,
    ) -> Result<Self, String> {
        let expanded = expanded(checked, name)?;
        let placements = conduit_planner::default_expanded_placements(&expanded.expanded, hosts)
            .map_err(|error| format!("Parsec source placement: {error:?}"))?;
        let bounds = expanded
            .input_bindings
            .iter()
            .map(|binding| (conduit_core::PortDirection::Input, binding))
            .chain(
                expanded
                    .output_bindings
                    .iter()
                    .map(|binding| (conduit_core::PortDirection::Output, binding)),
            )
            .map(|(direction, binding)| {
                (
                    ForeBoundaryKey {
                        direction,
                        front_port_id: binding.front_port_id.clone(),
                        track: binding.track,
                    },
                    ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
                    },
                )
            })
            .collect();
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &expanded,
            hosts,
            &placements,
            &[],
            options(&empty, &lines),
            &bounds,
        )
        .map_err(|error| format!("Parsec source Plan: {error:?}"))?;
        Ok(Self {
            expanded,
            placements,
            bounds,
            plan,
        })
    }
    fn custody<'a>(
        &'a self,
        checked: &CheckedSyntaxDocument,
        hosts: &[HostAdvertisement; 1],
    ) -> Result<PreparedCheckedSourcePlan<'a>, String> {
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        prepare_checked_source_plan(
            checked,
            &self.expanded,
            &ProfileCatalog::new(),
            &CanonicalBackCatalog::new(),
            &self.plan,
            CheckedSourcePlanInputs {
                hosts,
                placements: &self.placements,
                bases: &[],
                options: options(&empty, &lines),
                boundary_limits: &self.bounds,
            },
        )
        .map_err(|error| format!("Parsec source custody: {error:?}"))
    }
}
fn options<'a>(
    bases: &'a BTreeMap<
        (conduit_core::GearId, conduit_core::GearId),
        conduit_core::BaseImplementationId,
    >,
    lines: &'a BTreeMap<(conduit_core::GearId, conduit_core::GearId), Vec<conduit_core::LineId>>,
) -> PlanningOptions<'a> {
    PlanningOptions {
        connection_bases: bases,
        line_candidates: lines,
        connection_item_capacity: 1,
        connection_byte_capacity: PARSEC_MAP_FRAME_BYTES as u32,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    }
}
impl ParsecSourcePlans {
    pub fn prepare(advertisement: &HostAdvertisement) -> Result<Self, String> {
        for required in required_parsec_source_offers()? {
            if !advertisement
                .capabilities
                .iter()
                .any(|offer| offer == &required)
            {
                return Err(
                    "Parsec computational producer Back is absent from current Host offers".into(),
                );
            }
        }
        let checked = checked()?;
        let hosts = [advertisement.clone()];
        let game = Producer::prepare(&checked, &hosts, "parsec/state-step")?;
        let map = Producer::prepare(&checked, &hosts, "parsec/map-transition")?;
        Ok(Self {
            checked,
            hosts,
            game,
            map,
        })
    }
    pub fn prepare_evaluator(&self) -> Result<PreparedParsecMapTransition, String> {
        prepare_checked_parsec_map_transition(
            &self.game.custody(&self.checked, &self.hosts)?,
            &self.map.custody(&self.checked, &self.hosts)?,
        )
        .map_err(|error| format!("Parsec source evaluator: {error:?}"))
    }
    pub fn host_identity(&self) -> PreparationHostIdentity {
        let host = &self.hosts[0];
        PreparationHostIdentity {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
        }
    }
}
