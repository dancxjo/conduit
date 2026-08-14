use std::{path::Path, process::Command};

use crate::{
    cli::{BluetoothProofRole, GlobalOpts, ProveArgs},
    process::StepError,
};

pub fn run(args: &ProveArgs, root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    let evidence_root = args.evidence_root.as_deref().ok_or_else(|| {
        StepError::prereq(
            "prove.bluetooth-line.arguments",
            "--evidence-root <directory> is required for retained physical evidence",
        )
    })?;
    let role = args.bluetooth_role.ok_or_else(|| {
        StepError::prereq(
            "prove.bluetooth-line.arguments",
            "--bluetooth-role source|sink is required",
        )
    })?;
    let adapter = args.bluetooth_adapter.as_deref().ok_or_else(|| {
        StepError::prereq(
            "prove.bluetooth-line.arguments",
            "--bluetooth-adapter <hci-name> is required",
        )
    })?;
    let peer = args.bluetooth_peer_address.as_deref().ok_or_else(|| {
        StepError::prereq(
            "prove.bluetooth-line.arguments",
            "--bluetooth-peer-address <address> is required",
        )
    })?;
    let role = match role {
        BluetoothProofRole::Source => "source",
        BluetoothProofRole::Sink => "sink",
    };
    let mut displayed = vec![
        "cargo",
        "run",
        "--package",
        "conduit-std-host",
        "--features",
        "bluetooth-bluez",
        "--bin",
        "bluetooth-line-probe",
        "--",
        role,
        adapter,
        peer,
    ];
    if opts.locked {
        displayed.insert(2, "--locked");
    }
    if !opts.quiet && !opts.json {
        println!("» [prove.bluetooth-line] Run one exact bounded BlueZ BLE GATT proof side");
        println!("  $ {}", displayed.join(" "));
    }
    if opts.dry_run {
        return Ok(());
    }

    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .arg("run")
        .arg("--package")
        .arg("conduit-std-host")
        .arg("--features")
        .arg("bluetooth-bluez")
        .arg("--bin")
        .arg("bluetooth-line-probe");
    if opts.locked {
        command.arg("--locked");
    }
    let output = command
        .arg("--")
        .arg(role)
        .arg(adapter)
        .arg(peer)
        .output()
        .map_err(|error| StepError::prereq("prove.bluetooth-line.launch", error.to_string()))?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(StepError {
            id: "prove.bluetooth-line".into(),
            command_line: displayed.join(" "),
            status: Some(output.status),
            message: String::new(),
        });
    }
    std::fs::create_dir_all(evidence_root)
        .map_err(|error| StepError::prereq("prove.bluetooth-line.evidence", error.to_string()))?;
    let receipt = evidence_root.join(format!("bluetooth-line-{role}.json"));
    std::fs::write(&receipt, &output.stdout)
        .map_err(|error| StepError::prereq("prove.bluetooth-line.evidence", error.to_string()))?;
    if !opts.quiet {
        print!("{}", String::from_utf8_lossy(&output.stdout));
        println!("retained {}", receipt.display());
    }
    Ok(())
}
