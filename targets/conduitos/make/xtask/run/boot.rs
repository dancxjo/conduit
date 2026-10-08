//! Boot one admitted proof image and validate its exact guest transcript.
use super::*;
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

#[derive(serde::Serialize)]
pub(crate) struct QemuWavCapture {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub nonzero_samples: usize,
    pub source: &'static str,
}

pub(crate) fn inspect_wav(path: &Path) -> Result<QemuWavCapture, ConduitosError> {
    const MAX_WAV_BYTES: u64 = 32 * 1024 * 1024;
    let size = fs::metadata(path)
        .map_err(|error| ConduitosError::refusal("qemu-audio-missing", error.to_string()))?
        .len();
    if !(48..=MAX_WAV_BYTES).contains(&size) {
        return Err(ConduitosError::refusal(
            "qemu-audio-size-invalid",
            format!("QEMU WAV has {size} bytes; expected 48..={MAX_WAV_BYTES}"),
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| ConduitosError::refusal("qemu-audio-read-failed", error.to_string()))?;
    let word = |start| u16::from_le_bytes([bytes[start], bytes[start + 1]]);
    let dword = |start| u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap());
    if &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || dword(16) != 16
        || word(20) != 1
        || &bytes[36..40] != b"data"
        || dword(4) as u64 + 8 != size
        || dword(40) as u64 + 44 != size
        || size % 4 != 0
        || word(22) != 2
        || word(34) != 16
        || dword(24) != 44_100
        || dword(28) != 176_400
        || word(32) != 4
    {
        return Err(ConduitosError::refusal(
            "qemu-audio-format-invalid",
            "QEMU WAV must be exact RIFF PCM16 stereo 44.1 kHz",
        ));
    }
    let nonzero_samples = bytes[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .filter(|sample| sample[0] != 0 || sample[1] != 0)
        .count();
    if nonzero_samples == 0 {
        return Err(ConduitosError::refusal(
            "qemu-audio-silent",
            "QEMU output contained no audible PCM samples",
        ));
    }
    Ok(QemuWavCapture {
        path: path.display().to_string(),
        sha256: super::super::report::sha256_file(path)?,
        bytes: size,
        sample_rate_hz: 44_100,
        channels: 2,
        nonzero_samples,
        source: "same-run-qemu-wav-output",
    })
}

pub(crate) fn boot_once(paths: &Paths, opts: &GlobalOpts) -> Result<GuestRun, ConduitosError> {
    boot_with_memory(paths, opts, "64M", QEMU_PROFILE, 16 * 1024 * 1024, None)
}

/// Record the actual QEMU mixer output of this boot. The caller owns the WAV
/// path and must keep it with the returned boot/Plan/Play identities.
pub(crate) fn boot_once_with_audio(
    paths: &Paths,
    opts: &GlobalOpts,
    wav: &std::path::Path,
) -> Result<GuestRun, ConduitosError> {
    boot_with_memory(
        paths,
        opts,
        "64M",
        QEMU_PROFILE,
        16 * 1024 * 1024,
        Some(wav),
    )
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
        conduitos::make::USB_CONFIGURATION_ARENA_BYTES,
        None,
    )
}

fn boot_with_memory(
    paths: &Paths,
    opts: &GlobalOpts,
    memory: &str,
    qemu_profile: &str,
    arena_bytes: u64,
    wav: Option<&std::path::Path>,
) -> Result<GuestRun, ConduitosError> {
    let monitor_endpoint = super::monitor_socket::MonitorSocket::new()?;
    let monitor_socket = monitor_endpoint.path();
    let serial_path = paths.target.join("boot-serial.log");
    let _ = fs::remove_file(&serial_path);
    if let Some(wav) = wav {
        if wav.exists() {
            return Err(ConduitosError::refusal(
                "qemu-audio-output-exists",
                format!("audio capture must be new: {}", wav.display()),
            ));
        }
        if wav.to_str().is_none_or(|path| path.contains(',')) {
            return Err(ConduitosError::refusal(
                "qemu-audio-output-path-invalid",
                "QEMU WAV output path must be UTF-8 without option separators",
            ));
        }
    }
    let monitor = format!(
        "unix:{},server=on,wait=off",
        monitor_socket.to_string_lossy()
    );
    let serial_target = format!("file:{}", serial_path.to_string_lossy());
    let mut command = Command::new("qemu-system-x86_64");
    command.args([
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
    ]);
    let mut child = command
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
    let injected = if qemu_profile == conduitos::make::USB_CONFIGURATION_QEMU_PROFILE {
        hid_qmp::inject_configuration(monitor_socket, &serial_path, &mut child)
    } else if let Some(wav) = wav {
        hid_qmp::inject_with_capture(monitor_socket, &serial_path, &mut child, wav)
    } else {
        hid_qmp::inject(monitor_socket, &serial_path, &mut child)
    };
    if let Err(error) = injected {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
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
    validate_boot(&boot, qemu_profile, arena_bytes)?;
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
    let domain_cost = super::super::protected_product_receipt::capture(
        &serial,
        &serde_json::json!({"ordinary_plan_id": kernel.plan_id,
            "ordinary_play_id": kernel.active_play_id}),
        "x86_64",
    )?;
    validate_observatory(&boot, &kernel, &presentation, &domain_cost, &observatory)?;
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
