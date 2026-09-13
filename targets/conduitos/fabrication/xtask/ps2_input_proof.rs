//! Emulator proof for bounded i8042 keyboard and pointer product input.

use std::{
    fs,
    process::{Command, Stdio},
};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    hid_qmp, image, journey_input, profile::Paths, qmp, qmp_display, report::git_head,
    ConduitosArch, ConduitosError,
};

#[derive(Serialize)]
struct Proof {
    schema: &'static str,
    base_commit: String,
    image_sha256: String,
    keyboard_body_born: bool,
    keyboard_same_play_result: String,
    pointer_semantics_reached: bool,
    bounded: bool,
    proof_class: &'static str,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-ps2-input-proof",
            "PS/2 input proof requires a real guest and QMP input",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let built = image::execute_ps2_input(ConduitosArch::X86_64, opts)?;
    let socket = paths.target.join("ps2-input-monitor.sock");
    let serial_path = paths.target.join("ps2-input-serial.log");
    let proof_path = paths.target.join("ps2-input-proof.json");
    let frames = paths.target.join("ps2-input-frames");
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
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=2,p3=0",
            "-device",
            "usb-kbd,id=bootstrap-keyboard,bus=conduitos-xhci.0,port=1",
            "-device",
            "usb-mouse,id=bootstrap-pointer,bus=conduitos-xhci.0,port=2",
            "-cdrom",
            paths.iso.to_str().ok_or_else(|| {
                ConduitosError::refusal("ps2-image-path-invalid", "non-UTF-8 ISO path")
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

    let result = (|| {
        let (mut stream, mut reader) = qmp::connect(&socket, &mut child)?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_BOOT_STAGE front-door-ready",
            "ps2-front-door-timeout",
        )?;
        for id in ["bootstrap-keyboard", "bootstrap-pointer"] {
            let request = serde_json::json!({"execute":"device_del","arguments":{"id":id}});
            qmp::request(
                &mut stream,
                &mut reader,
                &serde_json::to_vec(&request).unwrap(),
                "remove-bootstrap-usb-input",
            )?;
        }
        journey_input::key_pair(&mut stream, &mut reader, "ret", "ps2-birth-body")?;
        journey_input::wait_status(&serial_path, &mut child, "quiescent-awaiting-input")?;
        super::journey_standing::type_hello(&mut stream, &mut reader, &serial_path, &mut child)?;
        journey_input::key_pair(&mut stream, &mut reader, "f8", "ps2-stop")?;
        journey_input::wait_status(&serial_path, &mut child, "stopped")?;
        journey_input::key_pair(&mut stream, &mut reader, "f9", "ps2-open-tour")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_TOUR_CHECKPOINT workspace-opened",
            "ps2-tour-timeout",
        )?;
        journey_input::key_pair(&mut stream, &mut reader, "f11", "ps2-open-patchbay")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_BOOT_STAGE pointer-awaiting-report",
            "ps2-pointer-timeout",
        )?;
        journey_input::relative_motion(&mut stream, &mut reader, 8, 8, "ps2-pointer-motion")?;
        hid_qmp::wait_for_stage_count(
            &serial_path,
            &mut child,
            "CONDUIT_BOOT_STAGE pointer-awaiting-report",
            2,
            "ps2-pointer-motion-timeout",
        )?;
        journey_input::primary_button(&mut stream, &mut reader, true, "ps2-pointer-press")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_TOUR_CHECKPOINT transient-pointer-dismissed",
            "ps2-pointer-dismiss-timeout",
        )?;
        journey_input::primary_button(&mut stream, &mut reader, false, "ps2-pointer-release")?;
        hid_qmp::wait_for_stage(
            &serial_path,
            &mut child,
            "CONDUIT_POINTER_SIGN",
            "ps2-pointer-sign-timeout",
        )?;
        let (_, health) =
            qmp_display::capture(&mut stream, &mut reader, &frames, "pointer-routed")?;
        health.map_or(Ok(()), Err)
    })();
    let _ = child.kill();
    let _ = child.wait();
    result?;
    let serial = fs::read_to_string(&serial_path)
        .map_err(|error| ConduitosError::refusal("ps2-serial-unavailable", error.to_string()))?;
    if !serial.contains("CONDUIT_PS2_INPUT_SIGN") || !serial.contains("\"bounded\":true") {
        return Err(ConduitosError::refusal(
            "ps2-ready-sign-absent",
            "guest did not establish bounded PS/2 readiness",
        ));
    }
    if serial
        .lines()
        .any(|line| line.contains("CONDUIT_KERNEL_SIGN") && line.contains("\"status\":\"refused\""))
    {
        return Err(ConduitosError::refusal(
            "ps2-guest-refused-input",
            "guest refused the PS/2 interaction sequence",
        ));
    }
    let records = super::journey_records::decode(&serial)?;
    let result = super::journey_standing::validate(&records)?;
    let proof = Proof {
        schema: "conduit.conduitos.ps2-input-proof/v3",
        base_commit: git_head(&paths.root)?,
        image_sha256: built.iso_sha256,
        keyboard_body_born: serial.contains("\"status\":\"born-lulled\""),
        keyboard_same_play_result: result["result"].as_str().unwrap().into(),
        pointer_semantics_reached: serial.contains("CONDUIT_POINTER_SIGN"),
        bounded: true,
        proof_class: "freestanding-emulator",
    };
    fs::write(&proof_path, serde_json::to_vec_pretty(&proof).unwrap())
        .map_err(|error| ConduitosError::refusal("ps2-proof-unavailable", error.to_string()))?;
    if !opts.quiet && !opts.json {
        println!("ConduitOS PS/2 input proof: {}", proof_path.display());
    }
    Ok(())
}
