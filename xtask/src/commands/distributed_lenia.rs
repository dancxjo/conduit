use std::{path::Path, process::Command};

use crate::{
    cli::{GlobalOpts, ProveArgs},
    process::StepError,
};

pub fn run(args: &ProveArgs, root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    let required = |value: &Option<String>, name: &str| {
        value.clone().ok_or_else(|| {
            StepError::prereq(
                "prove.distributed-lenia.arguments",
                format!("--{name} is required"),
            )
        })
    };
    let adapter = required(&args.bluetooth_adapter, "bluetooth-adapter")?;
    let values = [
        required(&args.lenia_wroom_address, "lenia-wroom-address")?,
        required(&args.lenia_wroom_boot, "lenia-wroom-boot")?,
        required(&args.lenia_c3_address, "lenia-c3-address")?,
        required(&args.lenia_c3_boot, "lenia-c3-boot")?,
        required(&args.lenia_pico_address, "lenia-pico-address")?,
        required(&args.lenia_pico_boot, "lenia-pico-boot")?,
    ];
    let evidence_root = args
        .evidence_root
        .clone()
        .unwrap_or_else(|| root.join("target/conduit-evidence/distributed-lenia"));
    let output = evidence_root.join("generation.pgm");
    if opts.dry_run {
        if !opts.quiet {
            println!("would run the bounded three-MCU distributed Lenia proof");
        }
        return Ok(());
    }
    std::fs::create_dir_all(&evidence_root).map_err(|error| {
        StepError::prereq("prove.distributed-lenia.evidence", error.to_string())
    })?;
    let mut command = Command::new("cargo");
    command.current_dir(root).arg("run");
    if opts.locked {
        command.arg("--locked");
    }
    command.args([
        "-p",
        "conduit-std-host",
        "--features",
        "bluetooth-bluez",
        "--bin",
        "distributed-lenia-probe",
        "--",
    ]);
    command.arg(adapter);
    command.args(values);
    command.arg(output);
    if args.withhold_lenia_pico {
        command.args(["--withhold-region", "2"]);
    }
    let status = command
        .status()
        .map_err(|error| StepError::prereq("prove.distributed-lenia.launch", error.to_string()))?;
    if !status.success() {
        return Err(StepError::prereq(
            "prove.distributed-lenia",
            format!("probe exited with {status}"),
        ));
    }
    Ok(())
}
