//! Correlate the ordinary sealed Plan and completed Play with boot evidence.
use super::*;

pub(super) fn validate_observatory(
    boot: &GuestBootSign,
    kernel: &GuestKernelSign,
    presentation: &GuestPresentationSign,
    domain_cost: &serde_json::Value,
    snapshot: &conduit_observatory::ObservatorySnapshot,
) -> Result<(), ConduitosError> {
    use conduit_observatory::{BootProofClass, OperationalState, PlanLifecycle};

    conduit_observatory::validate_snapshot(snapshot)
        .map_err(|error| ConduitosError::refusal("invalid-observatory-snapshot", error))?;
    validate_protected_realization(snapshot, domain_cost)?;
    super::super::ordinary_source_conformance::capture(&snapshot.plans[0])?;
    let host = snapshot.hosts.first();
    let plan = snapshot.plans.first();
    let play = snapshot.plays.first();
    let provenance = snapshot.sealed_boot_provenance.first();
    let expected_artifact = format!("conduitos-build/{}", boot.build_id);
    let exact_placement = |kind: &str, contract: &str, implementation: &str| {
        plan.and_then(|plan| plan.fragments.first())
            .and_then(|fragment| {
                fragment
                    .placements
                    .iter()
                    .find(|placement| placement.kind_id.as_str() == kind)
            })
            .is_some_and(|placement| {
                let runtime_memory = placement.resources.iter().any(|resource| {
                    resource.class_id.as_str() == "conduit.resource/runtime-memory@1"
                        && host.is_some_and(|host| {
                            host.advertisement.capabilities.iter().any(|capability| {
                                capability.kind_id == placement.kind_id
                                    && capability.resource_requirements.iter().any(|requirement| {
                                        requirement.class_id == resource.class_id
                                            && requirement.units == resource.units
                                    })
                            })
                        })
                });
                let exact_effect_realization = match kind {
                    "time/tick" => {
                        placement.resources.len() == 2
                            && placement.resources.iter().any(|resource| {
                                resource.class_id.as_str() == "conduit.resource/timer-slot@1"
                                    && resource.units == 1
                            })
                            && placement.host_calls.len() == 1
                            && placement.host_calls[0].contract_id.as_str() == "conduit.host/wait@1"
                    }
                    "presentation/tick" => {
                        placement.resources.len() == 2
                            && placement.resources.iter().any(|resource| {
                                resource.class_id.as_str() == "conduit.resource/presentation-slot@1"
                                    && resource.units == 1
                            })
                            && placement.host_calls.len() == 1
                            && placement.host_calls[0].contract_id.as_str()
                                == "conduit.host/present@1"
                    }
                    "text/literal" => {
                        placement.resources.len() == 1 && placement.host_calls.is_empty()
                    }
                    "text/upper" => {
                        placement.resources.len() == 1
                            && placement.host_calls.len() == 1
                            && placement.host_calls[0].contract_id.as_str()
                                == "conduit.host/text-upper@1"
                            && placement.host_calls[0].maximum_in_flight == 1
                            && placement.host_calls[0].maximum_input_bytes == 256
                            && placement.host_calls[0].maximum_output_bytes == 256
                    }
                    "presentation/text" => {
                        placement.resources.len() == 2
                            && placement.resources.iter().any(|resource| {
                                resource.class_id.as_str() == "conduit.resource/presentation-slot@1"
                                    && resource.units == 1
                            })
                            && placement.host_calls.len() == 1
                            && placement.host_calls[0].contract_id.as_str()
                                == "conduit.host/present@1"
                            && placement.host_calls[0].maximum_in_flight == 1
                            && placement.host_calls[0].maximum_input_bytes == 256
                    }
                    _ => false,
                };
                placement.kind_contract_revision.as_str() == contract
                    && placement.execution_profile_id.as_str()
                        == "conduitos/single-lane-cooperative@1"
                    && placement.implementation_id.as_str() == implementation
                    && placement.artifact_id.as_str() == expected_artifact
                    && placement.host_id.as_str() == boot.host_id
                    && placement.boot_id.as_str() == boot.boot_id
                    && placement.offer_generation.0 == 1
                    && runtime_memory
                    && exact_effect_realization
            })
    };
    let exact_text_placements = exact_placement(
        "text/literal",
        "conduit.std/text-literal@1",
        "conduitos/kernel-text-literal@1",
    ) && exact_placement(
        "text/upper",
        "conduit.std/text-upper@1",
        "conduitos/kernel-text-upper@1",
    ) && exact_placement(
        "presentation/text",
        "conduit.std/presentation-text@1",
        "conduitos/kernel-serial-text@1",
    );
    let exact_tick_placements = exact_placement(
        "time/tick",
        conduit_time::TICK_CONTRACT_REVISION,
        "conduitos/kernel-time-tick@1",
    ) && exact_placement(
        "presentation/tick",
        conduit_semantic_catalog::TICK_PRESENTATION_CONTRACT_REVISION,
        "conduitos/kernel-serial-tick@1",
    );
    let advertised_bases = host
        .map(|host| host.advertisement.bases.as_slice())
        .unwrap_or_default();
    let bases_match = snapshot.bases.len()
        == expected_base_ids(
            &kernel.base_ids,
            advertised_bases,
            &presentation.display_base_id,
        )
        .len()
        && snapshot.bases.iter().all(|base| {
            base.host_id.as_str() == boot.host_id
                && base.boot_id.as_str() == boot.boot_id
                && base.state == OperationalState::Available
                && (kernel.base_ids.iter().any(|id| id == base.base_id.as_str())
                    || base.base_id.as_str() == presentation.display_base_id
                    || advertised_bases
                        .iter()
                        .any(|advertised| advertised.base_id == base.base_id))
        });
    let exact_base = |kind: &str, capacity: u64| {
        snapshot
            .bases
            .iter()
            .any(|base| base.kind_id.as_str() == kind && base.capacity_units == capacity)
    };
    let base_inventory = exact_base("conduitos.base/memory@1", boot.runtime_arena_bytes)
        && exact_base("conduitos.base/clock@1", 1)
        && exact_base("conduitos.base/timer@1", 1)
        && exact_base("serial", 2)
        && exact_base("conduitos.base/interrupt@1", 4)
        && exact_base("conduitos.base/idle@1", 1)
        && exact_base("conduitos.base/execution-lane@1", 2)
        && exact_base(
            "conduitos.base/framebuffer@1",
            u64::from(presentation.display_pitch) * u64::from(presentation.display_height),
        );
    let current_signs = snapshot
        .observations
        .iter()
        .filter(|sign| {
            matches!(
                sign.kind,
                conduit_core::ObservationKind::PlacementTerminal { .. }
                    | conduit_core::ObservationKind::ConnectionTerminal { .. }
                    | conduit_core::ObservationKind::PlanTerminal { .. }
            )
        })
        .count()
        == 9;
    let overlap_sign = snapshot.observations.iter().any(|sign| {
        matches!(
            &sign.kind,
            conduit_core::ObservationKind::ExecutionRegionOverlap {
                waiting_region_id,
                progressing_region_id,
                physical_parallelism: false,
            } if waiting_region_id.as_str() == "region/timer"
                && progressing_region_id.as_str() == "region/text"
        )
    });
    let historical_signs = [
        conduit_core::ObservationKind::HostStarted,
        conduit_core::ObservationKind::AdvertisementPublished,
        conduit_core::ObservationKind::PlanFragmentReceived,
        conduit_core::ObservationKind::PlanPlayStarted,
    ]
    .iter()
    .all(|kind| {
        snapshot
            .historical_observations
            .iter()
            .any(|sign| core::mem::discriminant(&sign.kind) == core::mem::discriminant(kind))
    }) && snapshot
        .historical_observations
        .iter()
        .filter(|sign| matches!(sign.kind, conduit_core::ObservationKind::PlacementPrepared))
        .count()
        == 5;
    if snapshot.hosts.len() != 1
        || !snapshot.lines.is_empty()
        || snapshot.plans.len() != 1
        || snapshot.plays.len() != 1
        || snapshot.observations.len() != 10
        || snapshot.historical_observations.len() != 9
        || snapshot.sealed_boot_provenance.len() != 1
        || !bases_match
        || !base_inventory
        || !exact_text_placements
        || !exact_tick_placements
        || !current_signs
        || !overlap_sign
        || !historical_signs
        || host.is_none_or(|host| {
            host.advertisement.host_id.as_str() != boot.host_id
                || host.advertisement.boot_id.as_str() != boot.boot_id
                || host.advertisement.profile.as_str() != kernel.scheduler_profile
                || host.advertisement.capabilities.len() != 6
                || !host.advertisement.capabilities.iter().any(|capability| {
                    capability.kind_id.as_str() == "text/upper"
                        && capability.implementation.implementation_id.as_str()
                            == "conduitos/kernel-text-upper@1"
                        && capability.limits.max_queue_items == 4
                        && capability.limits.max_queue_bytes == 256
                })
                || !host.advertisement.capabilities.iter().any(|capability| {
                    capability.kind_id.as_str() == "text/literal"
                        && capability.implementation.implementation_id.as_str()
                            == "conduitos/kernel-text-literal@1"
                        && capability.limits.max_queue_items == 4
                        && capability.limits.max_queue_bytes == 256
                })
                || !host.advertisement.capabilities.iter().any(|capability| {
                    capability.kind_id.as_str() == "presentation/text"
                        && capability.implementation.implementation_id.as_str()
                            == "conduitos/kernel-serial-text@1"
                        && capability.limits.max_queue_items == 4
                        && capability.limits.max_queue_bytes == 256
                })
                || !host.advertisement.planner_capabilities.is_empty()
                || host.advertisement.resources.len() != 12
                || host.state != OperationalState::Available
        })
        || plan.is_none_or(|plan| {
            plan.plan_id.as_str() != kernel.plan_id
                || plan.source_document_id.as_str() != kernel.source_document_id
                || plan.checked_plot_id.as_str() != kernel.checked_plot_id
                || plan.expanded_plot_id.as_str() != kernel.expanded_plot_id
                || plan.fragments.len() != 1
                || plan.fragments[0].fragment_id.as_str() != kernel.fragment_id
                || plan.fragments[0].placements.len() != 5
                || plan.fragments[0].connections.len() != 3
                || plan.fragments[0].execution_regions.len() != 2
                || plan.fragments[0].execution_regions[0].region_id.as_str() != "region/text"
                || plan.fragments[0].execution_regions[1].region_id.as_str() != "region/timer"
                || plan.fragments[0].execution_regions[0]
                    .lane_resource
                    .pool_id
                    .as_str()
                    != kernel.lane_resource_ids[0]
                || plan.fragments[0].execution_regions[1]
                    .lane_resource
                    .pool_id
                    .as_str()
                    != kernel.lane_resource_ids[1]
                || plan.fragments[0].execution_regions.iter().any(|region| {
                    region.lane_base_id.as_str() != kernel.lane_base_id
                        || region.lane_count != 1
                        || region.preemption_required
                            != (region.region_id.as_str() == "region/text")
                        || region.isolation_required != (region.region_id.as_str() == "region/text")
                })
                || plan.fragments[0]
                    .connections
                    .iter()
                    .map(|connection| u32::from(connection.item_capacity))
                    .sum::<u32>()
                    != u32::from(kernel.cord_item_capacity)
                || plan.fragments[0]
                    .connections
                    .iter()
                    .map(|connection| connection.byte_capacity)
                    .sum::<u32>()
                    != kernel.cord_byte_capacity
        })
        || play.is_none_or(|play| {
            play.active_play_id.as_str() != kernel.active_play_id
                || play.plan_id.as_str() != kernel.plan_id
                || play.host_id.as_str() != boot.host_id
                || play.boot_id.as_str() != boot.boot_id
                || play.lifecycle != PlanLifecycle::Completed
                || play.placements.len() != 5
                || play.connections.len() != 3
                || play.connections.iter().any(|connection| {
                    connection.pressure.as_ref().is_none_or(|pressure| {
                        pressure.current_in_flight_items != Some(0)
                            || pressure.current_buffered_bytes != Some(0)
                            || pressure.pressure_events != 0
                            || pressure.last_pressure_sequence.is_some()
                    })
                })
        })
        || snapshot.retention.item_capacity != 64
        || snapshot.retention.retained_items != 19
        || snapshot.retention.dropped_items != 0
        || provenance.is_none_or(|provenance| {
            provenance.host_id.as_str() != boot.host_id
                || provenance.boot_id.as_str() != boot.boot_id
                || provenance.firmware_environment != boot.firmware
                || provenance.adapter_name != "Limine"
                || provenance.adapter_version != boot.limine
                || provenance.adapter_revision != "3"
                || provenance.image_id.as_str() != boot.image_binding
                || provenance.build_id.as_str() != boot.build_id
                || provenance.memory_map.normalized_region_count != boot.memory_regions
                || provenance.memory_map.runtime_arena_bytes != boot.runtime_arena_bytes
                || !provenance.boot_artifacts.is_empty()
                || provenance.initial_plan_artifact_id.is_some()
                || provenance.recovery_plan_artifact_id.is_some()
                || provenance.framebuffers.len() != 1
                || provenance.framebuffers[0].base_id.as_str() != presentation.display_base_id
                || provenance.framebuffers[0].width != presentation.display_width
                || provenance.framebuffers[0].height != presentation.display_height
                || provenance.framebuffers[0].pitch_bytes != presentation.display_pitch
                || provenance.framebuffers[0].bits_per_pixel != presentation.display_bits_per_pixel
                || provenance.proof_class != BootProofClass::FreestandingEmulator
        })
    {
        return Err(ConduitosError::refusal(
            "invalid-observatory-snapshot",
            "ordinary Observatory identities, bounds, lifecycle, or sealed provenance disagreed with boot/kernel Signs",
        ));
    }
    let report = conduit_observatory::build_report(snapshot)
        .map_err(|error| ConduitosError::refusal("observatory-report-refused", error))?;
    let linear = conduit_observatory::render_text_report(&report);
    for required in [
        boot.host_id.as_str(),
        boot.boot_id.as_str(),
        kernel.plan_id.as_str(),
        kernel.active_play_id.as_str(),
        "input/keyboard",
        "conduitos/usb-hid-keyboard@1",
        "boot provenance [sealed]",
    ] {
        if !linear.contains(required) {
            return Err(ConduitosError::refusal(
                "observatory-report-incomplete",
                format!("ordinary Observatory report omitted {required}"),
            ));
        }
    }
    Ok(())
}

