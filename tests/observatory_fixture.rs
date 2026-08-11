use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
use conduit_core::{
    process_owned_line_offer, BootId, CapabilityId, ConnectionBase, GearId, HostCommand, HostEvent,
    HostId, OfferGeneration,
};
use conduit_form::parse;
use conduit_observatory::{
    build_report, render_text_report, CapabilityAvailability, CapabilityStatusReport,
    CapabilitySupport, HostReport, LineReport, ObservatorySnapshot, OfferFreshness,
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
    let line_offers = [
        process_owned_line_offer(
            "line/std-browser",
            "link/std-browser",
            ConnectionBase::FixtureFrame,
            "fixture/frame/std-browser",
            &advertisements[0],
            &advertisements[1],
            4,
            64,
        ),
        process_owned_line_offer(
            "line/std-pico",
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
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &line_offers,
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
        bases: Vec::new(),
        lines: line_offers
            .into_iter()
            .map(|offer| LineReport {
                offer,
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
        historical_observations: Vec::new(),
        sealed_boot_provenance: Vec::new(),
    };
    let report = build_report(&snapshot)?;
    Ok(format!(
        "SIMULATION ONLY: synthetic observatory fixture; not connected-host sign\n{}",
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
    assert!(stdout.contains("capabilities 34"), "{stdout}");
    assert!(
        stdout.contains("capability=time-tick-v2 kind=time/tick contract=conduit.std/time-tick@2"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=time-debounce-bool-v1 kind=time/debounce contract=conduit.std/time-debounce-bool@1"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=time-timeout-tick-bool-v1 kind=time/timeout contract=conduit.std/time-timeout-tick-bool@1"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=text-literal-v1 kind=text/literal contract=conduit.std/text-literal@1"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("capability=file-copy-v1 kind=file/copy contract=conduit.std/file-copy@1"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=logic-compare-scalar-v1 kind=logic/compare contract=conduit.std/logic-compare-scalar@1"
        ),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("capability=text-upper-v1 kind=text/upper contract=conduit.std/text-upper@1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("capability=text-join-v1 kind=text/join contract=conduit.std/text-join@1"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=state-latest-scalar-v2 kind=state/latest contract=conduit.std/state-latest-scalar@2"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=flow-tee-scalar-v2 kind=flow/tee contract=conduit.std/flow-tee-scalar@2"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=flow-gate-scalar-v1 kind=flow/gate contract=conduit.std/flow-gate-scalar@1"
        ),
        "{stdout}"
    );
    assert!(
        stdout
            .contains("capability=time-every-v1 kind=time/every contract=conduit.std/time-every@1"),
        "{stdout}"
    );
    assert!(
        stdout.contains("capability=presentation-tick-v1 kind=presentation/tick contract=conduit.std/presentation-tick@1"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=presentation-text-v1 kind=presentation/text contract=conduit.std/presentation-text@1"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=state-count-v1 kind=state/count contract=conduit.std/state-count@1"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "capability=presentation-count-v1 kind=presentation/count contract=conduit.std/presentation-count@1"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("lines 2"), "{stdout}");
    assert!(stdout.contains("plans 1"), "{stdout}");
    assert!(stdout.contains("placements 4"), "{stdout}");
    assert!(stdout.contains("connections 3"), "{stdout}");
    assert!(stdout.contains("base=FixtureFrame"), "{stdout}");
    assert!(stdout.contains("base=FixtureDatagram"), "{stdout}");
    assert!(
        stdout.contains("sign id=") && stdout.contains("active_play=none presentation=none"),
        "{stdout}"
    );
    assert!(!stdout.contains("sign id=sign/"), "{stdout}");
    assert!(stdout.contains("retention bounded=true"), "{stdout}");
    assert!(
        !stdout.contains("receipt signal placement="),
        "observatory report must not trigger work: {stdout}"
    );
}
