use std::{
    path::Path,
    process::{Command, Output},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

mod transcript;

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
    let peer_identity = match (
        args.bluetooth_peer_host_id.as_deref(),
        args.bluetooth_peer_boot_id.as_deref(),
    ) {
        (Some(host), Some(boot)) => Some((host, boot)),
        (None, None) => None,
        _ => {
            return Err(StepError::prereq(
                "prove.bluetooth-line.arguments",
                "--bluetooth-peer-host-id and --bluetooth-peer-boot-id must be supplied together",
            ));
        }
    };
    let role = match role {
        BluetoothProofRole::Source => "source",
        BluetoothProofRole::Sink => "sink",
    };
    let operation = if args.induce_transport_loss {
        "loss"
    } else {
        role
    };
    let esp32_peer = peer_identity.is_some_and(|(host, _)| host.starts_with("esp32/"));
    let sign_port = if esp32_peer {
        Some(args.sign_port.as_deref().ok_or_else(|| {
            StepError::prereq(
                "prove.bluetooth-line.arguments",
                "--sign-port <path> is required for retained ESP32 firmware evidence",
            )
        })?)
    } else {
        None
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
        operation,
        adapter,
        peer,
    ];
    if opts.locked {
        displayed.insert(2, "--locked");
    }
    if let Some((host, boot)) = peer_identity {
        displayed.extend([host, boot]);
    }
    if !opts.quiet && !opts.json {
        println!("» [prove.bluetooth-line] Run one exact bounded BlueZ BLE GATT proof side");
        println!("  $ {}", displayed.join(" "));
    }
    if opts.dry_run {
        return Ok(());
    }

    std::fs::create_dir_all(evidence_root)
        .map_err(|error| StepError::prereq("prove.bluetooth-line.evidence", error.to_string()))?;
    let capture = if let Some(sign_port) = sign_port {
        let port = sign_port_options(false)
            .open(sign_port)
            .map_err(|error| StepError::prereq("prove.bluetooth-line.sign", error.to_string()))?;
        conduit_std_host::usb_cdc::configure_cdc_port(&port, 0, 10)
            .map_err(|error| StepError::prereq("prove.bluetooth-line.sign", error.to_string()))?;
        let terminal_marker = if args.induce_transport_loss {
            "CONDUIT_ESP32_BLE_LOST"
        } else {
            "CONDUIT_ESP32_LINE_COMPLETE"
        };
        let stop = Arc::new(AtomicBool::new(false));
        let capture_stop = Arc::clone(&stop);
        Some((
            stop,
            std::thread::spawn(move || {
                transcript::capture(port, terminal_marker, capture_stop, None)
            }),
        ))
    } else {
        None
    };

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
    let command = command.arg("--").arg(operation).arg(adapter).arg(peer);
    if let Some((host, boot)) = peer_identity {
        command.arg(host).arg(boot);
    }
    let output = command
        .output()
        .map_err(|error| StepError::prereq("prove.bluetooth-line.launch", error.to_string()))?;
    let transcript = if let Some((stop, capture)) = capture {
        if !output.status.success() {
            stop.store(true, Ordering::Release);
        }
        let lines = capture
            .join()
            .map_err(|_| StepError::prereq("prove.bluetooth-line.sign", "capture thread panicked"))?
            .map_err(|error| StepError::prereq("prove.bluetooth-line.sign", error))?;
        let path = evidence_root.join("bluetooth-line-esp32-transcript.jsonl");
        std::fs::write(&path, format!("{}\n", lines.join("\n"))).map_err(|error| {
            StepError::prereq("prove.bluetooth-line.evidence", error.to_string())
        })?;
        if output.status.success() {
            let (expected_host, expected_boot) =
                peer_identity.expect("ESP32 capture requires identity");
            transcript::verify_esp32(
                &lines,
                args.induce_transport_loss,
                expected_host,
                expected_boot,
                peer,
            )
            .map_err(|error| StepError::prereq("prove.bluetooth-line.transcript", error))?;
        }
        Some(path)
    } else {
        None
    };
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(StepError {
            id: "prove.bluetooth-line".into(),
            command_line: displayed.join(" "),
            status: Some(output.status),
            message: "retained partial ESP32 transcript after hosted probe failure".into(),
        });
    }
    let receipt = evidence_root.join(format!("bluetooth-line-{operation}.json"));
    std::fs::write(&receipt, &output.stdout)
        .map_err(|error| StepError::prereq("prove.bluetooth-line.evidence", error.to_string()))?;
    if !opts.quiet {
        print!("{}", String::from_utf8_lossy(&output.stdout));
        println!("retained {}", receipt.display());
        if let Some(transcript) = transcript {
            println!("retained {}", transcript.display());
        }
    }
    Ok(())
}

