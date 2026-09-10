//! Ordinary-image USB keyboard acceptance beyond the former session bound.
mod validation;
use super::{
    hid_qmp, image, journey_input, journey_records, profile::Paths, qemu_artifacts::Artifacts, qmp,
    report::git_head, ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use serde_json::json;
use std::{
    fs,
    process::{Command, Stdio},
};

const CHARACTERS: usize = 160;
const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-keyboard-repeat-proof",
            "a normal image and actual QEMU input are required",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let image = image::execute_architecture_proof(ConduitosArch::X86_64, opts)?;
    let socket = paths.target.join("keyboard-repeat.sock");
    let serial = paths.target.join("keyboard-repeat-serial.log");
    let frames = paths.target.join("keyboard-repeat-frames");
    let _ = fs::remove_file(&socket);
    let _ = fs::remove_file(&serial);
    let monitor_arg = format!("unix:{},server=on,wait=off", socket.display());
    let serial_arg = format!("file:{}", serial.display());
    let mut command = Command::new("qemu-system-x86_64");
    command
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
            &monitor_arg,
            "-serial",
            &serial_arg,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=2,p3=0",
            "-device",
            "usb-kbd,id=conduitos-keyboard,bus=conduitos-xhci.0,port=1",
            "-device",
            "usb-mouse,id=conduitos-pointer,bus=conduitos-xhci.0,port=2",
            "-cdrom",
            paths.iso.to_str().ok_or_else(|| {
                ConduitosError::refusal("keyboard-repeat-image-path", "non-UTF-8 image path")
            })?,
            "-boot",
            "d",
        ])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            fs::File::create(paths.target.join("keyboard-repeat-stderr.log")).map_err(io_error)?,
        );
    let context = json!({
        "source_commit": git_head(&paths.root)?, "image_sha256": image.iso_sha256,
        "qemu_argv": command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect::<Vec<_>>()
    });
    let mut artifacts = Artifacts::new(frames, serial.clone(), context.clone())?;
    let mut child = command
        .spawn()
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let result = (|| {
        let (mut stream, mut reader) = qmp::connect_traced(
            &socket,
            &mut child,
            Some(&paths.target.join("keyboard-repeat-qmp.log")),
        )?;
        hid_qmp::wait_for_stage(
            &serial,
            &mut child,
            "CONDUIT_BOOT_STAGE front-door-ready",
            "keyboard-repeat-arrival-timeout",
        )?;
        artifacts.capture(&mut stream, &mut reader, "creche-ready", false)?;
        journey_input::key_pair(&mut stream, &mut reader, "ret", "keyboard-repeat-birth")?;
        journey_input::wait_status(&serial, &mut child, "playing")?;
        for index in 0..CHARACTERS {
            let key = char::from(ALPHABET[index % ALPHABET.len()]).to_string();
            journey_input::key_pair(&mut stream, &mut reader, &key, "keyboard-repeat-input")?;
            let expected = validation::expected_text(index + 1);
            journey_input::wait_for_record(
                &serial,
                &mut child,
                "keyboard-repeat-input-timeout",
                &format!("character {}", index + 1),
                |text| {
                    Ok(journey_records::decode(text)?.last().is_some_and(|record| {
                        record["status"] == "playing"
                            && record["input_count"] == ((index + 1) * 2) as u64
                            && record["result"] == expected
                    }))
                },
            )?;
            if index == 39 {
                artifacts.capture(&mut stream, &mut reader, "beyond-former-report-limit", true)?;
            }
        }
        artifacts.capture(&mut stream, &mut reader, "repeated-input", true)?;
        journey_input::key_pair(&mut stream, &mut reader, "f8", "keyboard-repeat-stop")?;
        journey_input::wait_status(&serial, &mut child, "stopped")?;
        journey_input::key_pair(&mut stream, &mut reader, "f7", "keyboard-repeat-lull")?;
        journey_input::wait_status(&serial, &mut child, "lulled")?;
        artifacts.capture(&mut stream, &mut reader, "body-lulled", true)?;
        let text = fs::read_to_string(&serial).map_err(io_error)?;
        let evidence = validation::validate(&text)?;
        if child.try_wait().map_err(io_error)?.is_some() {
            return Err(ConduitosError::refusal(
                "keyboard-repeat-guest-exited",
                "normal product must remain alive after Lull",
            ));
        }
        let proof = json!({
            "schema": "conduit.conduitos/keyboard-repeat-proof@1", "proof_class": "freestanding-emulator",
            "context": context, "evidence": evidence, "remained_alive": true,
            "characters": CHARACTERS, "transitions": CHARACTERS * 2
        });
        fs::write(
            paths.target.join("keyboard-repeat-proof.json"),
            serde_json::to_vec_pretty(&proof).map_err(|error| {
                ConduitosError::refusal("keyboard-repeat-proof-json", error.to_string())
            })?,
        )
        .map_err(io_error)?;
        Ok(())
    })();
    if result.is_err() && child.try_wait().ok().flatten().is_none() {
        match qmp::connect(&socket, &mut child).and_then(|(mut stream, mut reader)| {
            artifacts.capture(&mut stream, &mut reader, "failure", false)
        }) {
            Ok(()) => {}
            Err(error) => artifacts.diagnostic_failure(&error),
        }
    }
    if let Some(status) = child.try_wait().map_err(io_error)? {
        artifacts.stopped(&status, "guest-exited");
    } else {
        child.kill().map_err(io_error)?;
        let status = child.wait().map_err(io_error)?;
        artifacts.stopped(&status, "harness-kill-after-keyboard-repeat-proof");
    }
    if let Err(error) = artifacts.finish(result.as_ref().err()) {
        if result.is_ok() {
            return Err(error);
        }
        eprintln!("keyboard repeat artifacts: {error}");
    }
    result?;
    if !opts.quiet && !opts.json {
        println!(
            "ConduitOS keyboard repeat proof: {}",
            paths.target.join("keyboard-repeat-proof.json").display()
        );
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("keyboard-repeat-proof-io", error.to_string())
}
