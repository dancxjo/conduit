//! Fast local integration over representative production paths.

use std::path::Path;

use serde::Serialize;

use crate::{
    cli::GlobalOpts,
    process::{run_probe, ProbeOutcome, Step, StepError},
    workspace::workspace_root,
};

use super::demo::{STD_STEP, TRIPLE_STEP};

const LANGUAGE_STEP: Step = Step::new(
    "integrate.language",
    "Parse, check, and lower representative Forms",
    "cargo",
    &["test", "-p", "conduit-form", "--test", "structured_values"],
);

const BODY_STEP: Step = Step::new(
    "integrate.body-lifecycle",
    "Exercise Body birth, wake, and retained biography transitions",
    "cargo",
    &[
        "test",
        "-p",
        "conduit-body",
        "--test",
        "lifecycle",
        "--test",
        "biography_wakes",
    ],
);

const RECOVERY_STEP: Step = Step::new(
    "integrate.failure-recovery",
    "Inject Line loss and recover through an ordinary replacement Plan",
    "cargo",
    &["xtask", "prove", "r1-new-plan-recovery"],
);

const PATCHBAY_STEP: Step = Step::new(
    "integrate.patchbay",
    "Project real Body planning and biography truth through Patchbay",
    "cargo",
    &[
        "test",
        "-p",
        "patchbay-model",
        "--test",
        "body_button_planning",
        "--test",
        "readable_body_history",
    ],
);

const BROWSER_STEP: Step = Step::new(
    "integrate.browser",
    "Build the production browser Host runtime for WASM",
    "cargo",
    &[
        "build",
        "-p",
        "conduit-browser-runtime",
        "--target",
        "wasm32-unknown-unknown",
    ],
);

const BROWSER_TARGET_PROBE: Step = Step::new(
    "integrate.browser-prerequisite",
    "Inspect the local Rust targets",
    "rustup",
    &["target", "list", "--installed"],
);

#[derive(Clone, Copy)]
struct IntegrationCheck {
    label: &'static str,
    step: &'static Step,
    reproduce: &'static str,
}

const CHECKS: &[IntegrationCheck] = &[
    IntegrationCheck {
        label: "language",
        step: &LANGUAGE_STEP,
        reproduce: "cargo test -p conduit-form --test structured_values",
    },
    IntegrationCheck {
        label: "planner/kernel, std Host, representative Form",
        step: &STD_STEP,
        reproduce: "cargo xtask demo std",
    },
    IntegrationCheck {
        label: "Body lifecycle",
        step: &BODY_STEP,
        reproduce: "cargo test -p conduit-body --test lifecycle --test biography_wakes",
    },
    IntegrationCheck {
        label: "local multi-placement",
        step: &TRIPLE_STEP,
        reproduce: "cargo xtask demo triple",
    },
    IntegrationCheck {
        label: "failure/recovery",
        step: &RECOVERY_STEP,
        reproduce: "cargo xtask prove r1-new-plan-recovery",
    },
    IntegrationCheck {
        label: "Patchbay",
        step: &PATCHBAY_STEP,
        reproduce:
            "cargo test -p patchbay-model --test body_button_planning --test readable_body_history",
    },
];

#[derive(Debug, Serialize)]
struct CheckReport {
    label: &'static str,
    status: &'static str,
    reproduce: &'static str,
    elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct IntegrationReport {
    schema: &'static str,
    passed: bool,
    checks: Vec<CheckReport>,
}

pub fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let mut reports = Vec::new();

    for check in CHECKS {
        run_check(check, &root, opts, &mut reports)?;
    }

