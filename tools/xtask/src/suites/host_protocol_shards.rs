//! Split the sustained ConduitOS portable proof across the four host runners.
use std::process::Command;

use crate::{process::Step, suites::workspace_shards::WorkspaceShard};

pub const PACKAGE: &str = "conduitos";
const BODY: &str = "protocol_operations::tests::automatic_body";
const CLOCK: &str = "protocol_operations::tests::automatic_clock";
const USB_PLOTS: &str = "usb_protocol_plots";
const USB_HID: &str = "usb_hid_reports";
const LIBRARY: Step = Step::new(
    "check.test.conduitos.library",
    "ConduitOS portable library proof",
    "cargo",
    &[],
);
const INTEGRATION: Step = Step::new(
    "check.test.conduitos.integration",
    "ConduitOS portable integration proof",
    "cargo",
    &[],
);
const DOC: Step = Step::new(
    "check.test.conduitos.doc",
    "ConduitOS documentation proof",
    "cargo",
    &[],
);

pub struct ProtocolProof {
    pub step: &'static Step,
    pub arguments: Vec<String>,
}

pub fn is_host_group(shard: WorkspaceShard) -> bool {
    matches!(
        shard,
        WorkspaceShard::TestHostsStd
            | WorkspaceShard::TestHostsBrowser
            | WorkspaceShard::TestHostsConduitos
            | WorkspaceShard::TestHostsWorkbench
    )
}

pub fn selected() -> Result<bool, String> {
    match std::env::var("CONDUIT_CI_CONDUITOS_PROOF") {
        Err(std::env::VarError::NotPresent) => Ok(true),
        Ok(value) if value == "true" => Ok(true),
        Ok(value) if value == "false" => Ok(false),
        _ => Err("CONDUIT_CI_CONDUITOS_PROOF must be true or false".into()),
    }
}

pub fn library_owner(name: &str) -> Result<WorkspaceShard, String> {
    match (name.contains(BODY), name.contains(CLOCK)) {
        (true, false) => Ok(WorkspaceShard::TestHostsStd),
        (false, true) => Ok(WorkspaceShard::TestHostsBrowser),
        (false, false) => Ok(WorkspaceShard::TestHostsConduitos),
        (true, true) => Err(format!("ambiguous ConduitOS library proof: {name}")),
    }
}

fn integration_owner(name: &str) -> WorkspaceShard {
    if name == USB_HID {
        WorkspaceShard::TestHostsBrowser
    } else if name == USB_PLOTS {
        WorkspaceShard::TestHostsWorkbench
    } else {
        WorkspaceShard::TestHostsConduitos
    }
}

