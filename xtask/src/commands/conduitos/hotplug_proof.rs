//! Typed repository entrance for the real ConduitOS keyboard hotplug proof.

use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    hotplug_qmp, image,
    profile::{Paths, EXPECTED_QEMU_SUCCESS, QEMU_PROFILE},
    report::git_head,
    ConduitosArch, ConduitosError,
};

const NEGATIVE_CASES: &[&str] = &[
    "stale-d1-completion-cannot-cross-d2-epoch",
    "d1-identity-cannot-be-reused",
    "p1-cannot-start-against-d2-offer",
    "device-loss-during-play-is-not-success-or-closure",
    "device-loss-fabricates-no-semantic-release",
    "non-keyboard-and-hid-setup-failure-publish-no-offer",
    "form-carries-no-usb-hid-or-authority-facts",
];

#[derive(Serialize)]
struct HotplugRecord {
    schema: &'static str,
    base_commit: String,
    proof_class: &'static str,
    qemu_profile: &'static str,
    initial_device: &'static str,
    sign: serde_json::Value,
    deterministic_negative_cases: &'static [&'static str],
    ordinary_planning: bool,
    production_kernel: bool,
    patchbay_projection_contract: bool,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-hotplug-proof",
            "hotplug proof requires real QMP and xHCI observations",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    image::execute_hotplug(ConduitosArch::X86_64, opts)?;
    let socket = paths.target.join("hotplug-monitor.sock");
    let serial_path = paths.target.join("hotplug-serial.log");
    let _ = fs::remove_file(&socket);
    let _ = fs::remove_file(&serial_path);
    let monitor = format!("unix:{},server=on,wait=off", socket.to_string_lossy());
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
            "-rtc",
            "base=2026-08-09T00:00:00,clock=vm",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
            "-device",
            "usb-kbd,id=keyboard-d1,bus=conduitos-xhci.0,port=1",
            "-cdrom",
            paths.iso.to_str().unwrap(),
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    hotplug_qmp::execute(&socket, &serial_path, &mut child)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child
            .try_wait()
            .map_err(|error| ConduitosError::refusal("qemu-hotplug-failed", error.to_string()))?
        {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                return super::hid_qmp::stop(
                    &mut child,
                    "hotplug-timeout",
                    "guest did not terminate".into(),
                )
            }
        }
    };
    if status.code() != Some(EXPECTED_QEMU_SUCCESS) {
        let serial = fs::read_to_string(&serial_path).unwrap_or_default();
        return Err(ConduitosError::refusal(
            "hotplug-guest-refused",
            format!("status {status}; serial: {serial}"),
        ));
    }
    let serial = fs::read_to_string(&serial_path)
        .map_err(|error| ConduitosError::refusal("hotplug-sign-missing", error.to_string()))?;
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_HOTPLUG_SIGN "))
        .collect();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "hotplug-sign-missing",
            format!("found {} signs", signs.len()),
        ));
    }
    let sign: serde_json::Value = serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("hotplug-sign-invalid", error.to_string()))?;
    for exact_true in [
        "p1_immutable",
        "same_form",
        "same_host",
        "same_boot",
        "stale_plan_refused",
        "semantic_topology_stable",
        "completed",
    ] {
        if sign.get(exact_true) != Some(&serde_json::Value::Bool(true)) {
            return Err(ConduitosError::refusal(
                "hotplug-sign-invalid",
                format!("{exact_true} was not true"),
            ));
        }
    }
    if sign["d1_device_id"] == sign["d2_device_id"]
        || sign["p1_plan_id"] == sign["p2_plan_id"]
        || sign["x_terminal"] != "failed-device-removed"
        || sign["fabricated_semantic_events"] != 0
    {
        return Err(ConduitosError::refusal(
            "hotplug-sign-invalid",
            sign.to_string(),
        ));
    }
    let record = HotplugRecord {
        schema: "conduit.conduitos.hotplug-proof/v1",
        base_commit: git_head(&paths.root)?,
        proof_class: "freestanding-emulator",
        qemu_profile: QEMU_PROFILE,
        initial_device: "usb-kbd,id=keyboard-d1,bus=conduitos-xhci.0,port=1",
        sign,
        deterministic_negative_cases: NEGATIVE_CASES,
        ordinary_planning: true,
        production_kernel: true,
        patchbay_projection_contract: true,
    };
    fs::write(
        &paths.hotplug_proof,
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .map_err(|error| ConduitosError::refusal("hotplug-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&record).unwrap());
    } else if !opts.quiet {
        println!("ConduitOS hotplug proof: {}", paths.hotplug_proof.display());
    }
    Ok(())
}
