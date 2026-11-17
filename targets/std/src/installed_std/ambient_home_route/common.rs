use super::*;
pub(super) fn planned(
    host: &crate::StdHost,
    updates: u32,
) -> (conduit_body::BodyPlan, conduit_body::Wake) {
    planned_with_cadence(host, updates, false)
}
pub(super) fn planned_with_cadence(
    host: &crate::StdHost,
    updates: u32,
    cadence: bool,
) -> (conduit_body::BodyPlan, conduit_body::Wake) {
    planned_source(host, updates, cadence, None, false, false)
}
pub(super) fn planned_clock(
    host: &crate::StdHost,
    updates: u32,
) -> (conduit_body::BodyPlan, conduit_body::Wake) {
    planned_source(host, updates, false, Some(updates), false, false)
}
pub(super) fn planned_clock_limits(
    host: &crate::StdHost,
    updates: u32,
    ticks: u32,
) -> (conduit_body::BodyPlan, conduit_body::Wake) {
    planned_source(host, updates, false, Some(ticks), false, false)
}
pub(super) fn planned_aspects(
    host: &crate::StdHost,
    updates: u32,
    clock: bool,
) -> (conduit_body::BodyPlan, conduit_body::Wake) {
    planned_source(host, updates, false, clock.then_some(updates), true, false)
}
pub(super) fn planned_resident(
    host: &crate::StdHost,
) -> (conduit_body::BodyPlan, conduit_body::Wake) {
    planned_source(host, 1, false, Some(1), true, true)
}
fn planned_source(
    host: &crate::StdHost,
    updates: u32,
    cadence: bool,
    clock_ticks: Option<u32>,
    aspects: bool,
    resident: bool,
) -> (conduit_body::BodyPlan, conduit_body::Wake) {
    let real_clock = clock_ticks.is_some();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profiles)
        .unwrap();
    conduit_ambient_display::install_ambient_home_contributor(&mut startup, &mut profiles).unwrap();
    let source = include_str!("../../../../../plots/ambient-home/main.conduit").replace(
        "face/ambient-home(\"Home\")",
        &format!("face/ambient-home(\"Home\", {updates})"),
    );
    conduit_ambient_display::install_ambient_observe(&mut startup, &mut profiles).unwrap();
    let source = if real_clock {
        include_str!("../../../../../plots/ambient-home/clock.conduit")
            .replace("ambient-clock-home", "ambient-home")
            .replace("1440", &updates.to_string())
            .replace(
                &format!("count = {updates}"),
                &format!("count = {}", clock_ticks.unwrap()),
            )
            .replace("60000", "0")
    } else {
        source
    };
    let source = if aspects {
        include_str!("../../../../../plots/ambient-home/aspects.conduit")
            .replace(
                if real_clock {
                    "ambient-observed-aspects"
                } else {
                    "ambient-aspects"
                },
                "ambient-home",
            )
            .replace("1440", &updates.to_string())
            .replace("60000", "0")
    } else {
        source
    };
    let source = if resident {
        include_str!("../../../../../plots/ambient-home/resident.conduit")
            .replace("ambient-resident", "ambient-home")
            .replace("1000ms", "1ms")
    } else {
        source
    };
    conduit_time::install_time_every_catalog(&mut startup, &mut profiles).unwrap();
    conduit_time::install_tick_catalog(&mut startup, &mut profiles).unwrap();
    let source = if cadence {
        source.replace(
            "    >> observations:",
            "    cadence: value/tick@1...| >>\n    >> observations:",
        ).replace("    encounter:", "    clock: time/tick(count = 2, period-ms = 0)\n    clock.tick >> cadence\n    encounter:")
    } else {
        source
    };
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "ambient-home", &profiles)
            .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let mut boundary = BTreeMap::from([(
        conduit_planner::ForeBoundaryKey {
            direction: PortDirection::Input,
            front_port_id: "observations".into(),
            track: ConnectionTrack::Payload,
        },
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 8192,
        },
    )]);
    if real_clock {
        boundary.clear();
    }
    if cadence {
        boundary.insert(
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: "cadence".into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 32,
            },
        );
    }
    let connection_limits = if real_clock {
        expanded
            .expanded
            .connections
            .iter()
            .map(|cord| {
                (
                    (
                        cord.source_gear_id.clone(),
                        cord.source_port_id.clone(),
                        cord.sink_gear_id.clone(),
                        cord.sink_port_id.clone(),
                    ),
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: if cord.source_port_id.as_str() == "tick" {
                            8
                        } else {
                            8192
                        },
                    },
                )
            })
            .collect()
    } else {
        BTreeMap::new()
    };
    let grants = host
        .native_source_authority_grants()
        .cloned()
        .collect::<Vec<_>>();
    let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8192,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connection_limits,
        &boundary,
    )
    .unwrap();
    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "home/born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "home/wake".into()).unwrap().1;
    let plan = conduit_body::BodyPlan::seal(
        &wake,
        vec![conduit_body::BodyPlotPlan {
            plot: conduit_body::ResidentPlot::new(
                plan.source_document_id.clone(),
                plan.checked_plot_id.clone(),
            ),
            plan,
        }],
    )
    .unwrap();
    (plan, wake)
}
pub(super) fn input() -> ExternalForeInput {
    let model = PreparedAmbientFaces::prepare().unwrap();
    let picture = fixtures::observed_wallpaper(&model, "available", 301_000);
    let clock = fixtures::clock(&model, "fresh", 150, 150);
    let observation = fixtures::record(&model.home_type().value_type, |name, _| match name {
        "wallpaper" => picture.clone(),
        "clock_weather" => clock.clone(),
        _ => panic!("Home field"),
    });
    ExternalForeInput {
        front_port_id: "observations".into(),
        track: ConnectionTrack::Payload,
        bytes: observation.canonical_bytes().unwrap(),
    }
}
pub(super) fn host(heap_bytes: u64) -> crate::StdHost {
    host_options(heap_bytes, false, false, false)
}
pub(super) fn clock_host(heap_bytes: u64) -> crate::StdHost {
    host_options(heap_bytes, true, false, false)
}
pub(super) fn aspects_host(heap_bytes: u64, real_clock: bool) -> crate::StdHost {
    host_options(heap_bytes, real_clock, true, false)
}
pub(super) fn resident_host(heap_bytes: u64) -> crate::StdHost {
    host_options(heap_bytes, true, true, true)
}
fn host_options(
    heap_bytes: u64,
    real_clock: bool,
    aspects: bool,
    resident: bool,
) -> crate::StdHost {
    use conduit_host_make::*;
    let mut configuration = parse_host_configuration_conduit(include_str!(
        "../../../profiles/raspberry-pi-5-measurement-cpu.host.conduit"
    ))
    .unwrap();
    configuration.target = ConfigurationTarget {
        architecture: std::env::consts::ARCH.into(),
        machine: "computer".into(),
        board: None,
        os: Some("linux".into()),
        make_descriptor: None,
    };
    configuration.capabilities = vec![
        CapabilityIntent {
            kind: "time/tick".into(),
            pinned_implementation: Some("std/kernel-time-tick@2".into()),
        },
        CapabilityIntent {
            kind: conduit_ambient_display::AMBIENT_HOME_CONTRIBUTOR_KIND.into(),
            pinned_implementation: Some(crate::ambient_home_offer::IMPLEMENTATION.into()),
        },
    ];
    if aspects {
        configuration
            .capabilities
            .retain(|c| c.kind != conduit_ambient_display::AMBIENT_HOME_CONTRIBUTOR_KIND);
        for aspect in [
            conduit_ambient_display::AmbientDisplayAspect::Wallpaper,
            conduit_ambient_display::AmbientDisplayAspect::ClockWeather,
        ] {
            configuration.capabilities.push(CapabilityIntent {
                kind: aspect.kind().into(),
                pinned_implementation: Some(
                    conduit_std_offers::ambient_aspect_identity(aspect).0.into(),
                ),
            });
        }
    }
    if real_clock {
        configuration.capabilities.push(CapabilityIntent {
            kind: conduit_ambient_display::AMBIENT_OBSERVE_KIND.into(),
            pinned_implementation: Some(conduit_std_offers::AMBIENT_OBSERVE_IMPLEMENTATION.into()),
        });
    }
    if resident {
        configuration.capabilities = vec![
            CapabilityIntent {
                kind: "time/every".into(),
                pinned_implementation: Some(conduit_std_offers::TIME_EVERY_IMPLEMENTATION.into()),
            },
            CapabilityIntent {
                kind: conduit_ambient_display::AMBIENT_RESIDENT_OBSERVE_KIND.into(),
                pinned_implementation: Some(
                    conduit_std_offers::AMBIENT_RESIDENT_OBSERVE_IMPLEMENTATION.into(),
                ),
            },
        ];
        for aspect in [
            conduit_ambient_display::AmbientDisplayAspect::Wallpaper,
            conduit_ambient_display::AmbientDisplayAspect::ClockWeather,
        ] {
            configuration.capabilities.push(CapabilityIntent {
                kind: conduit_ambient_display::ambient_resident_aspect_name(aspect)
                    .unwrap()
                    .into(),
                pinned_implementation: Some(
                    conduit_std_offers::ambient_resident_aspect_identity(aspect)
                        .0
                        .into(),
                ),
            });
        }
    }
    let workspace =
        crate::ambient_home_offer::workspace_bytes().unwrap() * if aspects { 2 } else { 1 };
    configuration
        .bases
        .iter_mut()
        .find(|base| base.kind == conduit_host_hosted::CPU_BASE_KIND)
        .unwrap()
        .initialization
        .as_mut()
        .unwrap()
        .values
        .insert("memory_bytes".into(), serde_json::json!(workspace));
    let budget = configuration
        .resources
        .iter_mut()
        .find(|r| r.id == "memory/measurement")
        .unwrap();
    budget.slots = workspace;
    budget.bytes = u64::from(workspace);
    configuration.limits.heap_arena_bytes = heap_bytes;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(configuration, &catalog, &packages).unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/ambient-cpu".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    crate::StdHost::from_selected_timer_and_cpu_image(
        crate::StdHostConfig {
            host_id: "fixture/ambient".into(),
            boot_id: "boot/ambient".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &image,
        &catalog,
        &checked,
    )
    .unwrap()
}

pub(super) fn clock_input(unix_ms: u64) -> ExternalForeInput {
    // Supplied deterministic timestamps, not a claim of actual clock sampling.
    let model = PreparedAmbientFaces::prepare().unwrap();
    let clock =
        crate::ambient_clock::PreparedAmbientClockWeather::prepare("America/Los_Angeles").unwrap();
    let weather = crate::ambient_weather::PreparedOpenMeteoWeather::prepare("Seattle", 900_000)
        .unwrap()
        .unavailable("Fenced live-input fixture: provider unavailable")
        .unwrap();
    let clock = clock.snapshot(unix_ms, &weather).unwrap();
    let picture = fixtures::observed_wallpaper(&model, "unavailable", 301_000);
    let value = fixtures::record(&model.home_type().value_type, |name, _| match name {
        "wallpaper" => picture.clone(),
        "clock_weather" => clock.clone(),
        _ => panic!("Home field"),
    });
    ExternalForeInput {
        front_port_id: "observations".into(),
        track: ConnectionTrack::Payload,
        bytes: value.canonical_bytes().unwrap(),
    }
}