fn arguments() -> Vec<String> {
    ["test", "--locked", "--no-fail-fast", "-p", PACKAGE]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

pub fn proofs(shard: WorkspaceShard) -> Result<Vec<ProtocolProof>, String> {
    if !is_host_group(shard) {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    if shard != WorkspaceShard::TestHostsWorkbench {
        let mut args = arguments();
        args.extend(
            ["--lib", "--", "--test-threads=1"]
                .into_iter()
                .map(str::to_owned),
        );
        match shard {
            WorkspaceShard::TestHostsStd => args.push(BODY.into()),
            WorkspaceShard::TestHostsBrowser => args.push(CLOCK.into()),
            WorkspaceShard::TestHostsConduitos => {
                for prefix in [BODY, CLOCK] {
                    args.extend(["--skip".into(), prefix.into()]);
                }
            }
            _ => unreachable!(),
        }
        result.push(ProtocolProof {
            step: &LIBRARY,
            arguments: args,
        });
    }
    if matches!(
        shard,
        WorkspaceShard::TestHostsConduitos
            | WorkspaceShard::TestHostsWorkbench
            | WorkspaceShard::TestHostsBrowser
    ) {
        let metadata = Command::new("cargo")
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
            .output()
            .map_err(|error| format!("host proof metadata: {error}"))?;
        if !metadata.status.success() {
            return Err(String::from_utf8_lossy(&metadata.stderr).into());
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&metadata.stdout).map_err(|error| error.to_string())?;
        let targets = integration_targets(&metadata)?;
        let owned: Vec<_> = targets
            .iter()
            .filter(|name| integration_owner(name) == shard)
            .collect();
        if !owned.is_empty() {
            let mut args = arguments();
            for name in owned {
                args.extend(["--test".into(), name.clone()]);
            }
            args.extend(["--", "--test-threads=1"].into_iter().map(str::to_owned));
            result.push(ProtocolProof {
                step: &INTEGRATION,
                arguments: args,
            });
        }
    }
    if shard == WorkspaceShard::TestHostsConduitos {
        let mut args = arguments();
        args.extend(
            ["--doc", "--", "--test-threads=1"]
                .into_iter()
                .map(str::to_owned),
        );
        result.push(ProtocolProof {
            step: &DOC,
            arguments: args,
        });
    }
    Ok(result)
}

fn integration_targets(metadata: &serde_json::Value) -> Result<Vec<String>, String> {
    let package = metadata["packages"]
        .as_array()
        .ok_or("metadata packages missing")?
        .iter()
        .find(|package| package["name"] == PACKAGE)
        .ok_or("ConduitOS package missing")?;
    // The old host suite proves the default feature set. Refuse a future default
    // or example change until its additional target obligations are classified.
    if package["features"]["default"] != serde_json::json!([]) {
        return Err(
            "ConduitOS default feature changes require updating portable proof selection".into(),
        );
    }
    let mut targets = Vec::new();
    for target in package["targets"]
        .as_array()
        .ok_or("ConduitOS targets missing")?
    {
        let kinds = target["kind"].as_array().ok_or("target kind missing")?;
        if kinds.iter().any(|kind| kind == "bin") && target["test"] == true {
            return Err("ConduitOS binary tests require updating portable proof selection".into());
        }
        if kinds.iter().any(|kind| kind == "example") {
            return Err("ConduitOS examples require updating portable proof selection".into());
        }
        if kinds.iter().any(|kind| kind == "test")
            && target["test"] == true
            && target["required-features"]
                .as_array()
                .is_none_or(Vec::is_empty)
        {
            targets.push(
                target["name"]
                    .as_str()
                    .ok_or("target name missing")?
                    .to_owned(),
            );
        }
    }
    targets.sort();
    if ![USB_PLOTS, USB_HID]
        .iter()
        .all(|required| targets.iter().any(|name| name == required))
    {
        return Err("USB protocol plot or HID report proof missing".into());
    }
    Ok(targets)
}

// Run once on the library binary which this lane already compiles. Validate
// Cargo's actual test inventory, including future cases, against the selectors.
pub fn validate_library_partition() -> Result<(), String> {
    let mut args = arguments();
    args.extend(["--lib", "--", "--list"].into_iter().map(str::to_owned));
    let output = Command::new("cargo")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    let listing = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    let mut counts = [0; 3];
    for line in listing.lines() {
        let Some(name) = line.strip_suffix(": test") else {
            continue;
        };
        let owner = library_owner(name)?;
        let index = match owner {
            WorkspaceShard::TestHostsStd => 0,
            WorkspaceShard::TestHostsBrowser => 1,
            WorkspaceShard::TestHostsConduitos => 2,
            _ => unreachable!(),
        };
        counts[index] += 1;
    }
    if counts.contains(&0) {
        return Err(format!(
            "empty ConduitOS library proof partition: {counts:?}"
        ));
    }
    eprintln!(
        "ConduitOS library inventory: std={}, browser={}, conduitos={}",
        counts[0], counts[1], counts[2]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_selectors_assign_each_case_exactly_once() {
        for (name, owner) in [
            (
                "protocol_operations::tests::automatic_body::preparation",
                WorkspaceShard::TestHostsStd,
            ),
            (
                "protocol_operations::tests::automatic_clock_failure::lost",
                WorkspaceShard::TestHostsBrowser,
            ),
            (
                "protocol_operations::tests::automatic_clock::present",
                WorkspaceShard::TestHostsBrowser,
            ),
            (
                "protocol_operations::tests::automatic_capture::sample",
                WorkspaceShard::TestHostsConduitos,
            ),
            (
                "usb_base::tests::control",
                WorkspaceShard::TestHostsConduitos,
            ),
        ] {
            assert_eq!(library_owner(name).unwrap(), owner);
            let matching = [
                name.contains(BODY),
                name.contains(CLOCK),
                !name.contains(BODY) && !name.contains(CLOCK),
            ];
            assert_eq!(matching.into_iter().filter(|matches| *matches).count(), 1);
        }
        assert!(library_owner(&format!("{BODY}::{CLOCK}")).is_err());
    }

    #[test]
    fn integration_inventory_preserves_all_default_targets_once() {
        let output = Command::new("cargo")
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let metadata = serde_json::from_slice(&output.stdout).unwrap();
        let targets = integration_targets(&metadata).unwrap();
        assert!(targets.len() > 1);
        for name in &targets {
            assert!(matches!(
                integration_owner(name),
                WorkspaceShard::TestHostsWorkbench
                    | WorkspaceShard::TestHostsConduitos
                    | WorkspaceShard::TestHostsBrowser
            ));
        }
        assert_eq!(integration_owner(USB_HID), WorkspaceShard::TestHostsBrowser);
        assert_eq!(
            targets
                .iter()
                .filter(|name| integration_owner(name) == WorkspaceShard::TestHostsWorkbench)
                .count(),
            1
        );
    }
}
