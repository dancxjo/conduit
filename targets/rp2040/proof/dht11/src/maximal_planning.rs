//! Same indexed planner/lowering checkpoint as ESP32, over actual Pico offers.
extern crate alloc;
use super::{mark, material};
use crate::usb_link::UsbLinkSession;
use alloc::collections::BTreeMap;
use conduit_core as c;
pub async fn measure(link: &mut UsbLinkSession, host: &c::HostAdvertisement) {
    let bytes = material::planning_source_image();
    assert_eq!(
        c::assigned_plan_payload_digest(bytes),
        material::planning_source_sha256()
    );
    // SAFETY: independently checked Source and digest linked by trusted Make;
    // no received bytes or claimed Source identity enters this constructor.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(bytes) }
            .unwrap();
    for index in 0..image.gear_count() {
        mark(link, "before-source-gear", index).await;
        let gear = image.gear(index).unwrap();
        mark(link, "source-gear-live", index).await;
        drop(gear);
    }
    mark(link, "before-indexed-selection", 0).await;
    let choices =
        conduit_planner::indexed_source::default_placements(&image, core::slice::from_ref(host))
            .unwrap();
    mark(link, "after-indexed-selection", choices.by_gear.len()).await;
    let metadata = image.metadata().unwrap();
    let mut queues = BTreeMap::new();
    for cord in metadata.connections() {
        let mut capacity = 1024;
        for gear in [&cord.source_gear_id, &cord.sink_gear_id] {
            let choice = &choices.by_gear[gear];
            let offer = host
                .capabilities
                .iter()
                .find(|o| o.capability_id == choice.capability_id)
                .unwrap();
            if offer.kind_id.as_str()
                == conduit_environment_runtime::publication::PublicationChannel::Reading.kind_id()
            {
                capacity = 4096;
            }
        }
        for gear in [&cord.source_gear_id, &cord.sink_gear_id] {
            let choice = &choices.by_gear[gear];
            let offer = host
                .capabilities
                .iter()
                .find(|o| o.capability_id == choice.capability_id)
                .unwrap();
            capacity = capacity.min(offer.limits.max_queue_bytes);
        }
        queues.insert(
            (
                cord.source_gear_id.clone(),
                cord.source_port_id.clone(),
                cord.sink_gear_id.clone(),
                cord.sink_port_id.clone(),
            ),
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: capacity,
            },
        );
    }
    drop(metadata);
    // Planning policy for the actual retained local providers. These facts
    // confer no dispatch capability; this gate never starts native effects.
    let grants = [
        grant(
            host,
            "pulse",
            conduit_pulse_capture::installation::PULSE_AUTHORITY,
            conduit_pulse_capture::PULSE_READ_CALL,
            conduit_pulse_capture::PULSE_READ_KIND,
            conduit_pulse_capture::installation::PULSE_CAPABILITY,
        ),
        grant(
            host,
            "wait",
            conduit_monotonic_wait::WAIT_AUTHORITY,
            c::WAIT_HOST_CALL_CONTRACT,
            conduit_time::TICK_KIND,
            conduit_monotonic_wait::WAIT_CAPABILITY,
        ),
    ];
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    mark(link, "before-compact-planning", 0).await;
    let compact = conduit_planner::indexed_source::plan_compact_with_connection_limits(
        &image,
        core::slice::from_ref(host),
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 1024,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &queues,
    )
    .unwrap();
    mark(link, "compact-plan-live", compact.fragment_count()).await;
    let plan = compact.into_verified_metadata().unwrap();
    mark(link, "verified-plan-live", plan.fragment_count()).await;
    let id = &plan.preparation_metadata().fragments[0].fragment_id;
    let lowered =
        conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(&plan, id).unwrap();
    mark(link, "lowered-live", lowered.nodes.len()).await;
    let tables = conduit_plan_lowering::local_tables::VerifiedLocalKernelTables::<
        15,
        14,
        1,
        15,
        14,
        15,
    >::prepare(&plan, id, &lowered)
    .unwrap();
    mark(link, "local-tables-live", 15).await;
    core::hint::black_box(tables);
}
fn grant(
    host: &c::HostAdvertisement,
    name: &str,
    contract: &str,
    call: &str,
    kind: &str,
    capability: &str,
) -> c::AuthorityGrant {
    c::AuthorityGrant {
        grant_id: alloc::format!("grant/rp2040/{name}/1").into(),
        contract_id: contract.into(),
        host_call_contract_id: call.into(),
        subject_kind: kind.into(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: capability.into(),
    }
}
