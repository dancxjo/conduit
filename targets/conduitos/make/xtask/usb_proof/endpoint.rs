//! Dedicated raw endpoint emulator proof; does not establish class acceptance.
use super::super::{
    image,
    profile::{Paths, EXPECTED_QEMU_SUCCESS},
    qmp,
    report::{git_head, GuestBootSign, GuestUsbSign, GuestXhciSign},
    run, usb_run, ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use conduit_core::bind_active_play;
use conduitos::usb_base::{
    endpoint_read_contract::EndpointReadContract,
    endpoint_read_proof_plan::{self, EndpointReadProofSubject},
    endpoint_read_result::PreparedEndpointReadResultEncoder,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

mod boot_selection;
mod hid;
mod identity;
mod mode;
use mode::ProofMode;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointSign {
    schema: String,
    proof_class: String,
    status: String,
    source_document_id: String,
    checked_plot_id: String,
    plan_id: String,
    active_play_id: String,
    device_instance_id: String,
    attachment_epoch: u32,
    endpoint_epoch: u64,
    endpoint_dci: u8,
    transfers: u16,
    cycle_transitions: u16,
    transcript_digest: String,
    normal_close: bool,
    acknowledged_stop: bool,
    fixture_protocol: bool,
}

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    execute_mode(opts, ProofMode::Raw)
}
pub(super) fn execute_hid(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    execute_mode(opts, ProofMode::Keyboard)
}
pub(super) fn execute_mouse(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    execute_mode(opts, ProofMode::Mouse)
}
fn execute_mode(opts: &GlobalOpts, mode: ProofMode) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal(
            "dry-run-has-no-proof",
            "endpoint proof needs actual device execution",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    if mode.entry().is_some() {
        image::execute_usb_hid_endpoint(opts, mode == ProofMode::Mouse)?;
    } else {
        image::execute_usb_endpoint(opts)?;
    }
    let socket = paths.target.join(match mode {
        ProofMode::Raw => "usb-endpoint-monitor.sock",
        ProofMode::Keyboard => "hid.sock",
        ProofMode::Mouse => "mouse.sock",
    });
    let serial_path = paths.target.join(mode.serial_name());
    for path in [&socket, &serial_path] {
        if path.exists() {
            fs::remove_file(path).map_err(|e| refusal("endpoint-proof-path", e.to_string()))?;
        }
    }
    let monitor = format!("unix:{},server=on,wait=off", socket.display());
    let serial = format!("file:{}", serial_path.display());
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
            mode.device(),
            "-cdrom",
            paths
                .iso
                .to_str()
                .ok_or_else(|| refusal("endpoint-proof-image", "non-UTF8 image path"))?,
            "-boot",
            "d",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| refusal("endpoint-proof-qemu", e.to_string()))?;
    let outcome = drive(&socket, &serial_path, &mut child, mode);
    if outcome.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    outcome?;
    let serial = read_serial(&serial_path)?;
    let boot: GuestBootSign = extract(&serial, "CONDUIT_BOOT_SIGN ")?;
    identity::validate_boot_mode(&boot, mode)?;
    let xhci: GuestXhciSign = extract(&serial, "CONDUIT_XHCI_SIGN ")?;
    run::validate_xhci(&boot, &xhci)?;
    let usb: GuestUsbSign = extract(&serial, "CONDUIT_USB_SIGN ")?;
    usb_run::validate(&boot, &xhci, &usb)?;
    if mode.entry().is_some() {
        return hid::retain(&paths, &serial, &boot, &xhci, &usb, mode);
    }
    let sign: EndpointSign = extract(&serial, "CONDUIT_USB_ENDPOINT_SIGN ")?;
    let boot_selection = boot_selection::verify(&serial, &boot, &usb)?;
    validate(&boot, &usb, &sign)?;
    let receipt = serde_json::json!({
        "schema": "conduit.conduitos.usb-endpoint-read-proof/v1", "base_commit": git_head(&paths.root)?,
        "proof_class": "freestanding-emulator", "qemu_controller": "qemu-xhci", "qemu_device": "usb-kbd",
        "boot": boot, "controller": xhci, "device": usb, "endpoint": sign,
        "boot_selection": boot_selection, "class_acceptance": false,
    });
    let output = paths.target.join("usb-endpoint-read-proof.json");
    fs::write(
        &output,
        serde_json::to_vec_pretty(&receipt)
            .map_err(|e| refusal("endpoint-proof-receipt", e.to_string()))?,
    )
    .map_err(|e| refusal("endpoint-proof-receipt", e.to_string()))?;
    println!("Raw endpoint kernel proof retained at {}", output.display());
    Ok(())
}

