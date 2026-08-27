//! Exact four-Host realization of the reviewed three-region Lenia Back.

use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
};
use conduit_core::{
    bind_active_play, process_owned_line_offer_with_limits, ArtifactId, BaseImplementationId,
    BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostAdvertisement,
    HostId, HostProfileId, ImplementationId, ImplementationOffer, LineOffer, LinkLimits,
    OfferGeneration, Plan, PlannedConnection, PROTOCOL_VERSION,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};

use crate::{expanded_three_region_lenia, LENIA_REGION_RESULT_INFO_ID, LENIA_REGION_WORK_INFO_ID};

pub const DISTRIBUTED_LENIA_STD_HOST_ID: &str = "std/distributed-lenia-coordinator";
pub const DISTRIBUTED_LENIA_STD_BOOT_ID: &str = "std/distributed-lenia/image-boot";
pub const DISTRIBUTED_LENIA_WROOM_HOST_ID: &str = "esp32/24dcc39a0a44";
pub const DISTRIBUTED_LENIA_WROOM_BOOT_ID: &str = "esp32/wroom/lenia-image-boot";
pub const DISTRIBUTED_LENIA_C3_HOST_ID: &str = "esp32/c04e30ee5ca8";
pub const DISTRIBUTED_LENIA_C3_BOOT_ID: &str = "esp32/c3/lenia-image-boot";
pub const DISTRIBUTED_LENIA_PICO_HOST_ID: &str = "s4/pico-usb-sink";
pub const DISTRIBUTED_LENIA_PICO_BOOT_ID: &str = "pico/lenia-image-boot";
pub const DISTRIBUTED_LENIA_VALUE_BYTES: u32 = 1_024;
pub const DISTRIBUTED_LENIA_FRAME_BYTES: u32 = 2_048;

#[derive(Debug, Clone)]
pub struct DistributedLeniaPlan {
    pub plan: Plan,
    pub hosts: [HostAdvertisement; 4],
    pub lines: [LineOffer; 6],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedLeniaParticipantBindings {
    pub work: DistributedLeniaLineBinding,
    pub result: DistributedLeniaLineBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedLeniaLineBinding {
    pub plan_id: String,
    pub play_id: String,
    pub line_id: String,
    pub source_host_id: String,
    pub source_boot_id: String,
    pub sink_host_id: String,
    pub sink_boot_id: String,
}

pub fn exact_distributed_lenia_plan() -> Result<DistributedLeniaPlan, String> {
    let expanded = expanded_three_region_lenia()?;
    let mut hosts = [
        host(DISTRIBUTED_LENIA_STD_HOST_ID, DISTRIBUTED_LENIA_STD_BOOT_ID),
        host(
            DISTRIBUTED_LENIA_WROOM_HOST_ID,
            DISTRIBUTED_LENIA_WROOM_BOOT_ID,
        ),
        host(DISTRIBUTED_LENIA_C3_HOST_ID, DISTRIBUTED_LENIA_C3_BOOT_ID),
        host(
            DISTRIBUTED_LENIA_PICO_HOST_ID,
            DISTRIBUTED_LENIA_PICO_BOOT_ID,
        ),
    ];
    for gear in &expanded.gears {
        let host_index = worker_index(gear.gear_id.as_str()).map_or(0, |index| index + 1);
        hosts[host_index]
            .capabilities
            .push(capability(gear, host_index));
    }
    let limits = LinkLimits {
        maximum_in_flight_items: 1,
        maximum_payload_bytes: DISTRIBUTED_LENIA_VALUE_BYTES,
        maximum_buffered_bytes: DISTRIBUTED_LENIA_VALUE_BYTES,
        maximum_frame_bytes: DISTRIBUTED_LENIA_FRAME_BYTES,
    };
    let lines = core::array::from_fn(|index| {
        let participant = index / 2 + 1;
        let result = index % 2 == 1;
        let (source, sink, direction) = if result {
            (&hosts[participant], &hosts[0], "result")
        } else {
            (&hosts[0], &hosts[participant], "work")
        };
        process_owned_line_offer_with_limits(
            &format!("lenia/line/region-{}/{direction}", participant - 1),
            &format!("lenia/binding/region-{}/{direction}", participant - 1),
            BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1"),
            &format!("bluetooth/session/region-{}", participant - 1),
            source,
            sink,
            limits,
        )
    });
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host_index = worker_index(gear.gear_id.as_str()).map_or(0, |index| index + 1);
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: hosts[host_index].host_id.clone(),
                        capability_id: capability_id(gear, host_index),
                    },
                )
            })
            .collect(),
    };
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: DISTRIBUTED_LENIA_VALUE_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(DistributedLeniaPlan { plan, hosts, lines })
}

