mod cli;
mod copy_task;
mod diagnostics;
mod report_artifact;

use clap::Parser;
use conduit_observatory::{build_report, render_text_report};
use conduit_std_host::{
    load_checked_form, load_placements, run_kernel_multivalue_path_to, StdHost, ThreadTimer,
};
use std::io;
use std::path::Path;

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
    let command = cli::Cli::parse().command;
    let result = match command {
        cli::Command::Run {
            form,
            placements,
            report,
        } => run_with_placements(
            &form.to_string_lossy(),
            placements.as_deref().map(Path::to_string_lossy).as_deref(),
            report.as_deref(),
        ),
        cli::Command::Check { form, json } | cli::Command::DiagnoseForm { form, json } => {
            match diagnostics::run(&form, json) {
                Ok(true) => Ok(()),
                Ok(false) => std::process::exit(1),
                Err(error) => Err(error),
            }
        }
        cli::Command::Inspect {
            command: cli::InspectCommand::RuntimeReport { report },
        }
        | cli::Command::ObservatoryReport { report } => {
            render_runtime_report(&report).map(|rendered| {
                print!("{rendered}");
            })
        }
        cli::Command::Copy { arguments } => copy_task::run(arguments),
        cli::Command::KernelMultivalue { form } => {
            let mut stdout = io::stdout().lock();
            run_kernel_multivalue_path_to(
                form.to_string_lossy().as_ref(),
                &mut stdout,
                &mut ThreadTimer,
            )
            .map(|_| ())
        }
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
