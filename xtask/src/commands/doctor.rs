use std::path::Path;

use serde::Serialize;

use crate::{
    cli::{DoctorArgs, DoctorTarget, GlobalOpts},
    process::{run_probe, ProbeOutcome, Step, StepError},
    workspace::workspace_root,
};

const REPORT_SCHEMA: &str = "conduit.xtask-doctor-report/v1";

#[derive(Debug, Serialize)]
struct DoctorReport {
    schema: &'static str,
    command: &'static str,
    target: &'static str,
    dry_run: bool,
    exact_git_commit: Option<String>,
    dirty: Option<bool>,
    probes: Vec<DoctorProbe>,
}

#[derive(Debug, Serialize)]
struct DoctorProbe {
    section: &'static str,
    repair: Option<&'static str>,
    #[serde(flatten)]
    outcome: ProbeOutcome,
}

struct ProbeSpec {
    section: &'static str,
    step: Step,
    repair: Option<&'static str>,
}

pub fn run(args: DoctorArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root().map_err(|error| StepError::prereq("workspace-root", error))?;
    let report = build_report(args.target, opts, &root);

    if opts.json {
        let encoded = serde_json::to_string_pretty(&report)
            .map_err(|error| StepError::prereq("doctor-json", error.to_string()))?;
        println!("{encoded}");
    } else if !opts.quiet {
        print_human_report(&report);
    }

    let failed = report
        .probes
        .iter()
        .filter(|probe| !probe.outcome.skipped && !probe.outcome.success)
        .map(|probe| probe.outcome.id.as_str())
        .collect::<Vec<_>>();

    if failed.is_empty() {
        Ok(())
    } else {
        Err(StepError::prereq(
            "doctor",
            format!("failed prerequisite probes: {}", failed.join(", ")),
        ))
    }
}

fn build_report(target: DoctorTarget, opts: &GlobalOpts, root: &Path) -> DoctorReport {
    let probes = probe_specs(target)
        .into_iter()
        .map(|spec| {
            let mut outcome = run_probe(&spec.step, root, opts);
            if outcome.id == "doctor.pico.thumb-target"
                && !outcome.skipped
                && outcome.success
                && !outcome
                    .stdout
                    .lines()
                    .any(|line| line.trim() == "thumbv6m-none-eabi")
            {
                outcome.success = false;
                outcome.stderr = "required Rust target is not installed".to_string();
            }
            DoctorProbe {
                section: spec.section,
                repair: spec.repair,
                outcome,
            }
        })
        .collect::<Vec<_>>();

    let exact_git_commit = probe_stdout(&probes, "doctor.git.commit");
    let dirty = probes
        .iter()
        .find(|probe| probe.outcome.id == "doctor.git.status")
        .and_then(|probe| {
            (!probe.outcome.skipped && probe.outcome.success)
                .then_some(!probe.outcome.stdout.is_empty())
        });

    DoctorReport {
        schema: REPORT_SCHEMA,
        command: "doctor",
        target: target.as_str(),
        dry_run: opts.dry_run,
        exact_git_commit,
        dirty,
        probes,
    }
}

fn probe_stdout(probes: &[DoctorProbe], id: &str) -> Option<String> {
    probes
        .iter()
        .find(|probe| probe.outcome.id == id)
        .filter(|probe| {
            !probe.outcome.skipped && probe.outcome.success && !probe.outcome.stdout.is_empty()
        })
        .map(|probe| probe.outcome.stdout.clone())
}

fn probe_specs(target: DoctorTarget) -> Vec<ProbeSpec> {
    let mut probes = Vec::new();

    if matches!(target, DoctorTarget::All) {
        probes.extend(general_probes());
    }
    if matches!(target, DoctorTarget::All | DoctorTarget::Browser) {
        probes.extend(browser_probes());
    }
    if matches!(target, DoctorTarget::All | DoctorTarget::Pico) {
        probes.extend(pico_probes());
    }

    probes
}

