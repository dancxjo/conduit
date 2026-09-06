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
const LINE_ID: &str = "book/browser-memory-line";
const BINDING_ID: &str = "book/browser-memory-binding";
const BASE_INSTANCE_ID: &str = "book/browser-memory-instance";
const SOURCE_ENDPOINT_ID: &str = "book/browser-a-egress";
const SINK_ENDPOINT_ID: &str = "book/browser-b-ingress";

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
    if source_host_id == sink_host_id || source_boot_id == sink_boot_id {
        return Err("two-browser lesson requires distinct Host and Boot identities".into());
    }
    let (startup, catalog) = catalogs()?;
    let syntax = conduit_form::parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!(
            "parse multi-Host executable-tour Form: {}",
            diagnostic.message
        ));
    }
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check multi-Host executable-tour Form: {error:?}"))?;
    let entry = checked
        .forms
        .last()
        .ok_or_else(|| "multi-Host executable-tour source has no Form".to_string())?
        .name
        .clone();
    let form = conduit_form::expand_canonical_form(&checked, &entry, &catalog)
        .map_err(|error| format!("expand multi-Host executable-tour Form: {error:?}"))?;
    if form.gears.len() < 2
        || form.gears.len() > crate::installed_browser::MAXIMUM_BROWSER_GEARS
        || form.connections.len() != form.gears.len() - 1
        || form.gears.iter().any(|gear| {
            form.connections
                .iter()
                .filter(|cord| cord.source_gear_id == gear.gear_id)
                .count()
                > 1
                || form
                    .connections
                    .iter()
                    .filter(|cord| cord.sink_gear_id == gear.gear_id)
                    .count()
                    > 1
        })
    {
        return Err("two-browser runner requires one bounded linear Form".into());
    }
    let roots = form
        .gears
        .iter()
        .filter(|gear| {
            !form
                .connections
                .iter()
                .any(|cord| cord.sink_gear_id == gear.gear_id)
        })
        .collect::<Vec<_>>();
    let [source_gear] = roots.as_slice() else {
        return Err("two-browser runner requires one bounded linear Form".into());
    };
    let source_host = advertisement(source_host_id.into(), source_boot_id.into());
    let sink_host = advertisement(sink_host_id.into(), sink_boot_id.into());
    let placements = PlacementChoices {
        by_gear: form
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.gear_id == source_gear.gear_id {
                    &source_host
                } else {
                    &sink_host
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
    let line = memory_line(&source_host, &sink_host);
    let plan = plan_expanded_canonical_with_options(
        &form,
        &[source_host.clone(), sink_host.clone()],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from(MEMORY_BASE),
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
    .map_err(|error| format!("plan multi-Host executable-tour Form: {error:?}"))?;
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
        return Err("received multi-Host Plan failed canonical identity verification".into());
    }
    if plan.fragments.len() != 2 {
        return Err("received multi-Host Plan does not contain exactly two fragments".into());
    }
    let mut selected = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter_map(|connection| connection.selected_line.as_ref());
    let admitted = selected
        .next()
        .ok_or_else(|| "received multi-Host Plan has no selected Line".to_string())?;
    if selected.any(|candidate| candidate != admitted) {
        return Err("received multi-Host Plan contains conflicting selected Lines".into());
    }
    if admitted.binding.sink.host_id.as_str() != sink_host_id
        || admitted.binding.sink.boot_id.as_str() != sink_boot_id
    {
        return Err("received multi-Host Plan does not name this exact sink Host and Boot".into());
    }
    if admitted.binding.source.host_id == admitted.binding.sink.host_id
        || admitted.binding.source.boot_id == admitted.binding.sink.boot_id
    {
        return Err(
            "received multi-Host Plan does not retain distinct Host and Boot identities".into(),
        );
    }
    let source_host = advertisement(
        admitted.binding.source.host_id.clone(),
        admitted.binding.source.boot_id.clone(),
    );
    let sink_host = advertisement(
        admitted.binding.sink.host_id.clone(),
        admitted.binding.sink.boot_id.clone(),
    );
    let line = memory_line(&source_host, &sink_host);
    if admitted != &line.admitted_line() {
        return Err("received multi-Host Plan changed the exact browser-memory Line".into());
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
        return Err("received multi-Host Plan changed its exact fragment ownership".into());
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

fn memory_line(source: &HostAdvertisement, sink: &HostAdvertisement) -> LineOffer {
    let binding = LinkBinding {
        binding_id: LinkBindingId::from(BINDING_ID),
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
        base: BaseImplementationId::from(MEMORY_BASE),
        base_instance_id: BaseInstanceId::from(BASE_INSTANCE_ID),
        credential: LinkCredentialReference::None,
        authority: LinkAuthorityReference::ProcessOwned,
        limits: LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
            maximum_buffered_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
            maximum_frame_bytes: 4_096,
        },
    };
    LineOffer {
        line_id: LineId::from(LINE_ID),
        availability: LineAvailabilitySign {
            line_id: LineId::from(LINE_ID),
            binding_id: binding.binding_id.clone(),
            availability: LineAvailability::Ready,
            sign_id: SignId::from("book/browser-memory-line/ready"),
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
    }
}
