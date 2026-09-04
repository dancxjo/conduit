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
use std::ffi::OsString;
use std::io;
use std::path::Path;

fn patchbay_process(
    host: cli::PatchbayHost,
    body_evidence: Option<&Path>,
    reviewed_forms: &[OsString],
) -> Result<std::process::Command, String> {
    let executable = match host {
        cli::PatchbayHost::Native => "patchbay-native",
        cli::PatchbayHost::Browser => "patchbay-html",
    };
    if host == cli::PatchbayHost::Native && (body_evidence.is_some() || !reviewed_forms.is_empty())
    {
        return Err(
            "opening exported Body evidence or reviewed Forms currently requires `conduit patchbay --on browser`"
                .into(),
        );
    }
    if body_evidence.is_none() && !reviewed_forms.is_empty() {
        return Err("reviewed Forms require exact exported Body evidence".into());
    }
    let mut command = std::process::Command::new(executable);
    if host == cli::PatchbayHost::Native {
        command.arg("--front-door");
    }
    if let Some(path) = body_evidence {
        command
            .arg("--body-evidence")
            .arg(path)
            .arg("--external-reader");
    }
    let mut pairs = reviewed_forms.chunks_exact(2);
    for pair in &mut pairs {
        command.arg("--form").arg(&pair[0]).arg(&pair[1]);
    }
    if !pairs.remainder().is_empty() {
        return Err("each reviewed Form requires an exact LABEL and PATH".into());
    }
    Ok(command)
}

fn enter_patchbay(
    host: cli::PatchbayHost,
    body_evidence: Option<&Path>,
    reviewed_forms: &[OsString],
) -> Result<(), String> {
    let mut command = patchbay_process(host, body_evidence, reviewed_forms)?;
    let executable = match host {
        cli::PatchbayHost::Native => "patchbay-native",
        cli::PatchbayHost::Browser => "patchbay-html",
    };
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

fn enter_creche() -> Result<(), String> {
    let executable = "conduit-browser-host";
    let conduit = std::env::current_exe()
        .map_err(|error| format!("cannot locate the installed Conduit entrance ({error})"))?;
    let application = conduit
        .parent()
        .ok_or("the installed Conduit entrance has no parent directory")?
        .join("conduit-creche");
    if !application.join("index.html").is_file() {
        return Err(format!(
            "the admitted Crèche application is unavailable at {}; install it alongside the Conduit executables",
            application.display()
        ));
    }
    let status = std::process::Command::new(executable)
        .arg("--application")
        .arg(application)
        .args(["--mount", "/creche/"])
        .status()
        .map_err(|error| {
            format!(
                "{executable} is unavailable ({error}); install the Conduit browser Host alongside the `conduit` product entrance"
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
        cli::Command::Creche => enter_creche(),
        cli::Command::Patchbay {
            on,
            body_evidence,
            reviewed_form,
        } => enter_patchbay(on, body_evidence.as_deref(), &reviewed_form),
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

#[cfg(test)]
mod patchbay_entrance_tests {
    use super::*;

    #[test]
    fn exported_body_evidence_enters_the_browser_external_reader_exactly() {
        let path = Path::new("roseau-body.json");
        let command = patchbay_process(cli::PatchbayHost::Browser, Some(path), &[]).unwrap();
        assert_eq!(command.get_program(), "patchbay-html");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--body-evidence", "roseau-body.json", "--external-reader"].map(std::ffi::OsStr::new)
        );
    }

    #[test]
    fn native_front_door_stays_distinct_from_unsupported_evidence_open() {
        let command = patchbay_process(cli::PatchbayHost::Native, None, &[]).unwrap();
        assert_eq!(command.get_program(), "patchbay-native");
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["--front-door"]);
        assert!(patchbay_process(
            cli::PatchbayHost::Native,
            Some(Path::new("roseau-body.json")),
            &[],
        )
        .unwrap_err()
        .contains("--on browser"));
    }

    #[test]
    fn reviewed_forms_are_forwarded_as_exact_bounded_pairs() {
        let forms = [
            OsString::from("Greet"),
            OsString::from("forms/greet/greet.conduit"),
            OsString::from("Count"),
            OsString::from("forms/count/count.conduit"),
        ];
        let command = patchbay_process(
            cli::PatchbayHost::Browser,
            Some(Path::new("roseau-body.json")),
            &forms,
        )
        .unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "--body-evidence",
                "roseau-body.json",
                "--external-reader",
                "--form",
                "Greet",
                "forms/greet/greet.conduit",
                "--form",
                "Count",
                "forms/count/count.conduit",
            ]
            .map(std::ffi::OsStr::new)
        );
        assert!(patchbay_process(
            cli::PatchbayHost::Browser,
            Some(Path::new("roseau-body.json")),
            &forms[..1],
        )
        .unwrap_err()
        .contains("LABEL and PATH"));
        assert!(patchbay_process(cli::PatchbayHost::Browser, None, &forms)
            .unwrap_err()
            .contains("Body evidence"));
    }
}
