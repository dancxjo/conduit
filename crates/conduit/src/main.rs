mod report_artifact;

use conduit_observatory::{build_report, render_text_report};
use conduit_std_host::{
    load_checked_form, load_placements, run_kernel_multivalue_path_to, StdHost, ThreadTimer,
};
use std::env;
use std::io;
use std::path::{Path, PathBuf};

use crate::report_artifact::{read_report, snapshot_from_execution, write_report};

fn run_with_placements(
    path: &str,
    placements_path: Option<&str>,
    report_path: Option<&Path>,
) -> Result<(), String> {
    let form = load_checked_form(path).map_err(|err| err.to_string())?;
    let placements = load_placements(placements_path).map_err(|err| err.to_string())?;
    let mut host = StdHost::new();
    let plan = host
        .plan_local(&form, placements.as_ref())
        .map_err(|err| err.to_string())?;
    let fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == host.advertisement().host_id)
        .cloned()
        .ok_or_else(|| "no local fragment for std host".to_string())?;
    let mut stdout = io::stdout().lock();
    let report = host.run_fragment_to(fragment, &mut stdout, &mut ThreadTimer)?;
    if let Some(report_path) = report_path {
        let snapshot = snapshot_from_execution(
            vec![host.advertisement().clone()],
            vec![plan],
            report.observations,
        );
        write_report(report_path, &snapshot)?;
    }
    Ok(())
}

fn render_runtime_report(path: &Path) -> Result<String, String> {
    let snapshot = read_report(path)?;
    let report = build_report(&snapshot)?;
    Ok(render_text_report(&report))
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
    if path == "observatory-report" {
        let Some(report_path) = args.next() else {
            eprintln!("usage: conduit observatory-report <runtime-report.json>");
            std::process::exit(2);
        };
        if args.next().is_some() {
            eprintln!("usage: conduit observatory-report <runtime-report.json>");
            std::process::exit(2);
        }
        match render_runtime_report(Path::new(&report_path)) {
            Ok(rendered) => print!("{rendered}"),
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        return;
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

    let mut placements_path = None;
    let mut report_path = None;
    while let Some(option) = args.next() {
        let value = args.next().unwrap_or_else(|| {
            eprintln!("missing value for {option}");
            std::process::exit(2);
        });
        match option.as_str() {
            "--placements" if placements_path.is_none() => placements_path = Some(value),
            "--report" if report_path.is_none() => report_path = Some(PathBuf::from(value)),
            _ => {
                eprintln!("usage: conduit <form-file> [--placements <placements-file>] [--report <runtime-report.json>]\nunexpected or duplicate argument: {option}");
                std::process::exit(2);
            }
        }
    }

    if let Err(err) = run_with_placements(&path, placements_path.as_deref(), report_path.as_deref())
    {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
