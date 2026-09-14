//! Exact multi-Host planning proof for the unchanged spoken House conversation.

use conduit_core::{
    authority_grant, process_owned_line_offer_with_limits, AuthorityGrant, BaseImplementationId,
    BootId, HostAdvertisement, HostId, LineId, LineOffer, LinkLimits,
};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const REMOTE_BASE: &str = "conduit.base/websocket-rfc6455@1";
type LineCandidates = BTreeMap<(conduit_core::GearId, conduit_core::GearId), Vec<LineId>>;
type PlannedLines = (Vec<LineOffer>, LineCandidates);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistributedHousePlan {
    pub checked_form_id: String,
    pub plan: conduit_core::Plan,
    pub hosts: [HostAdvertisement; 3],
    pub lines: Vec<LineOffer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistributedHouseRole {
    Capture,
    Cognition,
    Output,
}

impl DistributedHouseRole {
    fn index(self) -> usize {
        match self {
            Self::Capture => 0,
            Self::Cognition => 1,
            Self::Output => 2,
        }
    }
}

pub fn exact_distributed_spoken_house_plan(
    template: &HostAdvertisement,
) -> Result<DistributedHousePlan, Box<dyn std::error::Error>> {
    let topology = crate::house_conversation_topology::build(true, true)?;
    let hosts = [
        role_host(template, "capture"),
        role_host(template, "cognition"),
        role_host(template, "output"),
    ];
    let placements = exact_placements(template, &topology.expanded, &hosts)?;
    let authority_grants = authority_grants(&hosts, &placements)?;
    let (lines, line_candidates) = lines(
        &topology.expanded,
        &topology.connection_limits,
        &hosts,
        &placements,
    )?;
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &topology.expanded,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from(REMOTE_BASE),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::RECOGNITION_RESULT_QUEUE_BYTES,
            authority_grants: &authority_grants,
            protected_resource_grants: &[],
            line_offers: &lines,
        },
        &topology.connection_limits,
    )?;
    Ok(DistributedHousePlan {
        checked_form_id: topology.expanded.checked_form_id.as_str().to_owned(),
        plan,
        hosts,
        lines,
    })
}

pub fn replan_distributed_spoken_house_after_loss(
    template: &HostAdvertisement,
    lost: DistributedHouseRole,
) -> Result<conduit_core::Plan, String> {
    let exact = exact_distributed_spoken_house_plan(template).map_err(|error| error.to_string())?;
    let lost_host = &exact.hosts[lost.index()];
    let current_hosts = exact
        .hosts
        .iter()
        .filter(|host| host.host_id != lost_host.host_id)
        .cloned()
        .collect::<Vec<_>>();
    let current_lines = exact
        .lines
        .iter()
        .filter(|line| {
            line.binding.source.host_id != lost_host.host_id
                && line.binding.sink.host_id != lost_host.host_id
        })
        .cloned()
        .collect::<Vec<_>>();
    let topology =
        crate::house_conversation_topology::build(true, true).map_err(|error| error.to_string())?;
    let placements = exact_placements(template, &topology.expanded, &exact.hosts)
        .map_err(|error| error.to_string())?;
    let authority_grants =
        authority_grants(&exact.hosts, &placements).map_err(|error| error.to_string())?;
    let line_candidates = exact
        .lines
        .iter()
        .filter_map(|line| {
            let connection =
                topology
                    .expanded
                    .connections
                    .iter()
                    .enumerate()
                    .find(|(index, _)| {
                        line.line_id.as_str() == format!("line/distributed-house/{index}")
                    })?;
            Some((
                (
                    connection.1.source_gear_id.clone(),
                    connection.1.sink_gear_id.clone(),
                ),
                vec![line.line_id.clone()],
            ))
        })
        .collect::<LineCandidates>();
    conduit_planner::plan_expanded_canonical_with_connection_limits(
        &topology.expanded,
        &current_hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from(REMOTE_BASE),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_tongues::RECOGNITION_RESULT_QUEUE_BYTES,
            authority_grants: &authority_grants,
            protected_resource_grants: &[],
            line_offers: &current_lines,
        },
        &topology.connection_limits,
    )
    .map_err(|error| format!("{error:?}"))
}

