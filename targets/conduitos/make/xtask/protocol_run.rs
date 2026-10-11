//! Bounded emulator observation of an exact packaged protocol product image.
use super::{ConduitosError, report::sha256_file};
use crate::cli::GlobalOpts;
use clap::Args;
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const MAXIMUM_TRANSCRIPT: u64 = 262144;

#[derive(Args, Debug)]
pub(super) struct RunArgs {
    /// Directory produced by protocol-image; the retained digest must match.
    #[arg(long)]
    media: PathBuf,
    /// New directory retaining serial, emulator diagnostics and observed outcome.
    #[arg(long)]
    output_dir: PathBuf,
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=120))]
    seconds: u64,
    /// Attach an isolated emulated mouse to xHCI for class-neutral control proof.
    /// This supplies hardware only; the checked Plot and Root still admit effects.
    #[arg(long)]
    usb_control_mouse: bool,
    /// Inject exactly one known mouse report after the guest begins product Play.
    #[arg(long, requires = "usb_control_mouse")]
    usb_mouse_report: bool,
    /// Optional local emulator executable for the receipt's exact architecture.
    #[arg(long)]
    emulator: Option<PathBuf>,
    /// AArch64 UEFI firmware; required for the pinned virt product profile.
    #[arg(long)]
    aarch64_firmware: Option<PathBuf>,
}

fn emulator_profile(
    architecture: &str,
) -> Result<(&'static str, &'static str, Vec<&'static str>), ConduitosError> {
    match architecture {
        "x86_64" => Ok((
            "qemu-system-x86_64",
            "q35-single-cpu-512m-headless",
            vec![
                "-M",
                "q35",
                "-vga",
                "std",
                "-device",
                "isa-debug-exit,iobase=0xf4,iosize=0x04",
            ],
        )),
        "aarch64" => Ok((
            "qemu-system-aarch64",
            "virt-10.2-gic2-coherent-pcie-single-cpu-512m-headless",
            vec!["-M", "virt-10.2,highmem=off,gic-version=2,iommu=none"],
        )),
        _ => Err(refusal(
            "protocol-run-architecture-refused",
            "no admitted emulator profile for this architecture",
        )),
    }
}

