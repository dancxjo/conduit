//! Shared exact planning for the std-to-browser Signal execution pair.

use alloc::collections::BTreeMap;
use conduit_core::{
    BaseImplementationId, BootId, CapabilityId, GearId, HostAdvertisement, HostId, Plan,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedSignalPlan {
    pub source_advertisement: HostAdvertisement,
    pub sink_advertisement: HostAdvertisement,
    pub plan: Plan,
}

pub fn exact_distributed_signal_plan() -> Result<DistributedSignalPlan, alloc::string::String> {
    let source = crate::distributed_std_source_advertisement();
    exact_distributed_signal_plan_for(source.host_id, source.boot_id)
}

pub fn exact_distributed_signal_plan_for(
    source_host_id: HostId,
    source_boot_id: BootId,
) -> Result<DistributedSignalPlan, alloc::string::String> {
    exact_distributed_signal_plan_for_endpoints(
        source_host_id,
        source_boot_id,
        HostId::from(crate::DISTRIBUTED_BROWSER_HOST_ID),
        BootId::from(crate::DISTRIBUTED_BROWSER_BOOT_ID),
    )
}

pub fn exact_distributed_signal_plan_for_endpoints(
    source_host_id: HostId,
    source_boot_id: BootId,
    sink_host_id: HostId,
    sink_boot_id: BootId,
) -> Result<DistributedSignalPlan, alloc::string::String> {
    let source_advertisement =
        crate::distributed_source_advertisement_for(source_host_id.clone(), source_boot_id.clone());
    let sink_advertisement =
        crate::distributed_browser_advertisement_for(sink_host_id, sink_boot_id);
    let syntax = conduit_form::parse_syntax_document(include_str!(
        "../../../../forms/signal-demo/main.conduit"
    ));
    let checked = conduit_form::check_syntax_document(&syntax, &crate::signal_startup_catalog())
        .map_err(|error| alloc::format!("{}: {}", error.code, error.message))?;
    let form = conduit_form::expand_canonical_form(
        &checked,
        "signal-demo",
        &crate::signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: source_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                GearId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: sink_advertisement.host_id.clone(),
                    capability_id: CapabilityId::from("dom-show-1"),
                },
            ),
        ]),
    };
    let link = crate::distributed_websocket_line_offer_for_endpoints(
        source_host_id,
        source_boot_id,
        sink_advertisement.host_id.clone(),
        sink_advertisement.boot_id.clone(),
    );
    let plan = plan_expanded_canonical_with_options(
        &form,
        &[source_advertisement.clone(), sink_advertisement.clone()],
        &placements,
        &[BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1",
        )],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: crate::DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: crate::SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[link],
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(DistributedSignalPlan {
        source_advertisement,
        sink_advertisement,
        plan,
    })
}