    let prerequisite = run_probe(&BROWSER_TARGET_PROBE, &root, &silent(opts));
    if opts.dry_run || browser_target_available(&prerequisite) {
        run_check(
            &IntegrationCheck {
                label: "browser/WASM",
                step: &BROWSER_STEP,
                reproduce: "cargo build -p conduit-browser-runtime --target wasm32-unknown-unknown",
            },
            &root,
            opts,
            &mut reports,
        )?;
    } else {
        reports.push(CheckReport {
            label: "browser/WASM",
            status: "prerequisite-missing",
            reproduce: "rustup target add wasm32-unknown-unknown",
            elapsed_ms: prerequisite.elapsed_ms,
        });
        if !opts.quiet && !opts.json {
            println!("○ browser/WASM — prerequisite missing: wasm32-unknown-unknown");
            println!("  Install with: rustup target add wasm32-unknown-unknown");
        }
    }

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&IntegrationReport {
                schema: "conduit.local-integration/v1",
                passed: true,
                checks: reports,
            })?
        );
    } else if !opts.quiet {
        if opts.dry_run {
            println!("\nLocal integration planned; no checks were executed.");
        } else {
            println!("\nLocal integration passed.");
        }
    }
    Ok(())
}

fn run_check(
    check: &IntegrationCheck,
    root: &Path,
    opts: &GlobalOpts,
    reports: &mut Vec<CheckReport>,
) -> Result<(), StepError> {
    let outcome = run_probe(check.step, root, &silent(opts));
    if !outcome.success {
        print_failure(check, &outcome, opts);
        return Err(StepError::prereq(
            check.step.id,
            format!(
                "{} failed; reproduce with: {}",
                check.label, check.reproduce
            ),
        ));
    }
    reports.push(CheckReport {
        label: check.label,
        status: if outcome.skipped { "planned" } else { "passed" },
        reproduce: check.reproduce,
        elapsed_ms: outcome.elapsed_ms,
    });
    if !opts.quiet && !opts.json {
        println!(
            "{} {}",
            if outcome.skipped { "◇" } else { "✓" },
            check.label
        );
    }
    Ok(())
}

fn print_failure(check: &IntegrationCheck, outcome: &ProbeOutcome, opts: &GlobalOpts) {
    if opts.json {
        println!(
            "{}",
            serde_json::json!({
                "schema": "conduit.local-integration/v1",
                "passed": false,
                "failed": check.label,
                "reproduce": check.reproduce,
                "exit_code": outcome.exit_code,
                "stdout": outcome.stdout,
                "stderr": outcome.stderr,
                "launch_error": outcome.launch_error,
            })
        );
        return;
    }
    eprintln!("✗ {}", check.label);
    if let Some(error) = &outcome.launch_error {
        eprintln!("  {error}");
    }
    if !outcome.stderr.is_empty() {
        eprintln!("{}", outcome.stderr);
    }
    if !outcome.stdout.is_empty() {
        eprintln!("{}", outcome.stdout);
    }
    eprintln!("  Reproduce with: {}", check.reproduce);
}

fn browser_target_available(outcome: &ProbeOutcome) -> bool {
    outcome.success
        && outcome
            .stdout
            .lines()
            .any(|line| line.trim() == "wasm32-unknown-unknown")
}

fn silent(opts: &GlobalOpts) -> GlobalOpts {
    GlobalOpts {
        dry_run: opts.dry_run,
        quiet: true,
        json: false,
        locked: opts.locked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_prerequisite_requires_the_exact_target() {
        let mut outcome = ProbeOutcome {
            id: String::new(),
            description: String::new(),
            command_line: String::new(),
            skipped: false,
            success: true,
            exit_code: Some(0),
            stdout: "thumbv6m-none-eabi\nwasm32-unknown-unknown".into(),
            stderr: String::new(),
            launch_error: None,
            elapsed_ms: 0,
        };
        assert!(browser_target_available(&outcome));
        outcome.stdout = "thumbv6m-none-eabi".into();
        assert!(!browser_target_available(&outcome));
    }

    #[test]
    fn integration_checks_reuse_the_supported_entrances() {
        assert_eq!(CHECKS[1].step.id, "demo.std");
        assert_eq!(CHECKS[3].step.id, "demo.triple");
        assert!(CHECKS[3].step.args.contains(&"--await-terminal"));
        assert_eq!(
            CHECKS[4].reproduce,
            "cargo xtask prove r1-new-plan-recovery"
        );
    }
}
