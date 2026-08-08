use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
use conduit_core::{
    process_owned_link_binding, BootId, CapabilityId, ConnectionProvider, HostCommand, HostEvent,
    HostId, OfferGeneration, OperationId,
};
use conduit_form::parse;
use conduit_observatory::{
    build_report, render_text_report, CapabilityAvailability, CapabilityStatusReport,
    CapabilitySupport, HostReport, LinkReport, ObservatorySnapshot, OfferFreshness,
    OperationalState, RetentionReport, SNAPSHOT_SCHEMA,
};
use conduit_pico_sim::{pico_advertisement, PicoSim, PicoSimConfig};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
use conduit_signal::signal_profile_catalog;
use conduit_std_host::{LegacyStdFixtureHost, StdHostConfig};
use std::collections::BTreeMap;

trait HandleInspect {
    fn inspect(&mut self) -> Vec<conduit_core::Observation>;
}

impl HandleInspect for LegacyStdFixtureHost {
    fn inspect(&mut self) -> Vec<conduit_core::Observation> {
        self.handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .unwrap_or_default()
    }
}

impl HandleInspect for PicoSim {
    fn inspect(&mut self) -> Vec<conduit_core::Observation> {
        self.handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .unwrap_or_default()
    }
}

fn inspect(host: &mut impl HandleInspect) -> Vec<conduit_core::Observation> {
    host.inspect()
}

fn observatory_fixture_report() -> Result<String, String> {
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
    let mut pico = PicoSim::new(PicoSimConfig {
        host_id: HostId::from("pico-sim-triple"),
        boot_id: BootId::from("pico-sim-boot-triple"),
        offer_generation: OfferGeneration(1),
    });
    let browser_advertisement = page
        .advertisements()
        .into_iter()
        .next()
        .ok_or_else(|| "browser advertisement missing".to_string())?;
    let pico_advertisement = pico_advertisement(PicoSimConfig {
        host_id: HostId::from("pico-sim-triple"),
        boot_id: BootId::from("pico-sim-boot-triple"),
        offer_generation: OfferGeneration(1),
    });
    let advertisements = vec![
        std_host.advertisement().clone(),
        browser_advertisement.clone(),
        pico_advertisement.clone(),
    ];

    let form = parse(
        include_str!("../examples/triple-signal.form"),
        &signal_profile_catalog(),
    )
    .map_err(|err| err.to_string())?;
    let placements = PlacementChoices {
        by_operation: BTreeMap::from([
            (
                OperationId::from("pulse"),
                PlacementChoice {
                    host_id: HostId::from("std-host-triple"),
                    capability_id: CapabilityId::from("pulse-1"),
                },
            ),
            (
                OperationId::from("local"),
                PlacementChoice {
                    host_id: HostId::from("std-host-triple"),
                    capability_id: CapabilityId::from("stdout-show-1"),
                },
            ),
            (
                OperationId::from("web"),
                PlacementChoice {
                    host_id: HostId::from("browser-sim-triple"),
                    capability_id: CapabilityId::from("dom-show"),
                },
            ),
            (
                OperationId::from("light"),
                PlacementChoice {
                    host_id: HostId::from("pico-sim-triple"),
                    capability_id: CapabilityId::from("onboard-led"),
                },
            ),
        ]),
    };
    let connection_providers = BTreeMap::from([
        (
            (OperationId::from("pulse"), OperationId::from("local")),
            ConnectionProvider::Local,
        ),
        (
            (OperationId::from("pulse"), OperationId::from("web")),
            ConnectionProvider::FixtureFrame,
        ),
        (
            (OperationId::from("pulse"), OperationId::from("light")),
            ConnectionProvider::FixtureDatagram,
        ),
    ]);
    let link_bindings = [
        process_owned_link_binding(
            "link/std-browser",
            ConnectionProvider::FixtureFrame,
            "fixture/frame/std-browser",
            &advertisements[0],
            &advertisements[1],
            4,
            64,
        ),
        process_owned_link_binding(
            "link/std-pico",
            ConnectionProvider::FixtureDatagram,
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
            ConnectionProvider::Local,
            ConnectionProvider::FixtureFrame,
            ConnectionProvider::FixtureDatagram,
        ],
        PlanningOptions {
            connection_providers: &connection_providers,
            connection_item_capacity: 4,
            connection_byte_capacity: 64,
            authority_grants: &[],
            link_bindings: &link_bindings,
        },
    )
    .map_err(|err| err.to_string())?;

    let mut observations = inspect(&mut std_host);
    observations.extend(inspect(&mut pico));
    let snapshot = ObservatorySnapshot {
        schema: SNAPSHOT_SCHEMA.into(),
        hosts: advertisements
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
            .collect(),
        links: link_bindings
            .into_iter()
            .map(|binding| LinkReport {
                binding,
                state: OperationalState::Available,
            })
            .collect(),
        plans: vec![plan],
        plays: vec![],
        retention: RetentionReport {
            item_capacity: 64,
            retained_items: observations.len() as u32,
            dropped_items: 0,
        },
        observations,
    };
    let report = build_report(&snapshot)?;
    Ok(format!(
        "SIMULATION ONLY: synthetic observatory fixture; not connected-host evidence\n{}",
        render_text_report(&report)
    ))
}

#[test]
fn observatory_fixture_report_is_explicitly_synthetic_and_does_not_run_work() {
    let stdout = observatory_fixture_report().expect("observatory fixture report succeeds");
    assert!(
        stdout.contains("SIMULATION ONLY: synthetic observatory fixture"),
        "{stdout}"
    );
    assert!(stdout.contains("host observatory report"), "{stdout}");
    assert!(stdout.contains("hosts 3"), "{stdout}");
    assert!(
        stdout.contains("host id=std-host-triple boot=std-boot-triple"),
        "{stdout}"
    );
    assert!(
        stdout.contains("host id=browser-sim-triple boot=browser-sim-boot-triple"),
        "{stdout}"
    );
    assert!(
        stdout.contains("host id=pico-sim-triple boot=pico-sim-boot-triple"),
        "{stdout}"
    );
    assert!(stdout.contains("capabilities 8"), "{stdout}");
    assert!(
        stdout.contains("capability=time-tick-v2 kind=time/tick contract=conduit.std/time-tick@2"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=presentation-text-v1 kind=presentation/text contract=conduit.std/presentation-text@1"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("links 2"), "{stdout}");
    assert!(stdout.contains("plans 1"), "{stdout}");
    assert!(stdout.contains("placements 4"), "{stdout}");
    assert!(stdout.contains("connections 3"), "{stdout}");
    assert!(stdout.contains("provider=FixtureFrame"), "{stdout}");
    assert!(stdout.contains("provider=FixtureDatagram"), "{stdout}");
    assert!(
        stdout.contains("evidence id=") && stdout.contains("active_play=none presentation=none"),
        "{stdout}"
    );
    assert!(!stdout.contains("evidence id=evidence/"), "{stdout}");
    assert!(stdout.contains("retention bounded=true"), "{stdout}");
    assert!(
        !stdout.contains("receipt signal placement="),
        "observatory report must not activate work: {stdout}"
    );
}