pub fn distributed_lenia_participant_bindings(
    plan: &Plan,
    participant_host: &str,
    runtime_boot: &str,
) -> Result<DistributedLeniaParticipantBindings, String> {
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == participant_host)
        .ok_or_else(|| format!("missing participant fragment {participant_host}"))?;
    let mut work = None;
    let mut result = None;
    for connection in &fragment.connections {
        let binding = line_binding(plan, connection, participant_host, runtime_boot)?;
        match connection.value_kind.as_str() {
            LENIA_REGION_WORK_INFO_ID => work = Some(binding),
            LENIA_REGION_RESULT_INFO_ID => result = Some(binding),
            _ => {}
        }
    }
    Ok(DistributedLeniaParticipantBindings {
        work: work.ok_or_else(|| String::from("participant lacks work Line"))?,
        result: result.ok_or_else(|| String::from("participant lacks result Line"))?,
    })
}

fn line_binding(
    plan: &Plan,
    connection: &PlannedConnection,
    participant_host: &str,
    runtime_boot: &str,
) -> Result<DistributedLeniaLineBinding, String> {
    let line = connection
        .selected_line
        .as_ref()
        .ok_or_else(|| "distributed Cord lacks selected Line".to_string())?;
    let source_boot = if line.binding.source.host_id.as_str() == participant_host {
        runtime_boot
    } else {
        line.binding.source.boot_id.as_str()
    };
    let sink_boot = if line.binding.sink.host_id.as_str() == participant_host {
        runtime_boot
    } else {
        line.binding.sink.boot_id.as_str()
    };
    let source_play = bind_active_play(
        &plan.plan_id,
        &line.binding.source.host_id,
        &line.binding.source.boot_id,
        0,
    );
    Ok(DistributedLeniaLineBinding {
        plan_id: plan.plan_id.as_str().into(),
        play_id: source_play.active_play_id.as_str().into(),
        line_id: line.line_id.as_str().into(),
        source_host_id: line.binding.source.host_id.as_str().into(),
        source_boot_id: source_boot.into(),
        sink_host_id: line.binding.sink.host_id.as_str().into(),
        sink_boot_id: sink_boot.into(),
    })
}

fn host(host_id: &str, boot_id: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(format!("distributed-lenia/{host_id}@1")),
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

fn capability(gear: &conduit_form::CheckedGear, host_index: usize) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: gear.startup_parameters.clone(),
        shorthand: gear.shorthand.clone(),
        capability_id: capability_id(gear, host_index),
        kind_id: gear.kind_id.clone(),
        kind_contract_revision: gear.kind_contract_revision.clone(),
        inputs: gear.inputs.clone(),
        outputs: gear.outputs.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(format!("lenia/host-{host_index}@1")),
            implementation_id: ImplementationId::from(format!(
                "lenia/host-{host_index}/{}@1",
                gear.kind_id.as_str()
            )),
            artifact_id: ArtifactId::from(format!("lenia/host-{host_index}-image@1")),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: DISTRIBUTED_LENIA_VALUE_BYTES,
        },
    }
}

fn capability_id(gear: &conduit_form::CheckedGear, host_index: usize) -> CapabilityId {
    CapabilityId::from(format!("lenia/host-{host_index}/{}", gear.gear_id.as_str()))
}

fn worker_index(gear_id: &str) -> Option<usize> {
    (0..3).find(|index| gear_id.ends_with(&format!("region{index}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_plan_has_three_workers_and_six_directional_ble_lines() {
        let exact = exact_distributed_lenia_plan().unwrap();
        assert_eq!(exact.hosts.len(), 4);
        assert_eq!(exact.lines.len(), 6);
        assert_eq!(
            exact
                .plan
                .fragments
                .iter()
                .filter(|fragment| fragment.host_id.as_str() != DISTRIBUTED_LENIA_STD_HOST_ID)
                .count(),
            3
        );
        for host in [
            DISTRIBUTED_LENIA_WROOM_HOST_ID,
            DISTRIBUTED_LENIA_C3_HOST_ID,
            DISTRIBUTED_LENIA_PICO_HOST_ID,
        ] {
            let bindings =
                distributed_lenia_participant_bindings(&exact.plan, host, "boot/live").unwrap();
            assert!(bindings.work.line_id.ends_with("/work"));
            assert!(bindings.result.line_id.ends_with("/result"));
            assert_eq!(bindings.work.sink_boot_id, "boot/live");
            assert_eq!(bindings.result.source_boot_id, "boot/live");
        }
    }
}
