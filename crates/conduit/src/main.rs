use conduit_browser_host::{BrowserHostConfig, BrowserPage};
use conduit_core::{
    BootId, CapabilityId, ConnectionProvider, HostCommand, HostEvent, HostId, OfferGeneration,
    OperationId,
};
use conduit_form::parse;
use conduit_observatory::{build_report, render_text_report};
use conduit_pico_host::{pico_advertisement, PicoHost, PicoHostConfig};
use conduit_planner::{
    plan_with_connection_limits_and_provider_overrides, PlacementChoice, PlacementChoices,
};
use conduit_realm::{AdmissionRequest, LinkId, Realm, RealmId};
use conduit_signal::signal_profile_catalog;
use conduit_std_host::{
    load_checked_form, load_placements, render_copy_task_inspect, render_copy_task_report,
    run_copy_file_task, CopyFileRequest, CopyReplaceMode, CopyTaskResult, StdHost, StdHostConfig,
    ThreadTimer,
};
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

fn observatory_report() -> Result<String, String> {
    let mut std_host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("std-host-triple"),
        boot_id: BootId::from("std-boot-triple"),
        offer_generation: OfferGeneration(1),
    });
    let page = BrowserPage::with_hosts([BrowserHostConfig {
        host_id: HostId::from("browser-host-triple"),
        boot_id: BootId::from("browser-boot-triple"),
        offer_generation: OfferGeneration(1),
    }]);
    let mut pico = PicoHost::new(PicoHostConfig {
        host_id: HostId::from("pico-host-triple"),
        boot_id: BootId::from("pico-boot-triple"),
        offer_generation: OfferGeneration(1),
    });
    let browser_advertisement = page
        .advertisements()
        .into_iter()
        .next()
        .ok_or_else(|| "browser advertisement missing".to_string())?;
    let pico_advertisement = pico_advertisement(PicoHostConfig {
        host_id: HostId::from("pico-host-triple"),
        boot_id: BootId::from("pico-boot-triple"),
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
                    host_id: HostId::from("browser-host-triple"),
                    capability_id: CapabilityId::from("dom-show"),
                },
            ),
            (
                OperationId::from("light"),
                PlacementChoice {
                    host_id: HostId::from("pico-host-triple"),
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
            ConnectionProvider::WebSocket,
        ),
        (
            (OperationId::from("pulse"), OperationId::from("light")),
            ConnectionProvider::Udp,
        ),
    ]);
    let plan = plan_with_connection_limits_and_provider_overrides(
        &form,
        &advertisements,
        &placements,
        &[
            ConnectionProvider::Local,
            ConnectionProvider::WebSocket,
            ConnectionProvider::Udp,
        ],
        &connection_providers,
        4,
        64,
    )
    .map_err(|err| err.to_string())?;

    let mut observations = inspect(&mut std_host);
    observations.extend(inspect(&mut pico));
    let report = build_report(&advertisements, Some(&realm_view), &[plan], &observations);
    Ok(render_text_report(&report))
}

fn inspect(host: &mut impl HandleInspect) -> Vec<conduit_core::Observation> {
    host.inspect()
}

trait HandleInspect {
    fn inspect(&mut self) -> Vec<conduit_core::Observation>;
}

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

impl HandleInspect for PicoHost {
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
            eprintln!("usage: conduit <form-file> [--placements <placements-file>]\n       conduit observatory-report\n       conduit copy-file --source <path> --destination <path> [--replace|--reject-existing] [--max-bytes <n>] [--inspect]");
            std::process::exit(2);
        }
    };
    if path == "observatory-report" {
        if args.next().is_some() {
            eprintln!("usage: conduit observatory-report");
            std::process::exit(2);
        }
        match observatory_report() {
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
    if path == "copy-file" {
        match copy_file_from_args(args.collect()) {
            Ok(success) => std::process::exit(if success { 0 } else { 1 }),
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(2);
            }
        }
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

fn copy_file_from_args(args: Vec<String>) -> Result<bool, String> {
    let mut source = None;
    let mut destination = None;
    let mut replace_mode = CopyReplaceMode::RejectExisting;
    let mut max_bytes = 16 * 1024 * 1024;
    let mut inspect = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--source" => {
                index += 1;
                source = args.get(index).cloned();
            }
            "--destination" => {
                index += 1;
                destination = args.get(index).cloned();
            }
            "--replace" => {
                replace_mode = CopyReplaceMode::ReplaceExisting;
            }
            "--reject-existing" | "--create-only" => {
                replace_mode = CopyReplaceMode::RejectExisting;
            }
            "--max-bytes" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--max-bytes requires a value".to_string())?;
                max_bytes = value
                    .parse::<u64>()
                    .map_err(|_| "--max-bytes must be an unsigned integer".to_string())?;
            }
            "--inspect" => {
                inspect = true;
            }
            other => return Err(format!("unexpected copy-file argument: {other}")),
        }
        index += 1;
    }
    let source = source.ok_or_else(|| "copy-file requires --source <path>".to_string())?;
    let destination =
        destination.ok_or_else(|| "copy-file requires --destination <path>".to_string())?;
    let request = CopyFileRequest::new(source, destination, replace_mode, max_bytes, inspect);
    let report = run_copy_file_task(request);
    let mut stdout = io::stdout().lock();
    render_copy_task_report(&report, &mut stdout)?;
    if report.inspect_requested {
        render_copy_task_inspect(&report, &mut stdout)?;
    }
    Ok(matches!(
        report.result,
        CopyTaskResult::Created { .. } | CopyTaskResult::Replaced { .. }
    ))
}
