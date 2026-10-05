//! Boot one admitted proof image and validate its exact guest transcript.
use super::*;
use std::{
    thread,
    time::{Duration, Instant},
};

pub(crate) fn boot_once(paths: &Paths, opts: &GlobalOpts) -> Result<GuestRun, ConduitosError> {
    boot_with_memory(paths, opts, "64M", QEMU_PROFILE)
}

pub(crate) fn boot_configuration(
    paths: &Paths,
    opts: &GlobalOpts,
) -> Result<GuestRun, ConduitosError> {
    boot_with_memory(
        paths,
        opts,
        "512M",
        conduitos::make::USB_CONFIGURATION_QEMU_PROFILE,
    )
}

fn boot_with_memory(
    paths: &Paths,
    opts: &GlobalOpts,
    memory: &str,
    qemu_profile: &str,
) -> Result<GuestRun, ConduitosError> {
    let monitor_socket = paths.target.join("hid-monitor.sock");
    let serial_path = paths.target.join("boot-serial.log");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    let monitor = format!(
        "unix:{},server=on,wait=off",
        monitor_socket.to_string_lossy()
    );
    let serial_target = format!("file:{}", serial_path.to_string_lossy());
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            memory,
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
            &serial_target,
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
            "usb-kbd,bus=conduitos-xhci.0,port=1",
            "-audiodev",
            "none,id=conduitos-opl2-audio",
            "-device",
            "adlib,audiodev=conduitos-opl2-audio",
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
        .map_err(|error| {
            ConduitosError::refusal(
                "missing-qemu",
                format!("cannot launch qemu-system-x86_64: {error}"),
            )
        })?;
    if qemu_profile == conduitos::make::USB_CONFIGURATION_QEMU_PROFILE {
        hid_qmp::inject_configuration(&monitor_socket, &serial_path, &mut child)?;
    } else {
        hid_qmp::inject(&monitor_socket, &serial_path, &mut child)?;
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child.try_wait().map_err(|error| {
            ConduitosError::refusal("qemu-boot-failed", format!("cannot wait for QEMU: {error}"))
        })? {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                child.kill().map_err(|error| {
                    ConduitosError::refusal(
                        "qemu-timeout",
                        format!("cannot stop timed-out QEMU: {error}"),
                    )
                })?;
                let _ = child.wait();
                return Err(ConduitosError::refusal(
                    "qemu-timeout",
                    "QEMU did not emit a terminal debug-exit within 20 seconds",
                ));
            }
        }
    };
    let _output = child.wait_with_output().map_err(|error| {
        ConduitosError::refusal(
            "qemu-boot-failed",
            format!("cannot collect QEMU output: {error}"),
        )
    })?;
    let _ = fs::remove_file(&monitor_socket);
    if status.code() != Some(EXPECTED_QEMU_SUCCESS) {
        let serial = fs::read_to_string(&serial_path).unwrap_or_default();
        let desired_tail_start = serial.len().saturating_sub(360);
        let tail_start = serial
            .char_indices()
            .find_map(|(index, _)| (index >= desired_tail_start).then_some(index))
            .unwrap_or(0);
        return Err(ConduitosError::refusal(
            "qemu-boot-failed",
            format!(
                "expected isa-debug-exit status {EXPECTED_QEMU_SUCCESS}, got {}; serial tail: {}",
                status,
                &serial[tail_start..]
            ),
        ));
    }
    let serial = fs::read_to_string(&serial_path).map_err(|error| {
        ConduitosError::refusal(
            "malformed-boot-sign",
            format!("cannot read serial: {error}"),
        )
    })?;
    let signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_BOOT_SIGN "))
        .collect();
    let xhci_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_XHCI_SIGN "))
        .collect();
    if xhci_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-xhci-sign",
            format!(
                "expected one structured xHCI Sign, found {}",
                xhci_signs.len()
            ),
        ));
    }
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-boot-sign",
            format!("expected one structured boot Sign, found {}", signs.len()),
        ));
    }
    let kernel_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_KERNEL_SIGN "))
        .collect();
    let presentation_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_PRESENTATION_SIGN "))
        .collect();
    let pc_speaker_signs: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_PC_SPEAKER_SIGN "))
        .collect();
    if presentation_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-presentation-sign",
            format!(
                "expected one structured presentation Sign, found {}",
                presentation_signs.len()
            ),
        ));
    }
    if pc_speaker_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-pc-speaker-sign",
            format!(
                "expected one structured PC-speaker Sign, found {}",
                pc_speaker_signs.len()
            ),
        ));
    }
    if kernel_signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-kernel-sign",
            format!(
                "expected one structured kernel Sign, found {}",
                kernel_signs.len()
            ),
        ));
    }
    let observatory_snapshots: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_OBSERVATORY_SNAPSHOT "))
        .collect();
    let keyboard_text_observatory_snapshots: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix(conduitos::keyboard_text_observatory::EXPORT_PREFIX))
        .collect();
    let presentations: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_SERIAL_PRESENT "))
        .collect();
    if presentations.len() != 2
        || presentations[0] != "HELLO, CONDUITOS"
        || presentations[1].as_bytes() != [0; conduit_time::TICK_ENCODED_LEN as usize]
    {
        return Err(ConduitosError::refusal(
            "invalid-serial-presentation",
            format!("expected exact bounded text presentation, found {presentations:?}"),
        ));
    }
    if observatory_snapshots.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-observatory-snapshot",
            format!(
                "expected one ordinary Observatory snapshot, found {}",
                observatory_snapshots.len()
            ),
        ));
    }
    if keyboard_text_observatory_snapshots.len() != 1 {
        return Err(ConduitosError::refusal(
            "malformed-keyboard-text-observatory",
            format!(
                "expected one keyboard-text Observatory snapshot, found {}",
                keyboard_text_observatory_snapshots.len()
            ),
        ));
    }
    let boot: GuestBootSign = serde_json::from_str(signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-boot-sign", error.to_string()))?;
    let xhci: GuestXhciSign = serde_json::from_str(xhci_signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-xhci-sign", error.to_string()))?;
    let usb = usb_run::extract(&serial)?;
    let hid = hid_run::extract(&serial)?;
    let keyboard = keyboard_run::extract(&serial)?;
    let keyboard_text = keyboard_text_run::extract(&serial)?;
    let kernel: GuestKernelSign = serde_json::from_str(kernel_signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-kernel-sign", error.to_string()))?;
    let presentation: GuestPresentationSign =
        serde_json::from_str(presentation_signs[0]).map_err(|error| {
            ConduitosError::refusal("malformed-presentation-sign", error.to_string())
        })?;
    let pc_speaker: GuestPcSpeakerSign = serde_json::from_str(pc_speaker_signs[0])
        .map_err(|error| ConduitosError::refusal("malformed-pc-speaker-sign", error.to_string()))?;
    let observatory: conduit_observatory::ObservatorySnapshot =
        serde_json::from_str(observatory_snapshots[0]).map_err(|error| {
            ConduitosError::refusal("malformed-observatory-snapshot", error.to_string())
        })?;
    let keyboard_text_observatory: conduit_observatory::ObservatorySnapshot =
        serde_json::from_str(keyboard_text_observatory_snapshots[0]).map_err(|error| {
            ConduitosError::refusal("malformed-keyboard-text-observatory", error.to_string())
        })?;
    validate_boot(&boot, qemu_profile)?;
    validate_presentation(&boot, &presentation)?;
    validate_pc_speaker(&boot, &pc_speaker)?;
    validate_xhci(&boot, &xhci)?;
    usb_run::validate(&boot, &xhci, &usb)?;
    hid_run::validate(&boot, &xhci, &usb, &hid)?;
    keyboard_run::validate(&boot, &xhci, &usb, &hid, &keyboard, &observatory)?;
    keyboard_text_run::validate(
        &serial,
        &boot,
        &keyboard,
        &keyboard_text,
        &keyboard_text_observatory,
    )?;
    validate_kernel(&boot, &kernel)?;
    validate_observatory(&boot, &kernel, &presentation, &observatory)?;
    if !opts.quiet && !opts.json {
        println!("{}", signs[0]);
        println!("{}", kernel_signs[0]);
        println!("{}", observatory_snapshots[0]);
    }
    Ok(GuestRun {
        boot,
        presentation,
        pc_speaker,
        xhci,
        usb,
        hid,
        keyboard,
        keyboard_text,
        keyboard_text_observatory,
        kernel,
        observatory,
        serial,
    })
}
