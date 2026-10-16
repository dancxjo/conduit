//! Run exact named proof suites from their canonical shared definitions.
use crate::{process::Step, workspace::workspace_root};
#[path = "../../xtask/src/suites/acoustic_control.rs"]
mod acoustic_control;
#[path = "../../xtask/src/suites/acoustic_flow.rs"]
mod acoustic_flow;
#[path = "../../xtask/src/suites/angle_reference.rs"]
mod angle_reference;

struct Suite {
    steps: &'static [Step],
    path: &'static str,
    source: &'static [u8],
}

fn selected(arguments: &[String]) -> Option<Suite> {
    match arguments {
        [check, suite] if check == "check" && suite == "acoustic-flow" => Some(Suite {
            steps: acoustic_flow::STEPS,
            path: "tools/xtask/src/suites/acoustic_flow.rs",
            source: include_bytes!("../../xtask/src/suites/acoustic_flow.rs"),
        }),
        [check, suite] if check == "check" && suite == "acoustic-control" => Some(Suite {
            steps: acoustic_control::STEPS,
            path: "tools/xtask/src/suites/acoustic_control.rs",
            source: include_bytes!("../../xtask/src/suites/acoustic_control.rs"),
        }),
        [check, suite] if check == "check" && suite == "angle-reference" => Some(Suite {
            steps: angle_reference::STEPS,
            path: "tools/xtask/src/suites/angle_reference.rs",
            source: include_bytes!("../../xtask/src/suites/angle_reference.rs"),
        }),
        // Global options and all other commands retain the full CLI parser.
        _ => None,
    }
}

pub fn run(arguments: &[String]) -> Option<Result<(), String>> {
    let suite = selected(arguments)?;
    Some((|| {
        let root = workspace_root()?;
        let current = std::fs::read(root.join(suite.path))
            .map_err(|error| format!("cannot verify focused suite {}: {error}", suite.path))?;
        if current != suite.source {
            return Err(format!(
                "focused suite {} differs from this compiled dispatcher; rebuild the dispatcher from this checkout using a separate build cache before claiming complete proof",
                suite.path
            ));
        }
        execute(suite.steps, |step| {
            println!("» [{}] {}", step.id, step.description);
            println!("  $ {step}");
            let directory = step.cwd.map_or_else(|| root.clone(), |cwd| root.join(cwd));
            let status = crate::process::command_for(step.program)
                .args(step.args)
                .envs(step.environment.iter().copied())
                .current_dir(directory)
                .status()
                .map_err(|error| format!("{}: {error}", step.id))?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("{}: {step} failed: {status}", step.id))
            }
        })
    })())
}

fn execute(steps: &[Step], mut run: impl FnMut(&Step) -> Result<(), String>) -> Result<(), String> {
    for step in steps {
        run(step)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    #[test]
    fn exact_entrance_selects_the_shared_complete_suite() {
        for (name, expected) in [
            ("acoustic-flow", acoustic_flow::STEPS),
            ("acoustic-control", acoustic_control::STEPS),
            ("angle-reference", angle_reference::STEPS),
        ] {
            let steps = selected(&arguments(&["check", name])).unwrap().steps;
            assert_eq!(format!("{steps:?}"), format!("{expected:?}"), "{name}");
        }
    }
    #[test]
    fn options_and_other_commands_retain_full_parser() {
        for values in [
            vec!["--json", "check", "acoustic-flow"],
            vec!["check", "acoustic-flow", "--quiet"],
            vec!["check", "acoustic-flow", "--dry-run"],
            vec!["check", "acoustic-flow", "unexpected"],
            vec!["check", "workspace"],
            vec!["--json", "check", "acoustic-control"],
            vec!["check", "acoustic-control", "--dry-run"],
            vec!["--json", "check", "angle-reference"],
            vec!["check", "angle-reference", "--quiet"],
            vec!["ci", "pipeline", "unit", "foundation"],
            vec!["check"],
        ] {
            assert!(selected(&arguments(&values)).is_none(), "{values:?}");
        }
    }
    #[test]
    fn failure_stops_without_retry_or_skipping_a_prior_step() {
        let steps = [
            Step::new("first", "", "", &[]),
            Step::new("failure", "", "", &[]),
            Step::new("later", "", "", &[]),
        ];
        let mut seen = Vec::new();
        let result = execute(&steps, |step| {
            seen.push(step.id);
            if step.id == "failure" {
                Err("failure".into())
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err("failure".into()));
        assert_eq!(seen, ["first", "failure"]);
    }
}
