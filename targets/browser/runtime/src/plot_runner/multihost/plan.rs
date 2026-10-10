//! Exact two-browser-Host planning for the executable-tour lesson.

use crate::installed_browser::{advertisement, catalogs, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    verify_plan, BaseImplementationId, BaseInstanceId, CapabilityId, HostAdvertisement,
    LineAvailability, LineAvailabilitySign, LineContinuation, LineContract, LineDuplex, LineId,
    LineOffer, LineOrdering, LineReliability, LineScope, LineSecurity, LineTrafficShape,
    LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference, LinkEndpoint,
    LinkEndpointId, LinkLimits, Plan, SignId,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use std::collections::BTreeMap;

pub(super) const MEMORY_BASE: &str = "conduit.base/browser-memory@1";
pub(super) const ALTERNATE_MEMORY_BASE: &str = "conduit.base/browser-memory-buffered@1";
const LINE_ID: &str = "tour/browser-memory-line";
const BINDING_ID: &str = "tour/browser-memory-binding";
const BASE_INSTANCE_ID: &str = "tour/browser-memory-instance";
const SOURCE_ENDPOINT_ID: &str = "tour/browser-a-egress";
const SINK_ENDPOINT_ID: &str = "tour/browser-b-ingress";

pub(super) struct PreparedPlan {
    pub(super) plan: Plan,
    pub(super) source_host: HostAdvertisement,
    pub(super) sink_host: HostAdvertisement,
    pub(super) line: LineOffer,
}

pub(super) fn prepare(
    source_host_id: &str,
    source_boot_id: &str,
    sink_host_id: &str,
    sink_boot_id: &str,
    source: &str,
) -> Result<PreparedPlan, String> {
    prepare_with_base(
        source_host_id,
        source_boot_id,
        sink_host_id,
        sink_boot_id,
        source,
        MEMORY_BASE,
        None,
    )
}

fn prepare_with_base(
    source_host_id: &str,
    source_boot_id: &str,
    sink_host_id: &str,
    sink_boot_id: &str,
    source: &str,
    line_base: &str,
    sink_kinds: Option<&[&str]>,
) -> Result<PreparedPlan, String> {
    if source_host_id == sink_host_id || source_boot_id == sink_boot_id {
        return Err("two-browser lesson requires distinct Host and Boot identities".into());
    }
    let (startup, catalog) = catalogs()?;
    let syntax = conduit_plot::parse_syntax_document_with_glyph_notations(source, &startup);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!(
            "parse multi-host executable-tour Plot: {}",
            diagnostic.message
        ));
    }
    let checked =
        conduit_plot::check_syntax_document_with_literal_constructors(&syntax, &startup, &catalog)
            .map_err(|error| format!("check multi-host executable-tour Plot: {error:?}"))?;
    let entry = super::super::executable_entry(&checked)?;
    let plot = conduit_plot::expand_canonical_plot(&checked, &entry, &catalog)
        .map_err(|error| format!("expand multi-host executable-tour Plot: {error:?}"))?;
    let firefly_sink_kinds = [
        conduit_time::RHYTHM_STATE_SOURCE_KIND,
        conduit_time::PHASE_SYNCHRONIZE_KIND,
        conduit_semantic_catalog::RHYTHM_PRESENTATION_KIND,
    ];
    let sink_kinds = sink_kinds.or_else(|| {
        firefly_sink_kinds
            .iter()
            .all(|kind| plot.gears.iter().any(|gear| gear.kind_id.as_str() == *kind))
            .then_some(firefly_sink_kinds.as_slice())
    });
    if sink_kinds.is_none()
        && (plot.gears.len() < 2
            || plot.gears.len() > crate::installed_browser::MAXIMUM_BROWSER_GEARS
            || plot.connections.len() != plot.gears.len() - 1
            || plot.gears.iter().any(|gear| {
                plot.connections
                    .iter()
                    .filter(|cord| cord.source_gear_id == gear.gear_id)
                    .count()
                    > 1
                    || plot
                        .connections
                        .iter()
                        .filter(|cord| cord.sink_gear_id == gear.gear_id)
                        .count()
                        > 1
            }))
    {
        return Err("two-browser runner requires one bounded linear Plot".into());
    }
    let roots = plot
        .gears
        .iter()
        .filter(|gear| {
            !plot
                .connections
                .iter()
                .any(|cord| cord.sink_gear_id == gear.gear_id)
        })
        .collect::<Vec<_>>();
    let sink_gears = if let Some(kinds) = sink_kinds {
        let selected = plot
            .gears
            .iter()
            .filter(|gear| kinds.contains(&gear.kind_id.as_str()))
            .map(|gear| gear.gear_id.clone())
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err("partitioned two-browser runner selected no sink Gears".into());
        }
        let crossings = plot
            .connections
            .iter()
            .filter(|cord| {
                !selected.contains(&cord.source_gear_id) && selected.contains(&cord.sink_gear_id)
            })
            .count();
        if crossings != 1
            || plot.connections.iter().any(|cord| {
                selected.contains(&cord.source_gear_id) && !selected.contains(&cord.sink_gear_id)
            })
        {
            return Err("partitioned two-browser runner requires one directed crossing".into());
        }
        selected
    } else {
        let [source_gear] = roots.as_slice() else {
            return Err("two-browser runner requires one bounded linear Plot".into());
        };
        let sink_root = plot
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == conduit_net::TYPED_RECORD_DEFRAME_KIND)
            .unwrap_or_else(|| {
                plot.connections
                    .iter()
                    .find(|cord| cord.source_gear_id == source_gear.gear_id)
                    .and_then(|cord| {
                        plot.gears
                            .iter()
                            .find(|gear| gear.gear_id == cord.sink_gear_id)
                    })
                    .expect("a bounded linear Plot with two gears has a sink root")
            });
        let mut selected = vec![sink_root.gear_id.clone()];
        while let Some(next) = plot
            .connections
            .iter()
            .find(|cord| cord.source_gear_id == *selected.last().unwrap())
            .map(|cord| cord.sink_gear_id.clone())
        {
            selected.push(next);
        }
        selected
    };
    let expression_offers =
        crate::installed_browser::catalogs::offers_for_expanded_pure_expressions(&plot)?;
    let mut source_host = advertisement(source_host_id.into(), source_boot_id.into());
    let mut sink_host = advertisement(sink_host_id.into(), sink_boot_id.into());
    source_host.capabilities.extend(expression_offers.clone());
    sink_host.capabilities.extend(expression_offers);
    let placements = PlacementChoices {
        by_gear: plot
            .gears
            .iter()
            .map(|gear| {
                let host = if sink_gears.contains(&gear.gear_id) {
                    &sink_host
                } else {
                    &source_host
                };
                Ok((
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: capability(host, gear.kind_id.as_str())?,
                    },
                ))
            })
            .collect::<Result<_, String>>()?,
    };
    let line = memory_line(&source_host, &sink_host, line_base)?;
    let plan = plan_expanded_canonical_with_options(
        &plot,
        &[source_host.clone(), sink_host.clone()],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from(line_base),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_BROWSER_VALUE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: core::slice::from_ref(&line),
        },
    )
    .map_err(|error| format!("plan multi-host executable-tour Plot: {error:?}"))?;
    if plan.fragments.len() != 2 {
        return Err("two-browser lesson did not produce exactly two fragments".into());
    }
    Ok(PreparedPlan {
        plan,
        source_host,
        sink_host,
        line,
    })
}

