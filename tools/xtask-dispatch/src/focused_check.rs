//! Run the existing focused acoustic suite without the full tooling build.
use crate::{process::Step, workspace::workspace_root};
#[path = "../../xtask/src/suites/acoustic_flow.rs"]
mod acoustic_flow;

fn selected(arguments: &[String]) -> Option<&'static [Step]> {
    match arguments {
        [check, suite] if check == "check" && suite == "acoustic-flow" => {
            Some(acoustic_flow::STEPS)
        }
        // Global options and all other commands retain the full CLI parser.
        _ => None,
    }
}

pub fn run(arguments: &[String]) -> Option<Result<(), String>> {
    let steps = selected(arguments)?;
    Some((|| {
        let root = workspace_root()?;
        execute(steps, |step| {
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
        let steps = selected(&arguments(&["check", "acoustic-flow"])).unwrap();
        assert_eq!(format!("{steps:?}"), format!("{:?}", acoustic_flow::STEPS));
    }
    #[test]
    fn options_and_other_commands_retain_full_parser() {
        for values in [
            vec!["--json", "check", "acoustic-flow"],
            vec!["check", "acoustic-flow", "--quiet"],
            vec!["check", "acoustic-flow", "--dry-run"],
            vec!["check", "acoustic-flow", "unexpected"],
            vec!["check", "workspace"],
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