fn exact_placements(
    template: &HostAdvertisement,
    form: &conduit_form::ExpandedCanonicalForm,
    hosts: &[HostAdvertisement; 3],
) -> Result<PlacementChoices, conduit_planner::PlannerError> {
    let defaults =
        conduit_planner::default_expanded_placements(form, std::slice::from_ref(template))?;
    let capture_sources = form
        .connections
        .iter()
        .filter(|connection| connection.sink_gear_id.as_str().ends_with("/audio"))
        .map(|connection| connection.source_gear_id.clone())
        .collect::<BTreeSet<_>>();
    Ok(PlacementChoices {
        by_gear: defaults
            .by_gear
            .into_iter()
            .map(|(gear_id, choice)| {
                let role =
                    if gear_id.as_str().ends_with("/audio") || capture_sources.contains(&gear_id) {
                        0
                    } else if gear_id.as_str().ends_with("/synthesize")
                        || gear_id.as_str().ends_with("/convert")
                        || gear_id.as_str().ends_with("/output")
                    {
                        2
                    } else {
                        1
                    };
                (
                    gear_id,
                    PlacementChoice {
                        host_id: hosts[role].host_id.clone(),
                        capability_id: choice.capability_id,
                    },
                )
            })
            .collect(),
    })
}

fn role_host(template: &HostAdvertisement, role: &str) -> HostAdvertisement {
    let mut host = template.clone();
    host.host_id = HostId::from(format!("host/house-{role}"));
    host.boot_id = BootId::from(format!("boot/house-{role}"));
    host
}

fn authority_grants(
    hosts: &[HostAdvertisement; 3],
    placements: &PlacementChoices,
) -> Result<Vec<AuthorityGrant>, Box<dyn std::error::Error>> {
    let mut grants = Vec::new();
    for (gear_id, placement) in &placements.by_gear {
        let host = hosts
            .iter()
            .find(|host| host.host_id == placement.host_id)
            .ok_or("distributed House placement names no current Host")?;
        let capability = host
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .ok_or("distributed House placement names no current capability")?;
        for (index, requirement) in capability.authority_requirements.iter().enumerate() {
            grants.push(authority_grant(
                &format!("grant/distributed-house/{}/{index}", gear_id.as_str()),
                requirement,
                host.host_id.clone(),
                host.boot_id.clone(),
                capability.capability_id.clone(),
            ));
        }
    }
    Ok(grants)
}

fn lines(
    form: &conduit_form::ExpandedCanonicalForm,
    limits: &BTreeMap<
        crate::house_conversation_topology::HouseConnectionEndpoints,
        conduit_planner::ConnectionQueueLimits,
    >,
    hosts: &[HostAdvertisement; 3],
    placements: &PlacementChoices,
) -> Result<PlannedLines, Box<dyn std::error::Error>> {
    let mut offers = Vec::new();
    let mut candidates = BTreeMap::new();
    for (index, connection) in form.connections.iter().enumerate() {
        let source = placements
            .by_gear
            .get(&connection.source_gear_id)
            .ok_or("distributed House Cord has no source placement")?;
        let sink = placements
            .by_gear
            .get(&connection.sink_gear_id)
            .ok_or("distributed House Cord has no sink placement")?;
        if source.host_id == sink.host_id {
            continue;
        }
        let source_host = hosts
            .iter()
            .find(|host| host.host_id == source.host_id)
            .unwrap();
        let sink_host = hosts
            .iter()
            .find(|host| host.host_id == sink.host_id)
            .unwrap();
        let connection_limits = limits
            .get(&(
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            ))
            .copied()
            .unwrap_or(conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: conduit_tongues::RECOGNITION_RESULT_QUEUE_BYTES,
            });
        let line_id = format!("line/distributed-house/{index}");
        let maximum_frame_bytes = connection_limits
            .byte_capacity
            .checked_add(8_192)
            .ok_or("distributed House Line frame capacity overflow")?;
        let mut offer = process_owned_line_offer_with_limits(
            &line_id,
            &format!("binding/distributed-house/{index}"),
            BaseImplementationId::from(REMOTE_BASE),
            &format!("base/distributed-house/{index}"),
            source_host,
            sink_host,
            LinkLimits {
                maximum_in_flight_items: connection_limits.item_capacity,
                maximum_payload_bytes: connection_limits.byte_capacity,
                maximum_buffered_bytes: connection_limits.byte_capacity,
                maximum_frame_bytes,
            },
        );
        offer.contract.scope = conduit_core::LineScope::LocalNetwork;
        offer.contract.security = conduit_core::LineSecurity::PlaintextNetwork;
        candidates.insert(
            (
                connection.source_gear_id.clone(),
                connection.sink_gear_id.clone(),
            ),
            vec![offer.line_id.clone()],
        );
        offers.push(offer);
    }
    Ok((offers, candidates))
}
