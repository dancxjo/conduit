//! Hosted QEMU acceptance for the normal long-lived zero-Body product entrance.

use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{hid_qmp, image, profile::Paths, report::git_head, ConduitosArch, ConduitosError};

#[derive(Serialize)]
struct FrontDoorProof {
    schema: &'static str,
    base_commit: String,
    image_sha256: String,
    profile: &'static str,
    body: Option<String>,
    seed_opened: bool,
    details_opened: bool,
    effects: u8,
    remained_alive: bool,
    stopped_by_harness: bool,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-front-door-proof",
            "front-door proof requires a real normal IMAGE and QEMU lifecycle",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let image = image::execute(ConduitosArch::X86_64, opts)?;
    let monitor_socket = paths.target.join("front-door-monitor.sock");
    let serial_path = paths.target.join("front-door-serial.log");
    let proof_path = paths.target.join("front-door-proof.json");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    let monitor = format!(
        "unix:{},server=on,wait=off",
        monitor_socket.to_string_lossy()
    );
    let serial = format!("file:{}", serial_path.to_string_lossy());
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "64M",
            "-smp",
            "1",
            "-display",
            "none",
            "-vga",
            "std",
            "-monitor",
            "none",
            "-qmp",
            &monitor,
            "-serial",
            &serial,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
            "-device",
            "usb-kbd,bus=conduitos-xhci.0,port=1",
            "-cdrom",
            paths.iso.to_str().ok_or_else(|| {
                ConduitosError::refusal("front-door-image-path-invalid", "non-UTF-8 ISO path")
            })?,
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;

    let interaction = (|| {
        let (mut qmp, mut reader) = hid_qmp::connect(&monitor_socket, &mut child)?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_BOOT_STAGE front-door-ready",
            "front-door-ready-timeout",
        )?;
        hid_qmp::send_named_keys(&mut qmp, &mut reader, &["ret"], true, "front-door-open")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "\"status\":\"seed-opened\"",
            "front-door-open-timeout",
        )?;
        hid_qmp::send_named_keys(&mut qmp, &mut reader, &["ret"], false, "front-door-open")?;
        hid_qmp::send_named_keys(&mut qmp, &mut reader, &["f2"], true, "front-door-details")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "\"status\":\"details-opened\"",
            "front-door-details-timeout",
        )?;
        hid_qmp::send_named_keys(&mut qmp, &mut reader, &["f2"], false, "front-door-details")?;
        thread::sleep(Duration::from_millis(250));
        if child
            .try_wait()
            .map_err(|error| {
                ConduitosError::refusal("front-door-qemu-wait-failed", error.to_string())
            })?
            .is_some()
        {
            return Err(ConduitosError::refusal(
                "front-door-not-long-lived",
                "normal IMAGE exited after inert OPEN/DETAILS interaction",
            ));
        }
        Ok(())
    })();
    if interaction.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return interaction;
    }
    child.kill().map_err(|error| {
        ConduitosError::refusal("front-door-qemu-stop-failed", error.to_string())
    })?;
    let _ = child.wait();
    let serial = fs::read_to_string(&serial_path).map_err(|error| {
        ConduitosError::refusal("front-door-serial-unavailable", error.to_string())
    })?;
    let ready = serial.find("CONDUIT_BOOT_STAGE front-door-ready");
    let first_report = serial.find("CONDUIT_BOOT_STAGE hid-release-report");
    if ready.is_none() || first_report.is_none() || ready >= first_report {
        return Err(ConduitosError::refusal(
            "front-door-waited-for-input",
            "zero-Body WORLD was not ready before the first ordinary HID report",
        ));
    }
    if serial.contains("body-born")
        || serial.contains("CONDUIT_KERNEL_SIGN")
        || serial.contains("CONDUIT_BOOT_STAGE body-patchbay-open")
    {
        return Err(ConduitosError::refusal(
            "front-door-invented-lifecycle",
            "normal zero-Body interaction entered proof or Body lifecycle machinery",
        ));
    }
    let proof = FrontDoorProof {
        schema: "conduit.conduitos/front-door-proof@1",
        base_commit: git_head(&paths.root)?,
        image_sha256: image.iso_sha256,
        profile: super::demo::DEMO_PROFILE,
        body: None,
        seed_opened: serial.contains("\"status\":\"seed-opened\""),
        details_opened: serial.contains("\"status\":\"details-opened\""),
        effects: 0,
        remained_alive: true,
        stopped_by_harness: true,
    };
    let encoded = serde_json::to_vec_pretty(&proof)
        .map_err(|error| ConduitosError::refusal("front-door-proof-invalid", error.to_string()))?;
    fs::write(&proof_path, encoded).map_err(|error| {
        ConduitosError::refusal("front-door-proof-unavailable", error.to_string())
    })?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS front-door proof: {}", proof_path.display());
    }
    Ok(())
}