pub(super) fn accept(
    plan: Plan,
    sink_host_id: &str,
    sink_boot_id: &str,
) -> Result<PreparedPlan, String> {
    if !verify_plan(&plan) {
        return Err("received multi-host Plan failed canonical identity verification".into());
    }
    if plan.fragments.len() != 2 {
        return Err("received multi-host Plan does not contain exactly two fragments".into());
    }
    let mut selected = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter_map(|connection| connection.selected_line.as_ref());
    let admitted = selected
        .next()
        .ok_or_else(|| "received multi-host Plan has no selected Line".to_string())?;
    if selected.any(|candidate| candidate != admitted) {
        return Err("received multi-host Plan contains conflicting selected Lines".into());
    }
    if admitted.binding.sink.host_id.as_str() != sink_host_id
        || admitted.binding.sink.boot_id.as_str() != sink_boot_id
    {
        return Err("received multi-host Plan does not name this exact sink Host and Boot".into());
    }
    if admitted.binding.source.host_id == admitted.binding.sink.host_id
        || admitted.binding.source.boot_id == admitted.binding.sink.boot_id
    {
        return Err(
            "received multi-host Plan does not retain distinct Host and Boot identities".into(),
        );
    }
    let mut source_host = advertisement(
        admitted.binding.source.host_id.clone(),
        admitted.binding.source.boot_id.clone(),
    );
    let mut sink_host = advertisement(
        admitted.binding.sink.host_id.clone(),
        admitted.binding.sink.boot_id.clone(),
    );
    for fragment in &plan.fragments {
        for placement in &fragment.placements {
            if let Some(offer) =
                crate::installed_browser::pure_expression::offer_for_placement(placement)?
            {
                let host = if fragment.host_id == source_host.host_id {
                    &mut source_host
                } else if fragment.host_id == sink_host.host_id {
                    &mut sink_host
                } else {
                    return Err("expression placement belongs to an unknown browser Host".into());
                };
                if !host
                    .capabilities
                    .iter()
                    .any(|existing| existing.capability_id == offer.capability_id)
                {
                    host.capabilities.push(offer);
                }
            }
        }
    }
    let line = memory_line(&source_host, &sink_host, admitted.binding.base.as_str())?;
    if admitted != &line.admitted_line() {
        return Err("received multi-host Plan changed the exact browser-memory Line".into());
    }
    let source_fragment_count = plan
        .fragments
        .iter()
        .filter(|fragment| fragment.host_id == source_host.host_id)
        .count();
    let sink_fragment_count = plan
        .fragments
        .iter()
        .filter(|fragment| fragment.host_id == sink_host.host_id)
        .count();
    if source_fragment_count != 1 || sink_fragment_count != 1 {
        return Err("received multi-host Plan changed its exact fragment ownership".into());
    }
    Ok(PreparedPlan {
        plan,
        source_host,
        sink_host,
        line,
    })
}

