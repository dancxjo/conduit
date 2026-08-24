//! Product-owned admission of the existing std/browser Signal Line.

use crate::product_execution::{ProductExecutionContext, ProductRuntime};
use conduit_core::{BootId, CapabilityId, ConnectionBase, GearId, HostAdvertisement, HostId, Plan};
use conduit_planner::{PlacementChoice, PlacementChoices};
use conduit_std_host::distributed_signal::{bind_listener, DistributedSource};
use std::collections::BTreeMap;
use std::io::Write;

pub(crate) fn context() -> Result<ProductExecutionContext, String> {
    context_for_instance(u64::from(std::process::id()))
}

pub(crate) fn context_for_instance(instance: u64) -> Result<ProductExecutionContext, String> {
    let (source, sink) = advertisements_for(instance);
    let line = conduit_signal::distributed_websocket_line_offer_for_endpoints(
        source.host_id.clone(),
        source.boot_id.clone(),
        sink.host_id.clone(),
        sink.boot_id.clone(),
    );
    ProductExecutionContext::new(
        vec![source.clone(), sink.clone()],
        vec![
            ProductRuntime::coordinated(source),
            ProductRuntime::coordinated(sink),
        ],
        vec![ConnectionBase::WebSocket],
        vec![line],
    )
}

pub(crate) fn advertisements_for(instance: u64) -> (HostAdvertisement, HostAdvertisement) {
    let source = conduit_signal::distributed_source_advertisement_for(
        HostId::from(format!("product/std-source/{instance}")),
        BootId::from(format!("product/std-source/{instance}/boot")),
    );
    let sink = conduit_signal::distributed_browser_advertisement_for(
        HostId::from(format!("product/browser/{instance}")),
        BootId::from(format!("product/browser/{instance}/boot")),
    );
    (source, sink)
}

pub(crate) fn placements(
    form: &conduit_form::ExpandedCanonicalForm,
) -> Result<PlacementChoices, String> {
    let mut by_gear = BTreeMap::new();
    let (source, sink) = advertisements_for(u64::from(std::process::id()));
    for gear in &form.gears {
        let (host, capability) = match gear.kind_id.as_str() {
            conduit_signal::PULSE_KIND => (source.host_id.clone(), "pulse-1"),
            conduit_signal::SHOW_KIND => (sink.host_id.clone(), "dom-show-1"),
            other => {
                return Err(format!(
                    "std-browser-line fixture does not implement '{other}'"
                ))
            }
        };
        by_gear.insert(
            GearId::from(gear.gear_id.as_str()),
            PlacementChoice {
                host_id: host,
                capability_id: CapabilityId::from(capability),
            },
        );
    }
    Ok(PlacementChoices { by_gear })
}

pub(crate) fn execute<W: Write>(
    plan: &Plan,
    output: &mut W,
) -> Result<Vec<conduit_core::Observation>, String> {
    let (source_advertisement, sink_advertisement) =
        advertisements_for(u64::from(std::process::id()));
    let source = DistributedSource::prepare_planned(
        plan.clone(),
        source_advertisement.clone(),
        sink_advertisement.clone(),
    )?;
    let listener = bind_listener()?;
    let url = listener.url().map_err(|error| format!("{error:?}"))?;
    writeln!(
        output,
        "browser_product url={url} source_host={} source_boot={} browser_host={} browser_boot={} plan={}",
        source_advertisement.host_id.as_str(),
        source_advertisement.boot_id.as_str(),
        sink_advertisement.host_id.as_str(),
        sink_advertisement.boot_id.as_str(),
        plan.plan_id.as_str(),
    )
    .map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())?;
    source
        .run_report_with_peer_timeout(listener, std::time::Duration::from_secs(10), output)
        .map(|report| report.observations)
}
