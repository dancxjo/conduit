#[cfg(feature = "sim-fixtures")]
use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
#[cfg(feature = "sim-fixtures")]
use conduit_core::{
    process_owned_link_binding, BootId, CapabilityId, ConnectionProvider, HostCommand, HostEvent,
    HostId, OfferGeneration, OperationId,
};
#[cfg(feature = "sim-fixtures")]
use conduit_form::parse;
#[cfg(feature = "sim-fixtures")]
use conduit_observatory::{build_report, render_text_report};
#[cfg(feature = "sim-fixtures")]
use conduit_pico_sim::{pico_advertisement, PicoSim, PicoSimConfig};
#[cfg(feature = "sim-fixtures")]
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
#[cfg(feature = "sim-fixtures")]
use conduit_realm::{AdmissionRequest, LinkId, Realm, RealmId};
#[cfg(feature = "sim-fixtures")]
use conduit_signal::signal_profile_catalog;
#[cfg(feature = "sim-fixtures")]
use conduit_std_host::StdHostConfig;
use conduit_std_host::{
    load_checked_form, load_placements, run_kernel_multivalue_path_to, StdHost, ThreadTimer,
};
#[cfg(feature = "sim-fixtures")]
use std::collections::BTreeMap;
use std::env;
use std::io;

fn run_with_placements(path: &str, placements_path: Option<&str>) -> Result<(), String> {
    let form = load_checked_form(path).map_err(|err| err.to_string())?;
    let placements = load_placements(placements_path).map_err(|err| err.to_string())?;
    let mut host = StdHost::new();
    let plan = host
        .plan_local(&form, placements.as_ref())
        .map_err(|err| err.to_string())?;
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == host.advertisement().host_id)
        .ok_or_else(|| "no local fragment for std host".to_string())?;
    let mut stdout = io::stdout().lock();
    host.run_fragment_to(fragment, &mut stdout, &mut ThreadTimer)?;
    Ok(())
}

#[cfg(feature = "sim-fixtures")]
fn observatory_fixture_report() -> Result<String, String> {
    let mut std_host = StdHost::new_with_config(StdHostConfig {
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

    let mut realm = Realm::found(
        RealmId::from("realm-observatory"),
        std_host.advertisement().clone(),
        LinkId::from("link-std"),
        16,
    );
    realm
        .admit(AdmissionRequest {
            advertisement: browser_advertisement,
            link_id: LinkId::from("link-browser"),
            allow: true,
        })
        .map_err(|reason| format!("browser admission failed: {reason:?}"))?;
    realm
        .admit(AdmissionRequest {
            advertisement: pico_advertisement,
            link_id: LinkId::from("link-pico"),
            allow: true,
        })
        .map_err(|reason| format!("pico admission failed: {reason:?}"))?;
    let realm_view = realm
        .view_for(&HostId::from("std-host-triple"))
        .ok_or_else(|| "realm view missing".to_string())?;

    let form = parse(
        include_str!("../../../examples/triple-signal.form"),
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
    let report = build_report(&advertisements, Some(&realm_view), &[plan], &observations);
    Ok(format!(
        "SIMULATION ONLY: synthetic observatory fixture; not connected-host evidence\n{}",
        render_text_report(&report)
    ))
}

#[cfg(feature = "sim-fixtures")]
fn inspect(host: &mut impl HandleInspect) -> Vec<conduit_core::Observation> {
    host.inspect()
}

#[cfg(feature = "sim-fixtures")]
trait HandleInspect {
    fn inspect(&mut self) -> Vec<conduit_core::Observation>;
}

#[cfg(feature = "sim-fixtures")]
impl HandleInspect for StdHost {
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

#[cfg(feature = "sim-fixtures")]
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

fn main() {
    let mut args = env::args();
    let _program = args.next();
    let path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("usage: conduit <form-file> [--placements <placements-file>]");
            std::process::exit(2);
        }
    };
    #[cfg(feature = "sim-fixtures")]
    if path == "observatory-fixture-report" {
        if args.next().is_some() {
            eprintln!("usage: conduit observatory-fixture-report");
            std::process::exit(2);
        }
        match observatory_fixture_report() {
            Ok(report) => {
                print!("{report}");
                return;
            }
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
    }
    if path == "kernel-multivalue" {
        let Some(form_path) = args.next() else {
            eprintln!("usage: conduit kernel-multivalue <form-file>");
            std::process::exit(2);
        };
        if args.next().is_some() {
            eprintln!("usage: conduit kernel-multivalue <form-file>");
            std::process::exit(2);
        }
        let mut stdout = io::stdout().lock();
        if let Err(err) = run_kernel_multivalue_path_to(&form_path, &mut stdout, &mut ThreadTimer) {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
        return;
    }

    let placements_path = match (args.next().as_deref(), args.next()) {
        (Some("--placements"), value) => value,
        (Some(other), _) => {
            eprintln!(
                "usage: conduit <form-file> [--placements <placements-file>]\nunexpected argument: {other}"
            );
            std::process::exit(2);
        }
        (None, _) => None,
    };

    if let Err(err) = run_with_placements(&path, placements_path.as_deref()) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