fn general_probes() -> Vec<ProbeSpec> {
    vec![
        probe(
            "general",
            "doctor.rustc",
            "Rust compiler",
            "rustc",
            &["--version"],
            None,
        ),
        probe(
            "general",
            "doctor.cargo",
            "Cargo",
            "cargo",
            &["--version"],
            None,
        ),
        probe(
            "general",
            "doctor.rustup.targets",
            "installed Rust targets",
            "rustup",
            &["target", "list", "--installed"],
            Some("rustup target add wasm32-unknown-unknown thumbv6m-none-eabi"),
        ),
        probe(
            "general",
            "doctor.node",
            "Node.js",
            "node",
            &["--version"],
            None,
        ),
        probe("general", "doctor.npm", "npm", "npm", &["--version"], None),
        probe("general", "doctor.npx", "npx", "npx", &["--version"], None),
        probe(
            "git",
            "doctor.git.commit",
            "exact Git commit",
            "git",
            &["rev-parse", "HEAD"],
            None,
        ),
        probe(
            "git",
            "doctor.git.status",
            "Git dirty state",
            "git",
            &["status", "--porcelain"],
            None,
        ),
    ]
}

fn browser_probes() -> Vec<ProbeSpec> {
    vec![
        probe(
            "browser",
            "doctor.browser.playwright",
            "Playwright CLI",
            "npx",
            &["playwright", "--version"],
            Some("npm ci --ignore-scripts"),
        ),
        probe(
            "browser",
            "doctor.browser.chromium",
            "pinned Chromium availability",
            "npx",
            &["playwright", "install", "--dry-run", "chromium"],
            Some("npx playwright install chromium"),
        ),
        probe(
            "browser",
            "doctor.browser.firefox",
            "pinned Firefox availability",
            "npx",
            &["playwright", "install", "--dry-run", "firefox"],
            Some("npx playwright install firefox"),
        ),
    ]
}

fn pico_probes() -> Vec<ProbeSpec> {
    vec![
        probe(
            "pico",
            "doctor.pico.thumb-target",
            "thumbv6m-none-eabi target",
            "rustup",
            &["target", "list", "--installed"],
            Some("rustup target add thumbv6m-none-eabi"),
        ),
        probe(
            "pico",
            "doctor.pico.elf2uf2",
            "ELF to UF2 converter",
            "elf2uf2-rs",
            &["--help"],
            Some("cargo install elf2uf2-rs --locked"),
        ),
    ]
}

fn probe(
    section: &'static str,
    id: &'static str,
    description: &'static str,
    program: &'static str,
    args: &'static [&'static str],
    repair: Option<&'static str>,
) -> ProbeSpec {
    ProbeSpec {
        section,
        step: Step::new(id, description, program, args),
        repair,
    }
}

fn print_human_report(report: &DoctorReport) {
    println!("Conduit doctor ({})", report.target);
    if report.dry_run {
        println!("dry run: no external probes were executed");
    }

    let mut previous_section = None;
    for probe in &report.probes {
        if previous_section != Some(probe.section) {
            println!("\n{}:", probe.section);
            previous_section = Some(probe.section);
        }

        let status = if probe.outcome.skipped {
            "planned"
        } else if probe.outcome.success {
            "ok"
        } else {
            "missing/failing"
        };
        println!("  {:<30} {status}", probe.outcome.description);
        if !probe.outcome.stdout.is_empty() {
            for line in probe.outcome.stdout.lines() {
                println!("    {line}");
            }
        }
        if let Some(error) = &probe.outcome.launch_error {
            println!("    {error}");
        } else if !probe.outcome.stderr.is_empty() {
            for line in probe.outcome.stderr.lines() {
                println!("    {line}");
            }
        }
        if let (false, Some(repair)) = (probe.outcome.success, probe.repair) {
            println!("    repair: {repair}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dry_json_opts() -> GlobalOpts {
        GlobalOpts {
            dry_run: true,
            quiet: true,
            json: true,
            ..Default::default()
        }
    }

    #[test]
    fn dry_run_report_executes_no_probes() {
        let root = workspace_root().expect("workspace root");
        let report = build_report(DoctorTarget::All, &dry_json_opts(), &root);
        assert!(report.dry_run);
        assert!(!report.probes.is_empty());
        assert!(report.probes.iter().all(|probe| probe.outcome.skipped));
        assert!(report.exact_git_commit.is_none());
        assert!(report.dirty.is_none());
    }

    #[test]
    fn json_report_has_stable_schema_and_target() {
        let root = workspace_root().expect("workspace root");
        let report = build_report(DoctorTarget::Browser, &dry_json_opts(), &root);
        let value = serde_json::to_value(report).expect("serialize doctor report");
        assert_eq!(value["schema"], REPORT_SCHEMA);
        assert_eq!(value["command"], "doctor");
        assert_eq!(value["target"], "browser");
        assert_eq!(value["dry_run"], true);
        assert!(value["probes"]
            .as_array()
            .is_some_and(|probes| !probes.is_empty()));
    }
}
