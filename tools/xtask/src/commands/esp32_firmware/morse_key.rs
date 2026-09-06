//! Attended, bounded ESP32-C3 BOOT-button Morse-key proof.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

use clap::Args;
use conduit_host_esp32_fabrication::Esp32FamilyTarget;
use conduit_text::{MorseKeyInterpreter, MorseKeyPhase, MorseKeyTransition, MorsePattern};
use serde::Serialize;

use crate::{cli::GlobalOpts, workspace::workspace_root};

const ISSUE: u16 = 1918;
const READY_PREFIX: &str = "CONDUIT_LIGHT_SWITCH_READY";
const TRANSITION_PREFIX: &str = "CONDUIT_LIGHT_SWITCH_BUTTON";
const PLAYBACK_COMPLETE: &str = "CONDUIT_MORSE_KEY_PLAYBACK outcome=completed final-led=false";
const MAXIMUM_LINE_BYTES: usize = 256;
const EVENT_CAPACITY: usize = 32;
const FIRMWARE_PATTERN_BYTES: usize = 64;

#[derive(Args, Debug)]
pub(super) struct MorseKeyArgs {
    /// Stable USB-UART path for the inspected ESP32-C3 DevKitM-1.
    #[arg(
        long,
        default_value = "/dev/serial/by-id/usb-Silicon_Labs_CP2102N_USB_to_UART_Bridge_Controller_dcf8355da19ded11a7205f84e259fb3e-if00-port0"
    )]
    port: PathBuf,
    /// Exact CP2102N serial owned by the C3 fabrication descriptor.
    #[arg(long, default_value = "dcf8355da19ded11a7205f84e259fb3e")]
    confirm_serial: String,
    /// Planned Morse unit in milliseconds; dot=1, dash=3, letter gap=3, word gap=7.
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u16).range(40..=2_000))]
    unit_ms: u16,
    /// Exact attended text expected from the bounded transition capture.
    #[arg(long, default_value = "SOS")]
    expected: String,
    /// Whole-capture deadline, including the operator's physical keying.
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=300))]
    timeout_seconds: u64,
    #[arg(long, default_value = "target/esp32-firmware/morse-key-physical.json")]
    receipt: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
struct TransitionReceipt {
    phase: &'static str,
    sequence: u64,
    monotonic_micros: u64,
}

#[derive(Debug, Serialize)]
struct MorseKeyReceipt {
    schema: &'static str,
    issue: u16,
    outcome: &'static str,
    proof_class: &'static str,
    source_sha: String,
    target: &'static str,
    serial_path: String,
    usb_serial: String,
    firmware_artifact: String,
    firmware_sha256: String,
    flashing_tool_sha256: String,
    boot_identity: String,
    clock_basis: String,
    planned_unit_ms: u16,
    admitted_transitions: usize,
    transitions: Vec<TransitionReceipt>,
    canonical_morse_hex: String,
    decoded_text: String,
    expected_text: String,
    playback: &'static str,
    final_led: bool,
}

