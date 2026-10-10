//! Review uses ordinary finite Cord budgets supported by both selected offers.
use conduit_core::{
    authority_grant, process_owned_line_offer_with_limits, AuthorityGrant, BaseImplementationId,
    HostAdvertisement, LineId, LineOffer, LineScope, LineSecurity, LinkLimits,
};
use conduit_planner::{ConnectionQueueLimits, PlanningOptions};
use conduit_plot::ExpandedCanonicalPlot;
use std::collections::BTreeMap;

pub(in crate::creche) fn plan(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    bases: &[BaseImplementationId],
    joined_lines: &[crate::creche::JoinedLineObservation],
    authority: crate::creche::PlanningAuthority,
) -> Result<conduit_core::Plan, String> {
    let profile_limits = crate::plot_runner::finite_connection_limits(
        plot,
        crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES as u32,
    );
    let mut limits = BTreeMap::new();
    for cord in &plot.connections {
        let capability = |gear| {
            let choice = placements
                .by_gear
                .get(gear)
                .ok_or("missing review placement")?;
            hosts
                .iter()
                .find(|host| host.host_id == choice.host_id)
                .and_then(|host| {
                    host.capabilities
                        .iter()
                        .find(|offer| offer.capability_id == choice.capability_id)
                })
                .ok_or("missing selected review capability")
        };
        let source = capability(&cord.source_gear_id)?;
        let sink = capability(&cord.sink_gear_id)?;
        let endpoints = (
            cord.source_gear_id.clone(),
            cord.source_port_id.clone(),
            cord.sink_gear_id.clone(),
            cord.sink_port_id.clone(),
        );
        let required = profile_limits.get(&endpoints);
        limits.insert(
            endpoints,
            ConnectionQueueLimits {
                item_capacity: source
                    .limits
                    .max_queue_items
                    .min(sink.limits.max_queue_items)
                    .min(required.map_or(4, |limits| limits.item_capacity)),
                byte_capacity: required.map_or_else(
                    || {
                        source
                            .limits
                            .max_queue_bytes
                            .min(sink.limits.max_queue_bytes)
                            .min(crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES as u32)
                    },
                    |limits| limits.byte_capacity,
                ),
            },
        );
    }
    let authority_grants = authority_grants(hosts, placements, authority)?;
    let (line_offers, line_candidates) =
        line_offers(plot, hosts, placements, &limits, joined_lines)?;
    let mut available_bases = bases.to_vec();
    if !line_offers.is_empty()
        && !available_bases
            .iter()
            .any(|base| base.as_str() == "conduit.base/websocket-rfc6455@1")
    {
        available_bases.push(BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1",
        ));
    }
    conduit_planner::plan_expanded_canonical_with_connection_limits(
        plot,
        hosts,
        placements,
        &available_bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &authority_grants,
            protected_resource_grants: &[],
            line_offers: &line_offers,
        },
        &limits,
    )
    .map_err(|error| error.to_string())
}

fn authority_grants(
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    authority: crate::creche::PlanningAuthority,
) -> Result<Vec<AuthorityGrant>, String> {
    let mut grants = Vec::new();
    for (gear, placement) in &placements.by_gear {
        let host = hosts
            .iter()
            .find(|host| host.host_id == placement.host_id)
            .ok_or("Workspace placement names no current host")?;
        let capability = host
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .ok_or("Workspace placement names no current capability")?;
        for (index, requirement) in capability.authority_requirements.iter().enumerate() {
            let browser_audio = matches!(
                requirement.contract_id.as_str(),
                "conduit.authority/request-browser-microphone@1"
                    | "conduit.authority/use-browser-audio-output@1"
            );
            if !browser_audio || !authority.browser_audio {
                continue;
            }
            grants.push(authority_grant(
                &format!("grant/workspace/{}/{index}", gear.as_str()),
                requirement,
                host.host_id.clone(),
                host.boot_id.clone(),
                capability.capability_id.clone(),
            ));
        }
    }
    Ok(grants)
}

type LineCandidates = BTreeMap<(conduit_core::GearId, conduit_core::GearId), Vec<LineId>>;

fn line_offers(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    limits: &BTreeMap<
        (
            conduit_core::GearId,
            conduit_core::PortId,
            conduit_core::GearId,
            conduit_core::PortId,
        ),
        ConnectionQueueLimits,
    >,
    joined_lines: &[crate::creche::JoinedLineObservation],
) -> Result<(Vec<LineOffer>, LineCandidates), String> {
    let mut offers = Vec::new();
    let mut candidates = BTreeMap::new();
    for (index, connection) in plot.connections.iter().enumerate() {
        let source_placement = placements
            .by_gear
            .get(&connection.source_gear_id)
            .ok_or("Workspace Cord has no source placement")?;
        let sink_placement = placements
            .by_gear
            .get(&connection.sink_gear_id)
            .ok_or("Workspace Cord has no sink placement")?;
        if source_placement.host_id == sink_placement.host_id {
            continue;
        }
        let source = hosts
            .iter()
            .find(|host| host.host_id == source_placement.host_id)
            .ok_or("Workspace Cord source Host is absent")?;
        let sink = hosts
            .iter()
            .find(|host| host.host_id == sink_placement.host_id)
            .ok_or("Workspace Cord sink Host is absent")?;
        let joined = joined_lines.iter().find(|line| {
            line.carrier == "conduit-line/loopback-websocket@1"
                && ((line.host_id == source.host_id && line.boot_id == source.boot_id)
                    || (line.host_id == sink.host_id && line.boot_id == sink.boot_id))
        });
        if joined.is_none() {
            continue;
        }
        let cord_limits = limits
            .get(&(
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            ))
            .ok_or("Workspace Cord limits are absent")?;
        let maximum_frame_bytes = cord_limits
            .byte_capacity
            .checked_add(8_192)
            .ok_or("Workspace Line frame capacity overflow")?;
        let line_id = format!("line/workspace/{index}");
        let mut offer = process_owned_line_offer_with_limits(
            &line_id,
            &format!("binding/workspace/{index}"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "base/workspace/joined-websocket",
            source,
            sink,
            LinkLimits {
                maximum_in_flight_items: cord_limits.item_capacity,
                maximum_payload_bytes: cord_limits.byte_capacity,
                maximum_buffered_bytes: cord_limits.byte_capacity,
                maximum_frame_bytes,
            },
        );
        offer.contract.scope = LineScope::LocalNetwork;
        offer.contract.security = LineSecurity::PlaintextNetwork;
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

#[cfg(test)]
mod tests;
