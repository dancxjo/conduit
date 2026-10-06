//! Small entrance for CI work; Actions only schedules these commands.
use std::process::Command;

use crate::suites::{check::WORKSPACE_STEPS, workspace_shards::WorkspaceShard};

pub(super) fn run(arguments: &[String]) -> Result<(), String> {
    if arguments.first().map(String::as_str) == Some("unit") {
        if arguments.len() != 2 {
            return Err(
                "usage: cargo xtask ci pipeline unit <foundation|hosts-std|hosts-browser|hosts-conduitos|hosts-workbench|hosts|products|lint>".into(),
            );
        }
        return unit(&arguments[1]);
    }
    let status = Command::new("node")
        .arg("tools/ci/pipeline/cli.mjs")
        .args(arguments)
        .status()
        .map_err(|error| format!("launch pipeline: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("pipeline failed: {status}"))
    }
}

fn unit(name: &str) -> Result<(), String> {
    let shard = match name {
        "foundation" => WorkspaceShard::TestFoundation,
        "hosts" => WorkspaceShard::TestHosts,
        "hosts-std" => WorkspaceShard::TestHostsStd,
        "hosts-browser" => WorkspaceShard::TestHostsBrowser,
        "hosts-conduitos" => WorkspaceShard::TestHostsConduitos,
        "hosts-workbench" => WorkspaceShard::TestHostsWorkbench,
        "products" => WorkspaceShard::TestProducts,
        "lint" => WorkspaceShard::Lint,
        _ => return Err(format!("unknown unit shard: {name}")),
    };
    let steps = shard
        .package_test_step()
        .into_iter()
        // Formatting is already required by preflight on this exact source.
        .chain(
            WORKSPACE_STEPS
                .iter()
                .filter(|step| shard.owns(step) && step.id != "check.fmt"),
        );
    for step in steps {
        eprintln!("UNIT {name}: {}", step.description);
        let mut command = Command::new(step.program);
        if step.program == "cargo"
            && matches!(step.args.first(), Some(&"test" | &"clippy" | &"check"))
        {
            command
                .arg(step.args[0])
                .arg("--locked")
                .args(&step.args[1..]);
        } else {
            command.args(step.args);
        }
        let status = command
            .status()
            .map_err(|error| format!("{}: {error}", step.id))?;
        if !status.success() {
            let head = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .map_err(|error| error.to_string())?;
            return Err(format!(
                "FAILED: {name}/{}\nreproduce: cargo xtask ci pipeline unit {name}\ncandidate: {}\nIndependent target proof runs in parallel. See the test failure above for its exact test filter.",
                step.id, String::from_utf8_lossy(&head.stdout).trim()
            ));
        }
    }
    Ok(())
}
