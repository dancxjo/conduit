mod cli;
mod copy_task;
mod diagnostics;
mod form_source;
mod report_artifact;

use clap::Parser;
use conduit_observatory::{build_report, render_text_report};
use conduit_std_host::{load_placements, StdHost, ThreadTimer};
use std::io;
use std::path::Path;

fn enter_patchbay(host: cli::PatchbayHost) -> Result<(), String> {
    let executable = match host {
        cli::PatchbayHost::Native => "patchbay-native",
        cli::PatchbayHost::Browser => "patchbay-html",
    };
    let mut command = std::process::Command::new(executable);
    if host == cli::PatchbayHost::Native {
        command.arg("--front-door");
    }
    let status = command.status().map_err(|error| {
        format!(
            "{executable} is unavailable ({error}); install the selected Patchbay renderer or use `cargo xtask demo patchbay --on {}` from a Conduit checkout",
            match host {
                cli::PatchbayHost::Native => "native",
                cli::PatchbayHost::Browser => "browser",
            }
        )
    })?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{executable} exited with {status}"))
}

use crate::report_artifact::{read_report, snapshot_from_execution, write_report};

fn run_with_placements(
    path: &str,
    placements_path: Option<&str>,
    report_path: Option<&Path>,
) -> Result<(), String> {
    let source = form_source::load(Path::new(path))?;
    let form = source.expand_entry()?;
    let placements = load_placements(placements_path).map_err(|err| err.to_string())?;
    let mut host = StdHost::new();
    let hosts = vec![host.advertisement().clone()];
    let placements = match placements {
        Some(placements) => placements,
        None => conduit_planner::default_expanded_placements(&form, &hosts)
            .map_err(|error| error.to_string())?,
    };
    let plan = conduit_planner::plan_expanded_canonical(
        &form,
        &hosts,
        &placements,
        &[conduit_core::ConnectionBase::Local],
    )
    .map_err(|error| error.to_string())?;
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
    let command = cli::Cli::parse().command;
    let result = match command {
        cli::Command::Patchbay { on } => enter_patchbay(on),
        cli::Command::Run {
            form,
            placements,
            report,
        } => run_with_placements(
            &form.to_string_lossy(),
            placements.as_deref().map(Path::to_string_lossy).as_deref(),
            report.as_deref(),
        ),
        cli::Command::Check { form, json } => match diagnostics::run(&form, json) {
            Ok(true) => Ok(()),
            Ok(false) => std::process::exit(1),
            Err(error) => Err(error),
        },
        cli::Command::Inspect {
            command: cli::InspectCommand::RuntimeReport { report },
        } => render_runtime_report(&report).map(|rendered| {
            print!("{rendered}");
        }),
        cli::Command::Copy { arguments } => copy_task::run(arguments),
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
