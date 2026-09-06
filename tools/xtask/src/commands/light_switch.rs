//! Bounded attached-board entrance for issue #1904.
//!
//! The adapters manifest absolute values only. This command owns the finite
//! switch sequence and atomically admits both writes before releasing either.

use crate::cli::LightSwitchDemoArgs;
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
    process::Command,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

const RECEIPT_LIMIT: usize = 128;
const EVENT_CAPACITY: usize = 16;
const EVENT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug)]
enum DeviceEvent {
    C3(String),
    Pico(String),
    ReadFailure(&'static str, String),
}

#[derive(Serialize)]
struct TransitionReceipt {
    sequence: u8,
    level: bool,
    c3_manifested: bool,
    pico_manifested: bool,
}

#[derive(Serialize)]
struct DemoReceipt {
    schema: &'static str,
    issue: u16,
    form: &'static str,
    initial_level: bool,
    transitions: Vec<TransitionReceipt>,
}

pub fn run(args: LightSwitchDemoArgs) -> Result<(), Box<dyn std::error::Error>> {
    require_device(&args.c3_port, "ESP32-C3")?;
    require_device(&args.pico_link_port, "Pico CDC 0")?;
    require_device(&args.pico_sign_port, "Pico CDC 1")?;
    configure_serial(&args.c3_port, 115_200)?;

    let mut c3 = duplex(&args.c3_port)?;
    let mut pico = duplex(&args.pico_link_port)?;
    let pico_sign = OpenOptions::new().read(true).open(&args.pico_sign_port)?;
    let (events_tx, events_rx) = mpsc::sync_channel(EVENT_CAPACITY);
    spawn_reader("c3", c3.try_clone()?, events_tx.clone(), DeviceEvent::C3);
    spawn_reader("pico", pico_sign, events_tx, DeviceEvent::Pico);

    let mut receipt = DemoReceipt {
        schema: "conduit.demo/physical-light-switch@1",
        issue: 1904,
        form: "proof/fixtures/forms/physical-light-switch.conduit",
        initial_level: false,
        transitions: Vec::with_capacity(usize::from(args.presses) + 1),
    };

    let mut kernel = conduit_std_host::distributed_toggle::PhysicalLightSwitchKernel::prepare()?;
    let (initial_sequence, initial_level) = kernel.initial()?;
    eprintln!("[light-switch] initializing both manifestations OFF");
    apply_transition(
        u8::try_from(initial_sequence)?,
        initial_level,
        &mut c3,
        &mut pico,
        &events_rx,
        &mut receipt,
    )?;
    eprintln!(
        "[light-switch] ready: press the ESP32-C3 BOOT button {} time(s)",
        args.presses
    );
    for sequence in 1..=args.presses {
        wait_for_button(&events_rx)?;
        let (planned_sequence, level) = kernel.press()?;
        if planned_sequence != u64::from(sequence) {
            return Err(format!("planned sequence {planned_sequence} != press {sequence}").into());
        }
        apply_transition(
            sequence,
            level,
            &mut c3,
            &mut pico,
            &events_rx,
            &mut receipt,
        )?;
        eprintln!("[light-switch] transition {sequence}: level={level}");
    }

    let output = Path::new("target/light-switch/physical-receipt.json");
    fs::create_dir_all(output.parent().expect("receipt has parent"))?;
    fs::write(output, serde_json::to_vec_pretty(&receipt)?)?;
    println!("physical light-switch receipt: {}", output.display());
    Ok(())
}

fn require_device(path: &Path, label: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(format!("{label} is absent at {}", path.display()).into());
    }
    Ok(())
}

fn configure_serial(path: &Path, baud: u32) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("stty")
        .args(["-F"])
        .arg(path)
        .args([baud.to_string(), "raw".into(), "-echo".into()])
        .status()?;
    if !status.success() {
        return Err(format!("stty refused {}", path.display()).into());
    }
    Ok(())
}

fn duplex(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).write(true).open(path)
}

fn spawn_reader(
    label: &'static str,
    file: File,
    sender: SyncSender<DeviceEvent>,
    event: fn(String) -> DeviceEvent,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(file);
        loop {
            let mut line = String::with_capacity(RECEIPT_LIMIT);
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = sender.send(DeviceEvent::ReadFailure(label, "EOF".into()));
                    return;
                }
                Ok(_) if line.len() <= RECEIPT_LIMIT => {
                    if sender.send(event(line)).is_err() {
                        return;
                    }
                }
                Ok(_) => {
                    let _ = sender.send(DeviceEvent::ReadFailure(
                        label,
                        "receipt exceeded fixed bound".into(),
                    ));
                    return;
                }
                Err(error) => {
                    let _ = sender.send(DeviceEvent::ReadFailure(label, error.to_string()));
                    return;
                }
            }
        }
    });
}

fn wait_for_button(events: &Receiver<DeviceEvent>) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + EVENT_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(remaining)? {
            DeviceEvent::C3(line) if line.contains("BUTTON transition=pressed") => return Ok(()),
            DeviceEvent::ReadFailure(device, detail) => {
                return Err(format!("{device} receipt stream failed: {detail}").into())
            }
            DeviceEvent::C3(_) | DeviceEvent::Pico(_) => {}
        }
    }
}

fn apply_transition(
    sequence: u8,
    level: bool,
    c3: &mut File,
    pico: &mut File,
    events: &Receiver<DeviceEvent>,
    receipt: &mut DemoReceipt,
) -> Result<(), Box<dyn std::error::Error>> {
    let command = if level { b"1\n" } else { b"0\n" };
    // Both writes are admitted and available before either adapter is allowed
    // to supply success evidence.
    c3.write_all(command)?;
    pico.write_all(command)?;
    c3.flush()?;
    pico.flush()?;

    let expected = if level { "level=true" } else { "level=false" };
    let deadline = Instant::now() + EVENT_TIMEOUT;
    let mut c3_manifested = false;
    let mut pico_manifested = false;
    while !(c3_manifested && pico_manifested) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(remaining)? {
            DeviceEvent::C3(line) => {
                eprintln!("[light-switch] C3: {}", line.trim());
                if line.contains("LED") && line.contains(expected) {
                    c3_manifested = true;
                }
            }
            DeviceEvent::Pico(line) => {
                eprintln!("[light-switch] Pico: {}", line.trim());
                if line.contains("LED") && line.contains(expected) {
                    pico_manifested = true;
                }
            }
            DeviceEvent::ReadFailure(device, detail) => {
                return Err(format!("{device} receipt stream failed: {detail}").into())
            }
        }
    }
    receipt.transitions.push(TransitionReceipt {
        sequence,
        level,
        c3_manifested,
        pico_manifested,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn planned_sequence_is_off_on_off() {
        let mut kernel =
            conduit_std_host::distributed_toggle::PhysicalLightSwitchKernel::prepare().unwrap();
        assert_eq!(kernel.initial().unwrap(), (0, false));
        assert_eq!(kernel.press().unwrap(), (1, true));
        assert_eq!(kernel.press().unwrap(), (2, false));
    }
}