pub(super) fn execute(args: RunArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        println!(
            "Observe exact protocol media {} for at most {} seconds",
            args.media.display(),
            args.seconds
        );
        return Ok(());
    }
    let receipt: Value =
        serde_json::from_slice(&read(&args.media.join("protocol-receipt.json"), 16384)?)
            .map_err(|error| refusal("protocol-run-receipt-refused", error))?;
    let image = args.media.join("conduitos.iso");
    let digest = sha256_file(&image)?;
    if receipt["schema"] != "conduit.conduitos/protocol-image-receipt@1"
        || receipt["image_sha256"] != digest
    {
        return Err(refusal(
            "protocol-run-image-refused",
            "exact packaged image digest required",
        ));
    }
    let architecture = receipt["architecture"].as_str().unwrap_or("x86_64");
    let (default_emulator, profile, machine_args) = emulator_profile(architecture)?;
    if architecture == "aarch64" && args.aarch64_firmware.is_none() {
        return Err(refusal(
            "protocol-run-firmware-required",
            "AArch64 requires explicit UEFI firmware",
        ));
    }
    if architecture != "aarch64" && args.aarch64_firmware.is_some() {
        return Err(refusal(
            "protocol-run-firmware-refused",
            "AArch64 firmware cannot be used with another architecture",
        ));
    }
    fs::create_dir(&args.output_dir)
        .map_err(|error| refusal("protocol-run-output-refused", error))?;
    let serial = args.output_dir.join("serial.log");
    let stderr = args.output_dir.join("qemu.log");
    let output_handle = if args.usb_mouse_report {
        Some(
            fs::File::open(&args.output_dir)
                .map_err(|error| refusal("protocol-run-output-refused", error))?,
        )
    } else {
        None
    };
    let observer_socket = output_handle.as_ref().map(|directory| {
        PathBuf::from(format!("/proc/self/fd/{}/qmp.sock", directory.as_raw_fd()))
    });
    let serial_target = if args.usb_mouse_report {
        format!(
            "file:{}",
            fs::canonicalize(&args.output_dir)
                .map_err(|error| refusal("protocol-run-output-refused", error))?
                .join("serial.log")
                .display()
        )
    } else {
        format!("file:{}", serial.display())
    };
    let launch_image = if args.usb_mouse_report {
        fs::canonicalize(&image).map_err(|error| refusal("protocol-run-image-refused", error))?
    } else {
        image
    };
    let mut command = Command::new(
        args.emulator
            .as_deref()
            .unwrap_or(Path::new(default_emulator)),
    );
    command.args(machine_args);
    if let Some(firmware) = &args.aarch64_firmware {
        command.arg("-bios").arg(firmware);
    }
    command
        .args([
            "-cpu",
            "max",
            "-m",
            "512M",
            "-smp",
            "1",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
            &serial_target,
            "-no-reboot",
            "-net",
            "none",
            "-cdrom",
        ])
        .arg(&launch_image)
        .args(["-boot", "d"])
        .args(usb_devices(args.usb_control_mouse))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            fs::File::create(&stderr)
                .map_err(|error| refusal("protocol-run-output-refused", error))?,
        );
    if args.usb_mouse_report {
        command
            .current_dir(&args.output_dir)
            .args(["-qmp", "unix:qmp.sock,server=on,wait=off"]);
    }
    let mut child = command
        .spawn()
        .map_err(|error| refusal("protocol-run-emulator-unavailable", error))?;
    let started = Instant::now();
    let mut injected = false;
    let observed = (|| loop {
        let bytes = if serial.exists() {
            read(&serial, MAXIMUM_TRANSCRIPT)?
        } else {
            vec![]
        };
        if fs::metadata(&stderr)
            .map_err(|error| refusal("protocol-run-diagnostics-refused", error))?
            .len()
            > MAXIMUM_TRANSCRIPT
        {
            return Err(refusal(
                "protocol-run-transcript-bound",
                "emulator diagnostics exceeded admission",
            ));
        }
        if let Some(outcome) = terminal(&bytes)? {
            return Ok(outcome);
        }
        if args.usb_mouse_report && !injected && play_observed(&bytes) {
            // The shared QMP transport has finite 2-second connect/request bounds.
            // Reserve those bounds inside this run's existing deadline; never
            // restart the emulator or extend the admitted observation lifetime.
            if Duration::from_secs(args.seconds).saturating_sub(started.elapsed())
                < Duration::from_secs(8)
            {
                return Err(refusal(
                    "protocol-run-input-deadline",
                    "insufficient remaining QMP observation bound",
                ));
            }
            let socket = observer_socket
                .as_ref()
                .ok_or_else(|| refusal("protocol-run-input-refused", "missing observer socket"))?;
            let transcript = args.output_dir.join("qmp.jsonl");
            let (mut stream, mut reader) =
                super::qmp::connect_traced(socket, &mut child, Some(&transcript))?;
            super::qmp::request(
                &mut stream,
                &mut reader,
                mouse_report_command().to_string().as_bytes(),
                "protocol-mouse-report",
            )?;
            injected = true;
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| refusal("protocol-run-observation-refused", error))?
        {
            return Err(refusal("protocol-run-ended-without-terminal", status));
        }
        if started.elapsed() >= Duration::from_secs(args.seconds) {
            return Err(refusal(
                "protocol-run-timeout",
                "no protocol terminal outcome observed",
            ));
        }
        thread::sleep(Duration::from_millis(50));
    })();
    // The observer always retires its isolated emulator, including observation
    // errors. A timeout is not evidence that native owners retired themselves.
    let _ = child.kill();
    let _ = child.wait();
    let outcome = observed?;
    let observed = json!({
        "schema": "conduit.conduitos/protocol-run-receipt@1",
        "proof_class": "freestanding-emulator",
        "image_sha256": digest,
        "outcome": outcome,
        "serial_sha256": sha256_file(&serial)?,
        "qemu_profile": profile,
        "architecture": architecture,
        "emulator": args.emulator.as_deref().unwrap_or(Path::new(default_emulator)),
        "aarch64_firmware_sha256": args.aarch64_firmware.as_deref().map(sha256_file).transpose()?,
        "physical_compatibility": "unverified",
        "usb_devices": usb_devices(args.usb_control_mouse),
        "fixture_input": {
            "requested": args.usb_mouse_report,
            "injected": injected,
            "command": if args.usb_mouse_report { mouse_report_command() } else { Value::Null },
            "injection_gate": "CONDUIT_PROTOCOL_PLAY",
            "maximum_reports": if args.usb_mouse_report { 1 } else { 0 },
        },
    });
    fs::write(
        args.output_dir.join("receipt.json"),
        serde_json::to_vec_pretty(&observed)
            .map_err(|error| refusal("protocol-run-receipt-refused", error))?,
    )
    .map_err(|error| refusal("protocol-run-output-refused", error))?;
    println!("Observed protocol outcome: {outcome}");
    Ok(())
}

fn play_observed(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes)
        .split_inclusive('\n')
        .any(|line| line.ends_with('\n') && line.starts_with("CONDUIT_PROTOCOL_PLAY "))
}
fn mouse_report_command() -> Value {
    json!({"execute":"input-send-event","arguments":{"events":[
        {"type":"btn","data":{"button":"left","down":true}},
        {"type":"rel","data":{"axis":"x","value":1}},
        {"type":"rel","data":{"axis":"y","value":2}}
    ]}})
}

fn usb_devices(mouse: bool) -> &'static [&'static str] {
    if mouse {
        &[
            "-device",
            "qemu-xhci,id=protocol-usb",
            "-device",
            "usb-mouse,bus=protocol-usb.0,port=1",
        ]
    } else {
        &[]
    }
}

