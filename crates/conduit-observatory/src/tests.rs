use alloc::collections::BTreeMap;
use alloc::vec;

use super::{
    build_report, render_text_report, unsupported_state, CapabilityAvailability,
    CapabilityStatusReport, CapabilitySupport, HostReport, LinkReport, ObservatorySnapshot,
    OfferFreshness, OperationalState, PlanLifecycle, PlayConnectionReport, PlayPlacementReport,
    PlayReport, PressureReport, RetentionReport, SNAPSHOT_SCHEMA,
};
use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
use conduit_core::{
    authority_grant, present_authority_requirement, process_owned_link_binding, BootId,
    CapabilityId, ConnectionBase, GearId, HostCommand, HostId, ObservationKind, OfferGeneration,
    TerminalDisposition,
};
use conduit_form::parse;
use conduit_pico_sim::{pico_advertisement, PicoSimConfig};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
use conduit_signal::{exact_std_pico_usb_plan, signal_profile_catalog};
use conduit_std_host::{LegacyStdFixtureHost, StdHostConfig};

#[test]
fn report_separates_identity_capability_plan_connection_and_clue_tables() {
    let mut std_host = LegacyStdFixtureHost::new_with_config(StdHostConfig {
        host_id: HostId::from("std-host-triple"),
        boot_id: BootId::from("std-boot-triple"),
        offer_generation: OfferGeneration(1),
    });
    let page = BrowserSimPage::with_hosts([BrowserSimConfig {
        host_id: HostId::from("browser-sim-triple"),
        boot_id: BootId::from("browser-sim-boot-triple"),
        offer_generation: OfferGeneration(1),
    }]);
    let pico_ad = pico_advertisement(PicoSimConfig {
        host_id: HostId::from("pico-sim-triple"),
        boot_id: BootId::from("pico-sim-boot-triple"),
        offer_generation: OfferGeneration(1),
    });
    let mut browser_ad = page
        .advertisements()
        .into_iter()
        .next()
        .expect("browser advertisement exists");
    let browser_host_id = browser_ad.host_id.clone();
    let browser_boot_id = browser_ad.boot_id.clone();
    let browser_capability = browser_ad
        .capabilities
        .iter_mut()
        .find(|capability| capability.capability_id == CapabilityId::from("dom-show"))
        .expect("browser presentation capability exists");
    let presentation_subject = browser_capability
        .host_operations
        .iter()
        .find_map(|requirement| requirement.target_kind.clone())
        .expect("browser presentation declares a target subject");
    let authority_requirement = present_authority_requirement(presentation_subject);
    browser_capability
        .authority_requirements
        .push(authority_requirement.clone());
    let browser_authority_grant = authority_grant(
        "grant/browser-presentation",
        &authority_requirement,
        browser_host_id,
        browser_boot_id,
        browser_capability.capability_id.clone(),
    );
    let advertisements = vec![
        std_host.advertisement().clone(),
        browser_ad.clone(),
        pico_ad.clone(),
    ];

    let form = parse(
        include_str!("../../../examples/triple-signal.form"),
        &signal_profile_catalog(),
    )
    .expect("triple form parses");
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("pulse"),
                PlacementChoice {
                    host_id: HostId::from("std-host-triple"),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                GearId::from("local"),
                PlacementChoice {
                    host_id: HostId::from("std-host-triple"),
                    capability_id: CapabilityId::from("stdout-show-1"),
                },
            ),
            (
                GearId::from("web"),
                PlacementChoice {
                    host_id: HostId::from("browser-sim-triple"),
                    capability_id: CapabilityId::from("dom-show"),
                },
            ),
            (
                GearId::from("light"),
                PlacementChoice {
                    host_id: HostId::from("pico-sim-triple"),
                    capability_id: CapabilityId::from("onboard-led"),
                },
            ),
        ]),
    };
    let connection_bases = BTreeMap::from([
        (
            (GearId::from("pulse"), GearId::from("local")),
            ConnectionBase::Local,
        ),
        (
            (GearId::from("pulse"), GearId::from("web")),
            ConnectionBase::FixtureFrame,
        ),
        (
            (GearId::from("pulse"), GearId::from("light")),
            ConnectionBase::FixtureDatagram,
        ),
    ]);
    let link_bindings = vec![
        process_owned_link_binding(
            "link/std-browser",
            ConnectionBase::FixtureFrame,
            "fixture/frame/std-browser",
            &advertisements[0],
            &advertisements[1],
            4,
            64,
        ),
        process_owned_link_binding(
            "link/std-pico",
            ConnectionBase::FixtureDatagram,
            "fixture/datagram/std-pico",
            &advertisements[0],
            &advertisements[2],
            4,
            64,
        ),
    ];
    let plan = plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[
            ConnectionBase::Local,
            ConnectionBase::FixtureFrame,
            ConnectionBase::FixtureDatagram,
        ],
        PlanningOptions {
            connection_bases: &connection_bases,
            route_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 64,
            authority_grants: core::slice::from_ref(&browser_authority_grant),
            protected_resource_grants: &[],
            link_bindings: &link_bindings,
        },
    )
    .expect("M1 triple-simulation plan resolves");
    std_host.replace_link_bindings(link_bindings.clone());
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == HostId::from("std-host-triple"))
        .expect("std fragment exists")
        .clone();
    let _ = std_host.handle(HostCommand::Prepare(fragment.clone()));
    let _ = std_host.handle(HostCommand::StartPlay(fragment.plan_id.clone()));
    let observations = std_host
        .handle(HostCommand::Inspect)
        .events
        .into_iter()
        .find_map(|event| match event {
            conduit_core::HostEvent::Observations { items } => Some(items),
            _ => None,
        })
        .expect("inspect returns observations");

    let active_play_id = observations
        .iter()
        .find_map(|observation| observation.active_play_id.clone())
        .expect("active Play identity is reported");
    let hosts = advertisements
        .iter()
        .cloned()
        .map(|advertisement| HostReport {
            capabilities: advertisement
                .capabilities
                .iter()
                .map(|capability| CapabilityStatusReport {
                    capability_id: capability.capability_id.clone(),
                    freshness: OfferFreshness::Fresh,
                    support: CapabilitySupport::Supported,
                    availability: CapabilityAvailability::Available,
                })
                .collect(),
            advertisement,
            state: OperationalState::Available,
        })
        .collect();
    let play = PlayReport {
        active_play_id: active_play_id.clone(),
        plan_id: plan.plan_id.clone(),
        host_id: HostId::from("std-host-triple"),
        boot_id: BootId::from("std-boot-triple"),
        lifecycle: PlanLifecycle::Active,
        terminal_disposition: None,
        failure_message: None,
        placements: fragment
            .placements
            .iter()
            .map(|placement| PlayPlacementReport {
                placement_id: placement.placement_id.clone(),
                lifecycle: PlanLifecycle::Prepared,
                terminal_disposition: None,
                failure_message: None,
            })
            .collect(),
        connections: fragment
            .connections
            .iter()
            .map(|connection| PlayConnectionReport {
                connection_id: connection.connection_id.clone(),
                lifecycle: PlanLifecycle::Active,
                terminal_disposition: None,
                pressure: Some(PressureReport {
                    current_in_flight_items: Some(0),
                    current_buffered_bytes: Some(0),
                    pressure_events: 1,
                    last_pressure_sequence: Some(0),
                }),
                failure_message: None,
            })
            .collect(),
    };
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts,
        links: link_bindings
            .into_iter()
            .map(|binding| LinkReport {
                binding,
                state: OperationalState::Available,
            })
            .collect(),
        plans: vec![plan],
        plays: vec![play],
        retention: RetentionReport {
            item_capacity: 64,
            retained_items: observations.len() as u32,
            dropped_items: 0,
        },
        observations,
    };
    let report = build_report(&snapshot).expect("neutral report projects");
    assert_eq!(report.hosts.len(), 3);
    assert_eq!(report.links.len(), 2);
    assert!(report.capabilities.iter().all(|capability| {
        capability.support == CapabilitySupport::Supported
            && capability.availability == CapabilityAvailability::Available
            && capability.freshness == OfferFreshness::Fresh
    }));
    assert_eq!(report.plans.len(), 1);
    assert_eq!(report.plans[0].placement_count, 4);
    assert_eq!(report.plans[0].connection_count, 3);
    assert_eq!(report.placements.len(), 4);
    assert_eq!(report.fragments.len(), 3);
    assert_eq!(report.plays.len(), 1);
    assert_eq!(report.plays[0].active_play_id, active_play_id);
    assert!(report.capabilities.iter().any(|capability| {
        capability.capability_id == CapabilityId::from("dom-show")
            && capability.authority_requirements == vec![authority_requirement.clone()]
    }));
    assert!(report.placements.iter().any(|placement| {
        placement.capability_id == CapabilityId::from("dom-show")
            && placement.authority.len() == 1
            && placement.authority[0].grant_id == browser_authority_grant.grant_id
    }));
    assert_eq!(report.connections.len(), 3);
    assert!(report
        .connections
        .iter()
        .any(|connection| connection.base == ConnectionBase::FixtureFrame));
    assert!(report
        .connections
        .iter()
        .any(|connection| connection.base == ConnectionBase::FixtureDatagram));
    assert!(report.connections.iter().all(|connection| {
        (connection.base == ConnectionBase::Local && connection.link_binding.is_none())
            || (connection.base != ConnectionBase::Local && connection.link_binding.is_some())
    }));
    assert!(report
        .clues
        .iter()
        .all(|row| !row.clue_id.as_str().is_empty()));
    assert!(report.clues.iter().any(|row| {
        row.plan_id == Some(report.plans[0].plan_id.clone())
            && row.active_play_id.is_some()
            && matches!(row.kind, ObservationKind::PlanPlayStarted)
    }));
    assert!(report.retention.bounded);

    let rendered = render_text_report(&report);
    assert!(rendered.contains("host observatory report"));
    assert!(rendered.contains("host id=std-host-triple boot=std-boot-triple"));
    assert!(rendered.contains("capability host=browser-sim-triple"));
    assert!(rendered.contains(conduit_core::PRESENT_HOST_OPERATION_CONTRACT));
    assert!(rendered.contains("presentation/signal"));
    assert!(rendered.contains(conduit_core::PRESENTATION_RESOURCE_CLASS));
    assert!(rendered.contains("browser/presentation"));
    assert!(rendered.contains(conduit_core::PRESENT_AUTHORITY_CONTRACT));
    assert!(rendered.contains("grant/browser-presentation"));
    assert!(rendered.contains("base=FixtureFrame"));
    assert!(rendered.contains("base=FixtureDatagram"));
    assert!(rendered.contains("link/std-browser"));
    assert!(rendered.contains("fixture/datagram/std-pico"));
    assert!(rendered.contains("authority: ProcessOwned"));
    assert!(rendered.contains("plays 1"));
    assert!(rendered.contains("pressure=in_flight=Some(0)"));
    assert!(rendered.contains("active_play="));
    assert!(!rendered.contains("clue id=clue/"));

    let mut state_snapshot = snapshot.clone();
    state_snapshot.hosts[0].state = OperationalState::Stale;
    state_snapshot.hosts[1].state = OperationalState::Unreachable;
    state_snapshot.hosts[2].state = OperationalState::Denied;
    state_snapshot.hosts[0].capabilities[0].support = CapabilitySupport::Unsupported;
    state_snapshot.hosts[0].capabilities[0].availability = CapabilityAvailability::Unavailable;
    state_snapshot.links[0].state = OperationalState::Failed;
    state_snapshot.links[1].state = OperationalState::Unknown;
    state_snapshot.plays[0].lifecycle = PlanLifecycle::Failed;
    state_snapshot.plays[0].terminal_disposition = Some(TerminalDisposition::Failed {
        reason: conduit_core::FailureReason::UnsupportedKind,
    });
    state_snapshot.plays[0].failure_message = Some("installed implementation failed".into());
    state_snapshot.plays[0].connections[0].lifecycle = PlanLifecycle::Failed;
    state_snapshot.plays[0].connections[0].failure_message = Some("sink rejected".into());
    let mut gap = state_snapshot.observations[0].clone();
    gap.clue_id = conduit_core::ClueId::from("host-gap-clue");
    gap.kind = ObservationKind::ClueGap { dropped: 3 };
    state_snapshot.observations.push(gap);
    state_snapshot.retention.retained_items += 1;
    state_snapshot.retention.dropped_items = 2;

    let state_report = build_report(&state_snapshot).expect("distinct states remain valid");
    assert_eq!(state_report.hosts[0].state, OperationalState::Stale);
    assert_eq!(state_report.hosts[1].state, OperationalState::Unreachable);
    assert_eq!(state_report.hosts[2].state, OperationalState::Denied);
    assert_eq!(state_report.links[0].state, OperationalState::Failed);
    assert_eq!(state_report.links[1].state, OperationalState::Unknown);
    assert_eq!(
        state_report.capabilities[0].support,
        CapabilitySupport::Unsupported
    );
    assert_eq!(state_report.retention.visible_gap_count, 5);
    let states = render_text_report(&state_report);
    assert!(states.contains("failure=installed implementation failed"));
    assert!(states.contains("failure=sink rejected"));
    assert!(states.contains("pressure=in_flight=Some(0)"));
}

