//! Transitional hosted QEMU acceptance for the normal long-lived zero-Body Crèche arrival.
//!
//! Keep the externally selected `front-door` proof slot stable while the native product
//! entrance is being made genuinely Crèche-first. This proof deliberately does not encode
//! incidental USB/HID initialization ordering. Its contract is product-facing: the normal
//! IMAGE reaches a usable zero-Body Crèche, accepts an explicit Crèche interaction without
//! inventing lifecycle, and remains alive.

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
struct CrecheArrivalProof {
    schema: &'static str,
    base_commit: String,
    image_sha256: String,
    profile: &'static str,
    body: Option<String>,
    creche_ready: bool,
    form_opened: bool,
    naming_edited: bool,
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
    let image = image::execute_architecture_proof(ConduitosArch::X86_64, opts)?;
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
        hid_qmp::send_named_keys(&mut qmp, &mut reader, &["f2"], true, "creche-suggest-name")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_CRECHE_CHECKPOINT edited",
            "creche-name-edit-timeout",
        )?;
        hid_qmp::send_named_keys(&mut qmp, &mut reader, &["f2"], false, "creche-suggest-name")?;
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
                "normal IMAGE exited while interacting with the zero-Body Crèche",
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

    let creche_ready = serial.contains("CONDUIT_BOOT_STAGE front-door-ready")
        && serial.contains("CONDUIT_CRECHE_CHECKPOINT ready");
    if !creche_ready {
        return Err(ConduitosError::refusal(
            "creche-arrival-not-ready",
            "normal IMAGE never established the zero-Body Crèche readiness checkpoint",
        ));
    }
    if !serial.contains("\"body_id\":null") {
        return Err(ConduitosError::refusal(
            "creche-arrival-invented-body",
            "normal zero-Body arrival did not project an explicitly absent Body",
        ));
    }
    if serial.contains("\"status\":\"form-opened\"") {
        return Err(ConduitosError::refusal(
            "creche-arrival-opened-form",
            "normal zero-Body arrival opened a Form before explicit Crèche selection",
        ));
    }
    if !serial.contains("CONDUIT_CRECHE_CHECKPOINT edited") {
        return Err(ConduitosError::refusal(
            "creche-arrival-not-interactive",
            "the Crèche did not acknowledge the explicit naming interaction",
        ));
    }
    if serial.contains("\"status\":\"born-lulled\"")
        || serial.contains("CONDUIT_KERNEL_SIGN")
        || serial.contains("CONDUIT_BOOT_STAGE body-patchbay-open")
    {
        return Err(ConduitosError::refusal(
            "front-door-invented-lifecycle",
            "normal zero-Body interaction entered proof or Body lifecycle machinery",
        ));
    }

    let proof = CrecheArrivalProof {
        schema: "conduit.conduitos/creche-arrival-proof@1",
        base_commit: git_head(&paths.root)?,
        image_sha256: image.iso_sha256,
        profile: super::demo::DEMO_PROFILE,
        body: None,
        creche_ready,
        form_opened: false,
        naming_edited: true,
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
        println!(
            "ConduitOS transitional Crèche arrival proof: {}",
            proof_path.display()
        );
    }
    Ok(())
}