fn terminal(bytes: &[u8]) -> Result<Option<String>, ConduitosError> {
    let text = String::from_utf8_lossy(bytes);
    for line in text
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim() == "CONDUIT_PROTOCOL_RETIRED complete" {
            if !text
                .lines()
                .any(|line| line.starts_with("CONDUIT_PROTOCOL_PLAN "))
                || !text
                    .lines()
                    .any(|line| line.starts_with("CONDUIT_PROTOCOL_PLAY "))
            {
                return Err(refusal(
                    "protocol-run-retirement-without-play",
                    "missing plan/play observations",
                ));
            }
            return Ok(Some("play-retired".into()));
        }
        if let Some(reason) = line.strip_prefix("CONDUIT_PROTOCOL_REFUSAL ") {
            return Ok(Some(format!("play-refused:{reason}")));
        }
        for (prefix, schema, stage) in [
            (
                "CONDUIT_BOOT_SIGN ",
                conduitos::sign_format::BOOT_SIGN_SCHEMA,
                "boot",
            ),
            (
                "CONDUIT_KERNEL_SIGN ",
                conduitos::sign_format::MACHINE_SIGN_SCHEMA,
                "root",
            ),
        ] {
            if let Some(json) = line.strip_prefix(prefix) {
                let sign: Value = serde_json::from_str(json)
                    .map_err(|error| refusal("protocol-run-sign-refused", error))?;
                if sign["schema"] == schema && sign["status"] == "refused" {
                    let reason = sign["reason"].as_str().ok_or_else(|| {
                        refusal("protocol-run-sign-refused", "missing refusal reason")
                    })?;
                    return Ok(Some(format!("{stage}-refused:{reason}")));
                }
            }
        }
    }
    Ok(None)
}
fn read(path: &Path, maximum: u64) -> Result<Vec<u8>, ConduitosError> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| refusal("protocol-run-read-refused", error))?
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| refusal("protocol-run-read-refused", error))?;
    if bytes.len() as u64 > maximum {
        return Err(refusal("protocol-run-transcript-bound", path.display()));
    }
    Ok(bytes)
}
fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouse_fixture_is_one_exact_report_after_complete_play_observation() {
        assert!(!play_observed(b"CONDUIT_PROTOCOL_PLAN plan=p\n"));
        assert!(!play_observed(b"CONDUIT_PROTOCOL_PLAY play=x"));
        assert!(play_observed(b"CONDUIT_PROTOCOL_PLAY play=x\r\n"));
        assert_eq!(
            mouse_report_command(),
            json!({"execute":"input-send-event","arguments":{"events":[
                {"type":"btn","data":{"button":"left","down":true}},
                {"type":"rel","data":{"axis":"x","value":1}},
                {"type":"rel","data":{"axis":"y","value":2}}
            ]}})
        );
    }
    #[test]
    fn refusal_and_retirement_are_distinct_and_retirement_requires_actual_play() {
        assert!(terminal(b"CONDUIT_PROTOCOL_RETIRED complete\n").is_err());
        assert_eq!(terminal(b"CONDUIT_PROTOCOL_PLAN source=s plan=p\nCONDUIT_PROTOCOL_PLAY play=x\nCONDUIT_PROTOCOL_RETIRED complete\n").unwrap().as_deref(), Some("play-retired"));
        assert_eq!(
            terminal(b"CONDUIT_PROTOCOL_REFUSAL protocol-retirement-refused\n")
                .unwrap()
                .as_deref(),
            Some("play-refused:protocol-retirement-refused")
        );
        assert_eq!(terminal(b"CONDUIT_KERNEL_SIGN {\"schema\":\"conduit.conduitos.kernel-sign/v2\",\"status\":\"refused\",\"reason\":\"protocol-controller-unavailable\"}\n").unwrap().as_deref(), Some("root-refused:protocol-controller-unavailable"));
        assert_eq!(terminal(b"CONDUIT_BOOT_SIGN {\"schema\":\"conduit.conduitos.boot-sign/v1\",\"status\":\"refused\",\"reason\":\"overlapping-artifacts\"}\n").unwrap().as_deref(), Some("boot-refused:overlapping-artifacts"));
        assert_eq!(terminal(b"CONDUIT_BOOT_STAGE ready\n").unwrap(), None);
        assert_eq!(terminal(b"CONDUIT_KERNEL_SIGN {\"schema\":").unwrap(), None);
    }
}

#[cfg(test)]
mod architecture_profile_tests {
    use super::*;
    #[test]
    fn profiles_preserve_architecture_specific_hardware_and_refuse_other_rows() {
        let (binary, _, args) = emulator_profile("aarch64").unwrap();
        assert_eq!(binary, "qemu-system-aarch64");
        assert_eq!(
            args,
            ["-M", "virt-10.2,highmem=off,gic-version=2,iommu=none"]
        );
        assert!(!args.iter().any(|arg| arg.contains("isa-debug-exit")));
        assert_eq!(emulator_profile("x86_64").unwrap().0, "qemu-system-x86_64");
        assert!(emulator_profile("riscv64").is_err());
        assert!(emulator_profile("aarch64-orange-pi-5").is_err());
    }
}