fn drive(
    socket: &Path,
    serial: &Path,
    child: &mut Child,
    mode: ProofMode,
) -> Result<(), ConduitosError> {
    let trace = serial.with_extension("qmp.jsonl");
    let (mut stream, mut reader) = qmp::connect_traced(socket, child, Some(&trace))?;
    for sequence in 0..128 {
        wait_for(
            serial,
            child,
            &format!("CONDUIT_USB_ENDPOINT_READY {sequence}"),
            if sequence == 0 {
                Duration::from_secs(120)
            } else {
                Duration::from_secs(5)
            },
        )?;
        let command = mode.input_command(sequence);
        qmp::request(
            &mut stream,
            &mut reader,
            command.as_bytes(),
            "endpoint-input",
        )?;
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| refusal("endpoint-proof-wait", e.to_string()))?
        {
            if status.code() != Some(EXPECTED_QEMU_SUCCESS) {
                return Err(refusal("endpoint-proof-exit", status.to_string()));
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(refusal(
                "endpoint-proof-timeout",
                "guest did not acknowledge its final stop",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for(
    path: &Path,
    child: &mut Child,
    marker: &str,
    budget: Duration,
) -> Result<(), ConduitosError> {
    let deadline = Instant::now() + budget;
    loop {
        let serial = read_serial(path)?;
        if serial.lines().any(|line| line == marker) {
            return Ok(());
        }
        if child
            .try_wait()
            .map_err(|e| refusal("endpoint-proof-wait", e.to_string()))?
            .is_some()
        {
            return Err(refusal(
                "endpoint-proof-early-exit",
                format!("waiting for {marker}: {serial}"),
            ));
        }
        if Instant::now() >= deadline {
            return Err(refusal(
                "endpoint-proof-timeout",
                format!("waiting for {marker}"),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn read_serial(path: &Path) -> Result<String, ConduitosError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() > 1024 * 1024 => Err(refusal(
            "endpoint-proof-transcript-bound",
            "serial transcript exceeds 1 MiB",
        )),
        Ok(_) => {
            fs::read_to_string(path).map_err(|e| refusal("endpoint-proof-serial", e.to_string()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(refusal("endpoint-proof-serial", e.to_string())),
    }
}
fn extract<T: serde::de::DeserializeOwned>(
    serial: &str,
    prefix: &str,
) -> Result<T, ConduitosError> {
    let mut lines = serial.lines().filter_map(|line| line.strip_prefix(prefix));
    let value = lines
        .next()
        .ok_or_else(|| refusal("endpoint-proof-sign", format!("missing {prefix}")))?;
    if lines.next().is_some() {
        return Err(refusal(
            "endpoint-proof-sign",
            format!("duplicate {prefix}"),
        ));
    }
    serde_json::from_str(value).map_err(|e| refusal("endpoint-proof-sign", e.to_string()))
}
fn validate(
    boot: &GuestBootSign,
    usb: &GuestUsbSign,
    sign: &EndpointSign,
) -> Result<(), ConduitosError> {
    let contract = EndpointReadContract::prepare()
        .map_err(|e| refusal("endpoint-proof-contract", format!("{e:?}")))?;
    let plan = endpoint_read_proof_plan::plan(
        &contract,
        &EndpointReadProofSubject {
            host_id: &boot.host_id,
            boot_id: &boot.boot_id,
            controller_base_id: &usb.controller_base_id,
            device_instance_id: &usb.device_instance_id,
            root_port: usb.root_port,
            slot: usb.slot,
            attachment_epoch: usb.attachment_epoch,
            endpoint_dci: sign.endpoint_dci,
            endpoint_epoch: sign.endpoint_epoch,
        },
    )
    .map_err(|e| refusal("endpoint-proof-plan", e))?;
    let active = bind_active_play(
        &plan.plan_id,
        &plan.fragments[0].host_id,
        &plan.fragments[0].boot_id,
        0,
    );
    let mut encoder = PreparedEndpointReadResultEncoder::new(&contract)
        .map_err(|e| refusal("endpoint-proof-contract", format!("{e:?}")))?;
    let mut transcript = Sha256::new();
    for sequence in 0..128_u64 {
        let frame = [0, 0, if sequence % 2 == 0 { 4 } else { 0 }, 0, 0, 0, 0, 0];
        let bytes = encoder
            .completed(8, 8, &frame)
            .map_err(|e| refusal("endpoint-proof-contract", format!("{e:?}")))?;
        transcript.update(sequence.to_le_bytes());
        transcript.update((bytes.len() as u32).to_le_bytes());
        transcript.update(bytes);
    }
    let digest: [u8; 32] = transcript.finalize().into();
    if sign.schema != "conduit.conduitos.usb-endpoint-read/v1"
        || sign.proof_class != "freestanding-emulator"
        || sign.status != "completed"
        || sign.source_document_id != plan.source_document_id.as_str()
        || sign.checked_plot_id != plan.checked_plot_id.as_str()
        || sign.plan_id != plan.plan_id.as_str()
        || sign.active_play_id != active.active_play_id.as_str()
        || sign.device_instance_id != usb.device_instance_id
        || sign.attachment_epoch != usb.attachment_epoch
        || sign.endpoint_epoch != 1
        || sign.endpoint_dci != 3
        || sign.transfers != 128
        || sign.cycle_transitions != 2
        || sign.transcript_digest != conduitos::identity::hex(&digest)
        || !sign.normal_close
        || !sign.acknowledged_stop
        || !sign.fixture_protocol
    {
        return Err(refusal(
            "endpoint-proof-invalid-receipt",
            format!("{sign:?}"),
        ));
    }
    Ok(())
}
fn refusal(code: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(code, detail)
}