pub fn run_pico(args: &ProveArgs, root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    let evidence_root = args.evidence_root.as_deref().ok_or_else(|| {
        StepError::prereq(
            "prove.bluetooth-pico.arguments",
            "--evidence-root <directory> is required for retained physical evidence",
        )
    })?;
    let adapter = args.bluetooth_adapter.as_deref().ok_or_else(|| {
        StepError::prereq(
            "prove.bluetooth-pico.arguments",
            "--bluetooth-adapter <hci-name> is required",
        )
    })?;
    // Opening the sign port starts the constrained firmware and therefore its
    // fresh per-Boot random address. Let the bounded probe discover exactly
    // one compatible candidate when an address was not supplied, then retain
    // the address it actually observed in the hosted and aggregate receipts.
    let peer = args.bluetooth_peer_address.as_deref().unwrap_or("auto");
    let sign_port = args.sign_port.as_deref().ok_or_else(|| {
        StepError::prereq(
            "prove.bluetooth-pico.arguments",
            "--sign-port <path> is required",
        )
    })?;
    if !opts.quiet && !opts.json {
        println!("» [prove.bluetooth-pico] Pair and prove one exact hosted↔Pico BLE GATT Line");
    }
    if opts.dry_run {
        println!(
            "  sign={} adapter={} evidence={}",
            sign_port,
            adapter,
            evidence_root.display()
        );
        return Ok(());
    }

    std::fs::create_dir_all(evidence_root)
        .map_err(|error| StepError::prereq("prove.bluetooth-pico.evidence", error.to_string()))?;
    for name in [
        "bluetooth-pico-prepare.json",
        "bluetooth-pico-hosted.json",
        "bluetooth-pico-transcript.jsonl",
        "bluetooth-pico.json",
    ] {
        let path = evidence_root.join(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|error| {
                StepError::prereq("prove.bluetooth-pico.evidence", error.to_string())
            })?;
        }
    }

    let port = sign_port_options(true)
        .open(sign_port)
        .map_err(|error| StepError::prereq("prove.bluetooth-pico.sign", error.to_string()))?;
    conduit_std_host::usb_cdc::configure_cdc_port(&port, 0, 10)
        .map_err(|error| StepError::prereq("prove.bluetooth-pico.sign", error.to_string()))?;
    conduit_std_host::usb_cdc::assert_dtr(&port)
        .map_err(|error| StepError::prereq("prove.bluetooth-pico.sign", error.to_string()))?;
    let loss = args.induce_transport_loss;
    let terminal_marker = if loss {
        "CONDUIT_BLE_LINE_LOST"
    } else {
        "CONDUIT_BLE_LINE_COMPLETE"
    };
    let stop = Arc::new(AtomicBool::new(false));
    let capture_stop = Arc::clone(&stop);
    let (boot_sender, boot_receiver) = std::sync::mpsc::sync_channel(1);
    let capture = std::thread::spawn(move || {
        transcript::capture(port, terminal_marker, capture_stop, Some(boot_sender))
    });

    let runtime_boot = match boot_receiver.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(boot) => boot,
        Err(_) => {
            stop.store(true, Ordering::Release);
            let _ = capture.join();
            return Err(StepError::prereq(
                "prove.bluetooth-pico.identity",
                "timed out waiting for the Pico runtime Boot identity",
            ));
        }
    };

    let operation = if loss { "loss" } else { "source" };
    let hosted = run_probe(
        root,
        opts.locked,
        &[
            operation,
            adapter,
            peer,
            conduit_signal_conformance::STD_PICO_USB_SINK_HOST_ID,
            runtime_boot.as_str(),
        ],
    )?;
    if !hosted.status.success() {
        stop.store(true, Ordering::Release);
    }
    let lines = capture
        .join()
        .map_err(|_| StepError::prereq("prove.bluetooth-pico.sign", "capture thread panicked"))?
        .map_err(|error| StepError::prereq("prove.bluetooth-pico.sign", error))?;
    std::fs::write(
        evidence_root.join("bluetooth-pico-transcript.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .map_err(|error| StepError::prereq("prove.bluetooth-pico.evidence", error.to_string()))?;
    if !hosted.status.success() {
        eprint!("{}", String::from_utf8_lossy(&hosted.stderr));
        return Err(StepError {
            id: "prove.bluetooth-pico".into(),
            command_line: format!(
                "cargo run ... bluetooth-line-probe -- {}",
                [operation, adapter, peer].join(" ")
            ),
            status: Some(hosted.status),
            message: "retained partial Pico transcript after hosted probe failure".into(),
        });
    }
    let hosted_record = parse_probe_record(&hosted, operation)?;
    let observed_peer = hosted_record["peer_address"].as_str().ok_or_else(|| {
        StepError::prereq(
            "prove.bluetooth-pico.receipt",
            "hosted receipt lacks the observed peer address",
        )
    })?;
    let prepare_record = serde_json::json!({
        "success": true,
        "operation": "prepare",
        "adapter": adapter,
        "peer_address": observed_peer,
        "paired": hosted_record["paired"],
        "connection_adopted_by_line": true,
    });
    let identity = crate::commands::pico::read_identity_manifest(root)
        .map_err(|error| StepError::prereq("prove.bluetooth-pico.identity", error.to_string()))?;
    let summary = if loss {
        crate::commands::pico::verify_bluetooth_loss_transcript(&lines, &identity)
    } else {
        crate::commands::pico::verify_bluetooth_transcript(&lines, &identity)
    }
    .map_err(|error| StepError::prereq("prove.bluetooth-pico.transcript", error.to_string()))?;

    std::fs::write(
        evidence_root.join("bluetooth-pico-prepare.json"),
        format!("{}\n", prepare_record),
    )
    .and_then(|()| {
        std::fs::write(
            evidence_root.join("bluetooth-pico-hosted.json"),
            format!("{}\n", hosted_record),
        )
    })
    .map_err(|error| StepError::prereq("prove.bluetooth-pico.evidence", error.to_string()))?;
    let aggregate = serde_json::json!({
        "schema": "conduit-proof/bluetooth-pico@1",
        "success": true,
        "proof_class": "physical-hardware",
        "disposition": if loss { "transport-lost" } else { "completed" },
        "adapter": adapter,
        "peer_address": observed_peer,
        "paired": prepare_record["paired"],
        "base": hosted_record["base"],
        "plan_id": hosted_record["plan_id"],
        "connection_id": hosted_record["connection_id"],
        "base_instance_id": hosted_record["base_instance_id"],
        "firmware_build_id": identity.firmware_build_id,
        "runtime": summary,
        "transcript": "bluetooth-pico-transcript.jsonl",
        "hosted_receipt": "bluetooth-pico-hosted.json",
        "pairing_receipt": "bluetooth-pico-prepare.json",
    });
    std::fs::write(
        evidence_root.join("bluetooth-pico.json"),
        format!("{}\n", serde_json::to_string_pretty(&aggregate).unwrap()),
    )
    .map_err(|error| StepError::prereq("prove.bluetooth-pico.evidence", error.to_string()))?;
    if !opts.quiet {
        println!(
            "retained {}",
            evidence_root.join("bluetooth-pico.json").display()
        );
    }
    Ok(())
}

fn run_probe(root: &Path, locked: bool, arguments: &[&str]) -> Result<Output, StepError> {
    let mut command = Command::new("cargo");
    command.current_dir(root).arg("run");
    if locked {
        command.arg("--locked");
    }
    let output = command
        .arg("--package")
        .arg("conduit-std-host")
        .arg("--features")
        .arg("bluetooth-bluez")
        .arg("--bin")
        .arg("bluetooth-line-probe")
        .arg("--")
        .args(arguments)
        .output()
        .map_err(|error| StepError::prereq("prove.bluetooth-pico.launch", error.to_string()))?;
    Ok(output)
}

fn sign_port_options(writable: bool) -> std::fs::OpenOptions {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(writable);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A proof process launched without an existing controlling terminal
        // must not acquire the Pico CDC endpoint as one. The firmware's
        // intentional post-terminal USB re-enumeration would otherwise send
        // SIGHUP to the proof process before it can retain its receipt.
        // Linux uapi asm-generic/fcntl.h: O_NOCTTY = 0o00000400.
        options.custom_flags(0o00000400);
    }
    options
}

fn parse_probe_record(output: &Output, operation: &str) -> Result<serde_json::Value, StepError> {
    let record: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        StepError::prereq(
            "prove.bluetooth-pico.receipt",
            format!("invalid {operation} receipt: {error}"),
        )
    })?;
    if record["success"].as_bool() != Some(true) {
        return Err(StepError::prereq(
            "prove.bluetooth-pico.receipt",
            format!("{operation} receipt did not report success"),
        ));
    }
    Ok(record)
}