fn capability(host: &HostAdvertisement, kind: &str) -> Result<CapabilityId, String> {
    host.capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == kind)
        .map(|offer| offer.capability_id.clone())
        .ok_or_else(|| format!("browser Host does not offer {kind}"))
}

fn memory_line(
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    base: &str,
) -> Result<LineOffer, String> {
    let supported = base == MEMORY_BASE || base == ALTERNATE_MEMORY_BASE;
    if !supported {
        return Err("browser multi-host runner does not install the selected Line base".into());
    }
    let alternate = base == ALTERNATE_MEMORY_BASE;
    let line_id = if alternate {
        "tour/browser-memory-buffered-line"
    } else {
        LINE_ID
    };
    let binding = LinkBinding {
        binding_id: LinkBindingId::from(if alternate {
            "tour/browser-memory-buffered-binding"
        } else {
            BINDING_ID
        }),
        source: LinkEndpoint {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
            endpoint_id: LinkEndpointId::from(SOURCE_ENDPOINT_ID),
        },
        sink: LinkEndpoint {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
            endpoint_id: LinkEndpointId::from(SINK_ENDPOINT_ID),
        },
        base: BaseImplementationId::from(base),
        base_instance_id: BaseInstanceId::from(if alternate {
            "tour/browser-memory-buffered-instance"
        } else {
            BASE_INSTANCE_ID
        }),
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits: LinkLimits {
            maximum_in_flight_items: if alternate { 2 } else { 1 },
            maximum_payload_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
            maximum_buffered_bytes: (MAXIMUM_BROWSER_VALUE_BYTES as u32)
                * if alternate { 2 } else { 1 },
            maximum_frame_bytes: 4_096,
        },
    };
    Ok(LineOffer {
        line_id: LineId::from(line_id),
        availability: LineAvailabilitySign {
            line_id: LineId::from(line_id),
            binding_id: binding.binding_id.clone(),
            availability: LineAvailability::Ready,
            sign_id: SignId::from(if alternate {
                "tour/browser-memory-buffered-line/ready"
            } else {
                "tour/browser-memory-line/ready"
            }),
        },
        binding,
        contract: LineContract {
            scope: LineScope::Process,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::ProcessBoundary,
        },
    })
}

#[cfg(test)]
pub(super) fn prepare_with_alternate_line(
    source_host_id: &str,
    source_boot_id: &str,
    sink_host_id: &str,
    sink_boot_id: &str,
    source: &str,
) -> Result<PreparedPlan, String> {
    prepare_with_base(
        source_host_id,
        source_boot_id,
        sink_host_id,
        sink_boot_id,
        source,
        ALTERNATE_MEMORY_BASE,
        None,
    )
}