#[test]
fn status_vocabulary_keeps_failure_modes_distinct() {
    assert_ne!(OperationalState::Stale, OperationalState::Unreachable);
    assert_ne!(OperationalState::Failed, OperationalState::Denied);
    assert_ne!(OperationalState::Unknown, unsupported_state());
    assert_ne!(CapabilitySupport::Unsupported, CapabilitySupport::Unknown);
    assert_ne!(
        CapabilityAvailability::Unavailable,
        CapabilityAvailability::Unknown
    );
    assert_ne!(PlanLifecycle::Failed, PlanLifecycle::Cancelled);
    assert_ne!(
        TerminalDisposition::Completed,
        TerminalDisposition::Failed {
            reason: conduit_core::FailureReason::UnsupportedKind,
        }
    );
}

#[test]
fn projects_exact_std_pico_usb_arrangement_without_promoting_physical_proof() {
    let exact = exact_std_pico_usb_plan().expect("current std/Pico USB plan resolves");
    let hosts = [
        exact.source_advertisement.clone(),
        exact.sink_advertisement.clone(),
    ]
    .into_iter()
    .map(|advertisement| HostReport {
        capabilities: advertisement
            .capabilities
            .iter()
            .map(|capability| CapabilityStatusReport {
                capability_id: capability.capability_id.clone(),
                freshness: OfferFreshness::Fresh,
                support: CapabilitySupport::Supported,
                availability: CapabilityAvailability::Available,
            })
            .collect(),
        advertisement,
        state: OperationalState::Available,
    })
    .collect();
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts,
        links: vec![LinkReport {
            binding: exact.link_binding.clone(),
            state: OperationalState::Available,
        }],
        plans: vec![exact.plan],
        plays: vec![],
        observations: vec![],
        retention: RetentionReport {
            item_capacity: 256,
            retained_items: 0,
            dropped_items: 0,
        },
    };

    let report = build_report(&snapshot).expect("exact S4 arrangement projects");
    assert_eq!(report.hosts.len(), 2);
    assert_eq!(report.fragments.len(), 2);
    assert_eq!(report.links.len(), 1);
    assert_eq!(report.links[0].binding, exact.link_binding);
    let rendered = render_text_report(&report);
    assert!(rendered.contains("base=UsbCdc"));
    assert!(rendered.contains("s4/std-pico-usb-cdc-link"));
    assert!(rendered.contains("profile=rust-std-kernel"));
    assert!(rendered.contains("profile=rp2040-kernel"));
    assert!(rendered.contains("plays 0"));
}