fn expected_base_ids<'a>(
    machine: &'a [String],
    advertised: &'a [conduit_core::BaseProviderAdvertisement],
    display: &'a str,
) -> std::collections::BTreeSet<&'a str> {
    machine
        .iter()
        .map(String::as_str)
        .chain(advertised.iter().map(|base| base.base_id.as_str()))
        .chain(core::iter::once(display))
        .collect()
}

fn validate_protected_realization(
    snapshot: &conduit_observatory::ObservatorySnapshot,
    cost: &serde_json::Value,
) -> Result<(), ConduitosError> {
    let valid = snapshot
        .plans
        .first()
        .and_then(|plan| plan.fragments.first())
        .is_some_and(|fragment| {
            fragment.execution_regions.iter().all(|region| {
                let protected = region.region_id.as_str() == "region/text";
                region.preemption_required == protected
                    && region.isolation_required == protected
                    && region.execution_profile_id.as_str()
                        == if protected {
                            conduitos::ordinary_plan::PROTECTED_REGION_PROFILE
                        } else {
                            conduitos::ordinary_plan::COOPERATIVE_REGION_PROFILE
                        }
            }) && fragment.placements.iter().all(|placement| {
                let expected = if placement.kind_id.as_str() == "text/upper" {
                    cost["reserved_bytes"].as_u64().and_then(|reserved| {
                        cost["root_metadata_bytes"]
                            .as_u64()
                            .and_then(|metadata| reserved.checked_add(metadata))
                            .and_then(|total| total.checked_add(4096))
                    })
                } else {
                    Some(4096)
                };
                placement.resources.iter().any(|resource| {
                    resource.class_id.as_str() == "conduit.resource/runtime-memory@1"
                        && expected.is_some_and(|minimum| {
                            // Preparation reserves a finite ceiling for the two
                            // binding copies; the receipt measures their used bytes.
                            let bytes = u64::from(resource.units);
                            bytes >= minimum
                                && bytes <= minimum + if minimum == 4096 { 0 } else { 640 }
                        })
                })
            })
        });
    if !valid {
        return Err(ConduitosError::refusal("invalid-observatory-protection",
            "ordinary text protection or admitted domain memory disagreed with its measured realization"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