pub(super) fn run(args: MorseKeyArgs, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let expected = MorsePattern::from_text(&args.expected, args.unit_ms)
        .map_err(|error| format!("expected Morse text is not admitted: {error:?}"))?;
    let expected_bytes = expected
        .encode()
        .map_err(|error| format!("expected Morse pattern is not encodable: {error:?}"))?;
    if expected_bytes.len() > FIRMWARE_PATTERN_BYTES {
        return Err(format!(
            "expected pattern uses {} bytes; C3 playback admits {FIRMWARE_PATTERN_BYTES}",
            expected_bytes.len()
        )
        .into());
    }

    let root = workspace_root()?;
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would build, flash, capture, decode, and play back {:?} on {}",
                args.expected,
                args.port.display()
            );
        }
        return Ok(());
    }
    require_clean(&root)?;
    let facts = Esp32FamilyTarget::C3.facts();
    if args.confirm_serial != facts.usb_serial {
        return Err(format!(
            "C3 confirmation mismatch: expected {}, received {}",
            facts.usb_serial, args.confirm_serial
        )
        .into());
    }
    let observed = super::serial_properties(&args.port)?;
    if observed.get("ID_SERIAL_SHORT").map(String::as_str) != Some(facts.usb_serial) {
        return Err(format!(
            "attached serial identity at {} is not the planned C3",
            args.port.display()
        )
        .into());
    }

    let artifact = build_firmware(&root, opts.locked)?;
    let firmware_sha256 = super::sha256_file(&artifact)?;
    let tool = super::provision_espflash(&root)?;
    let mut flash = Command::new(&tool);
    flash
        .arg("flash")
        .args(["--chip", facts.espflash_chip, "--port"])
        .arg(&args.port)
        .args(["--non-interactive", "--skip-update-check"])
        .arg(&artifact);
    super::require_success(flash, "verified C3 Morse-key flash")?;
    wait_for_port(&args.port, Duration::from_secs(10))?;
    configure_serial(&args.port)?;

    let mut device = OpenOptions::new().read(true).write(true).open(&args.port)?;
    let (sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
    spawn_reader(device.try_clone()?, sender);
    device.write_all(b"?\n")?;
    device.flush()?;
    let deadline = Instant::now() + Duration::from_secs(args.timeout_seconds);
    let boot_identity = wait_for_ready(&events, deadline)?;
    let clock_basis = format!(
        "esp32-c3/{}/boot-{boot_identity}/monotonic-us@1",
        facts.usb_serial
    );

    let transition_count = expected
        .segments
        .len()
        .checked_add(1)
        .ok_or("Morse transition bound overflow")?;
    let maximum_transitions = u16::try_from(transition_count)?;
    let mut interpreter =
        MorseKeyInterpreter::new(clock_basis.clone(), args.unit_ms, maximum_transitions)
            .map_err(|error| format!("cannot admit Morse-key interpreter: {error:?}"))?;
    eprintln!(
        "[morse-key] type {} on the C3 BOOT button: dot≈{}ms dash≈{}ms letter-gap≈{}ms",
        args.expected,
        args.unit_ms,
        u32::from(args.unit_ms) * 3,
        u32::from(args.unit_ms) * 3
    );
    let mut transitions = Vec::with_capacity(transition_count);
    while transitions.len() < transition_count {
        let line = receive(&events, deadline)?;
        let Some(raw) = parse_transition(&line)? else {
            continue;
        };
        let transition = MorseKeyTransition {
            clock_basis: clock_basis.clone(),
            monotonic_micros: raw.monotonic_micros,
            phase: if raw.phase == "pressed" {
                MorseKeyPhase::Pressed
            } else {
                MorseKeyPhase::Released
            },
            sequence: raw.sequence,
        };
        interpreter
            .accept(&transition)
            .map_err(|error| format!("physical Morse transition refused: {error:?}"))?;
        eprintln!(
            "[morse-key] {} sequence={} tick={}us",
            raw.phase, raw.sequence, raw.monotonic_micros
        );
        transitions.push(raw);
    }
    let observed_pattern = interpreter
        .finish()
        .map_err(|error| format!("physical Morse capture refused: {error:?}"))?;
    let observed_bytes = observed_pattern
        .encode()
        .map_err(|error| format!("physical Morse capture omitted canonical encoding: {error:?}"))?;
    let decoded_text = observed_pattern
        .to_text()
        .map_err(|error| format!("physical Morse capture did not decode: {error:?}"))?;
    if observed_bytes != expected_bytes || decoded_text != args.expected {
        return Err(format!(
            "physical Morse mismatch: expected {:?}, decoded {:?}",
            args.expected, decoded_text
        )
        .into());
    }

    let canonical_morse_hex = hex(&observed_bytes);
    let command = format!("M{canonical_morse_hex}\n");
    device.write_all(command.as_bytes())?;
    device.flush()?;
    wait_for_playback(&events, deadline)?;

    let receipt = MorseKeyReceipt {
        schema: "conduit.esp32/morse-key-physical@1",
        issue: ISSUE,
        outcome: "completed",
        proof_class: "attended-physical-button-and-led",
        source_sha: super::git_head(&root)?,
        target: "esp32-c3/devkitm-1",
        serial_path: args.port.display().to_string(),
        usb_serial: facts.usb_serial.into(),
        firmware_artifact: super::relative(&root, &artifact)?,
        firmware_sha256,
        flashing_tool_sha256: super::sha256_file(&tool)?,
        boot_identity,
        clock_basis,
        planned_unit_ms: args.unit_ms,
        admitted_transitions: transition_count,
        transitions,
        canonical_morse_hex,
        decoded_text,
        expected_text: args.expected,
        playback: "completed",
        final_led: false,
    };
    super::write_receipt(&root.join(args.receipt), &receipt, opts)
}

fn build_firmware(root: &Path, locked: bool) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let facts = Esp32FamilyTarget::C3.facts();
    let package = root.join(facts.package_dir);
    let artifact = package
        .join("target")
        .join(facts.cargo_target)
        .join("release")
        .join("conduit-esp32-c3-light-switch");
    let mut command = Command::new("cargo");
    command
        .current_dir(&package)
        .arg(format!("+{}", facts.rust_toolchain))
        .args([
            "build",
            "--release",
            "--features",
            "light-switch",
            "--bin",
            "conduit-esp32-c3-light-switch",
        ]);
    if locked {
        command.arg("--locked");
    }
    super::require_success(command, "C3 Morse-key firmware build")?;
    if !artifact.is_file() {
        return Err(format!("C3 Morse-key build omitted {}", artifact.display()).into());
    }
    Ok(artifact)
}

fn require_clean(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    if !output.status.success() || !output.stdout.is_empty() {
        return Err("physical Morse proof requires one committed clean source head".into());
    }
    Ok(())
}

