//! Exact dynamic endpoint identity reconstruction for the browser fragment.

use super::PlanKind;
use conduit_core::{BootId, HostAdvertisement, HostId, Plan};

pub(super) fn advertisement_and_plan(
    kind: PlanKind,
    source_identity: Option<(HostId, BootId)>,
    sink_identity: Option<(HostId, BootId)>,
) -> Result<(HostAdvertisement, Plan), String> {
    let mut advertisement = match kind {
        PlanKind::StdBrowser => conduit_signal::distributed_browser_sink_advertisement(),
        PlanKind::Triple => conduit_signal::triple::browser_advertisement(),
    };
    if let Some((host_id, boot_id)) = &sink_identity {
        advertisement.host_id = host_id.clone();
        advertisement.boot_id = boot_id.clone();
    }
    let plan = match (kind, source_identity) {
        (PlanKind::Triple, _) => conduit_signal::triple::exact_plan().map(|exact| exact.plan),
        (PlanKind::StdBrowser, Some((source_host, source_boot))) => {
            let (sink_host, sink_boot) = sink_identity.unwrap_or_else(|| {
                (
                    HostId::from(conduit_signal::DISTRIBUTED_BROWSER_HOST_ID),
                    BootId::from(conduit_signal::DISTRIBUTED_BROWSER_BOOT_ID),
                )
            });
            conduit_signal::exact_distributed_signal_plan_for_endpoints(
                source_host,
                source_boot,
                sink_host,
                sink_boot,
            )
            .map(|exact| exact.plan)
        }
        (PlanKind::StdBrowser, None) => {
            conduit_signal::exact_distributed_signal_plan().map(|exact| exact.plan)
        }
    }?;
    Ok((advertisement, plan))
}
