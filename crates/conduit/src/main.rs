mod body_product;
mod cli;
mod construction;
mod copy_task;
#[cfg(test)]
mod copy_task_tests;
mod diagnostics;
mod form_source;
mod product_execution;
#[cfg(test)]
mod product_execution_tests;
mod protected_task;
mod report_artifact;
mod std_websocket_line;
#[cfg(test)]
mod two_std_line_tests;

use clap::Parser;
use conduit_observatory::{build_report, render_text_report};
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
    body_path: Option<&Path>,
) -> Result<(), String> {
    let source = form_source::load(Path::new(path))?;
    let form = source.expand_entry()?;
    let body_product = body_path.map(body_product::prepare).transpose()?;
    let mut context = match body_product {
        Some(product) => product.context,
        None => product_execution::ProductExecutionContext::local_std()?,
    };
    let plan = context.plan(&form, placements_path)?;
    let mut stdout = io::stdout().lock();
    let execution = context.execute(plan, &mut stdout)?;
    if let Some(report_path) = report_path {
        let snapshot = snapshot_from_execution(
            execution.advertisements,
            execution.line_offers,
            vec![execution.plan],
            execution.observations,
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
            body,
        } => run_with_placements(
            &form.to_string_lossy(),
            placements.as_deref().map(Path::to_string_lossy).as_deref(),
            report.as_deref(),
            body.as_deref(),
        ),
        cli::Command::Host { command } => construction::host(command),
        cli::Command::Body { command } => construction::body(command),
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