fn wait_for_port(path: &Path, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    while !path.exists() {
        if Instant::now() >= deadline {
            return Err(format!("C3 serial path {} did not re-enumerate", path.display()).into());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn configure_serial(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new("stty");
    command
        .args(["-F"])
        .arg(path)
        .args(["115200", "raw", "-echo", "min", "1", "time", "0"]);
    super::require_success(command, "C3 Morse-key serial configuration")
}

fn spawn_reader(file: File, sender: SyncSender<Result<String, String>>) {
    thread::spawn(move || {
        let mut file = file;
        let mut line = [0_u8; MAXIMUM_LINE_BYTES];
        let mut length = 0_usize;
        loop {
            let mut byte = [0_u8; 1];
            match file.read(&mut byte) {
                Ok(0) => {
                    let _ = sender.send(Err("C3 serial stream reached EOF".into()));
                    return;
                }
                Ok(_) if byte[0] == b'\n' => {
                    let value = String::from_utf8_lossy(&line[..length]).trim().to_owned();
                    length = 0;
                    if sender.send(Ok(value)).is_err() {
                        return;
                    }
                }
                Ok(_) if length < line.len() => {
                    line[length] = byte[0];
                    length += 1;
                }
                Ok(_) => {
                    let _ = sender.send(Err("C3 receipt exceeded the admitted line bound".into()));
                    return;
                }
                Err(error) => {
                    let _ = sender.send(Err(format!("C3 serial read failed: {error}")));
                    return;
                }
            }
        }
    });
}

fn receive(
    events: &Receiver<Result<String, String>>,
    deadline: Instant,
) -> Result<String, Box<dyn std::error::Error>> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    events
        .recv_timeout(remaining)
        .map_err(|error| format!("C3 Morse-key capture timed out: {error}"))?
        .map_err(Into::into)
}

fn wait_for_ready(
    events: &Receiver<Result<String, String>>,
    deadline: Instant,
) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        let line = receive(events, deadline)?;
        if line.starts_with(READY_PREFIX)
            && field(&line, "transitions") == Some("pressed-released")
            && field(&line, "clock") == Some("boot-monotonic-us@1")
        {
            let boot = field(&line, "boot").ok_or("C3 ready receipt omitted Boot identity")?;
            if boot.len() == 16
                && boot != "0000000000000000"
                && boot.bytes().all(|value| value.is_ascii_hexdigit())
            {
                return Ok(boot.to_owned());
            }
            return Err("C3 ready receipt carried a malformed Boot identity".into());
        }
    }
}

fn parse_transition(line: &str) -> Result<Option<TransitionReceipt>, Box<dyn std::error::Error>> {
    if !line.starts_with(TRANSITION_PREFIX) {
        return Ok(None);
    }
    let phase = match field(line, "transition") {
        Some("pressed") => "pressed",
        Some("released") => "released",
        _ => return Err("C3 transition carried an unsupported phase".into()),
    };
    Ok(Some(TransitionReceipt {
        phase,
        sequence: field(line, "sequence")
            .ok_or("C3 transition omitted sequence")?
            .parse()?,
        monotonic_micros: field(line, "monotonic-us")
            .ok_or("C3 transition omitted monotonic time")?
            .parse()?,
    }))
}

fn wait_for_playback(
    events: &Receiver<Result<String, String>>,
    deadline: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let line = receive(events, deadline)?;
        if line == PLAYBACK_COMPLETE {
            return Ok(());
        }
        if line.starts_with("CONDUIT_MORSE_KEY_PLAYBACK outcome=refused") {
            return Err(format!("C3 refused canonical Morse playback: {line}").into());
        }
    }
}

fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    line.split_ascii_whitespace()
        .find_map(|part| part.strip_prefix(&prefix))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("bounded String formatting cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_ready_and_transition_receipts_parse_without_device_vocabulary_leaking_upward() {
        let ready = "CONDUIT_LIGHT_SWITCH_READY host=esp32-c3/devkitm-1 boot=0123456789abcdef button=gpio9 led=gpio8 transitions=pressed-released clock=boot-monotonic-us@1";
        assert_eq!(field(ready, "boot"), Some("0123456789abcdef"));
        let pressed = parse_transition(
            "CONDUIT_LIGHT_SWITCH_BUTTON transition=pressed sequence=0 monotonic-us=123456",
        )
        .unwrap()
        .unwrap();
        assert_eq!(pressed.phase, "pressed");
        assert_eq!(pressed.sequence, 0);
        assert_eq!(pressed.monotonic_micros, 123_456);
    }

    #[test]
    fn malformed_transition_fields_and_playback_bound_fail_closed() {
        assert!(parse_transition("unrelated").unwrap().is_none());
        assert!(parse_transition("CONDUIT_LIGHT_SWITCH_BUTTON transition=held").is_err());
        let maximum = MorsePattern::from_text("SOS", 200)
            .unwrap()
            .encode()
            .unwrap();
        assert!(maximum.len() <= FIRMWARE_PATTERN_BYTES);
        assert_eq!(hex(&[0, 15, 16, 255]), "000f10ff");
    }
}
