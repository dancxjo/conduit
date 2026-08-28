//! Active ordinary-Play case for the local rescue reboot proof.

use std::{
    fs,
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

use super::{hid_qmp, profile::Paths, ConduitosError};

#[derive(Debug, Deserialize)]
struct BootSign {
    boot_id: String,
}

#[derive(Debug, Deserialize)]
struct RescueRequestSign {
    schema: String,
    status: String,
    proof_class: String,
    old_boot_id: String,
    authority: String,
    policy: String,
    operation: String,
    request_id: String,
    ordinary_keyboard_plan: bool,
}

#[derive(Default, Serialize)]
pub(super) struct ActivePlayProof {
    old_boot_id: String,
    new_boot_id: String,
    boot_id_changed: bool,
    request_id: String,
    ordinary_keyboard_plan_before_request: bool,
    ordinary_keyboard_plan_receipt: bool,
    same_qemu_process_observed_after_new_boot: bool,
}

pub(super) fn prove(paths: &Paths) -> Result<ActivePlayProof, ConduitosError> {
    let monitor_socket = paths.target.join("rescue-active-monitor.sock");
    let serial_path = paths.target.join("rescue-active-serial.log");
    let _ = fs::remove_file(&monitor_socket);
    let _ = fs::remove_file(&serial_path);
    let monitor = format!(
        "unix:{},server=on,wait=off",
        monitor_socket.to_string_lossy()
    );
    let serial_target = format!("file:{}", serial_path.to_string_lossy());
    let mut child = launch_qemu(paths, &monitor, &serial_target)?;
    hid_qmp::inject_active_play_rescue_and_wait_for_reboot(
        &monitor_socket,
        &serial_path,
        &mut child,
    )?;
    let still_running = child
        .try_wait()
        .map_err(|error| ConduitosError::refusal("active-rescue-wait-failed", error.to_string()))?
        .is_none();
    let serial = fs::read_to_string(&serial_path).map_err(|error| {
        ConduitosError::refusal("active-rescue-transcript-unavailable", error.to_string())
    })?;
    let result = validate(&serial, still_running);
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(&monitor_socket);
    result
}

fn validate(serial: &str, still_running: bool) -> Result<ActivePlayProof, ConduitosError> {
    let xhci: Vec<BootSign> = parse_signs(serial, "CONDUIT_XHCI_SIGN ")?;
    let requests: Vec<RescueRequestSign> = parse_signs(serial, "CONDUIT_RESCUE_SIGN ")?;
    let request_offset = serial.find("CONDUIT_RESCUE_SIGN ");
    let play_offset = serial.find("CONDUIT_BOOT_STAGE keyboard-text-play-started");
    let second_boot_offset = serial
        .match_indices("CONDUIT_XHCI_SIGN ")
        .nth(1)
        .map(|(offset, _)| offset);
    if xhci.len() != 2
        || requests.len() != 1
        || play_offset.is_none()
        || request_offset.is_none()
        || play_offset >= request_offset
        || request_offset >= second_boot_offset
        || requests[0].schema != "conduit.conduitos.local-rescue-request/v1"
        || requests[0].status != "accepted"
        || requests[0].proof_class != "freestanding-emulator"
        || requests[0].authority != "local-physical-input"
        || requests[0].policy != conduitos::local_rescue::LOCAL_RESCUE_POLICY
        || requests[0].operation != conduitos::local_rescue::LOCAL_REBOOT_OPERATION
        || !requests[0].ordinary_keyboard_plan
        || requests[0].old_boot_id != xhci[0].boot_id
        || requests[0].request_id != format!("local-rescue/{}/1", xhci[0].boot_id)
        || xhci[0].boot_id == xhci[1].boot_id
        || !still_running
    {
        return Err(ConduitosError::refusal(
            "active-rescue-correlation-invalid",
            "active K6 Play, local request, or fresh same-process Boot evidence disagreed",
        ));
    }
    Ok(ActivePlayProof {
        old_boot_id: xhci[0].boot_id.clone(),
        new_boot_id: xhci[1].boot_id.clone(),
        boot_id_changed: true,
        request_id: requests[0].request_id.clone(),
        ordinary_keyboard_plan_before_request: true,
        ordinary_keyboard_plan_receipt: true,
        same_qemu_process_observed_after_new_boot: true,
    })
}

fn parse_signs<T: for<'de> Deserialize<'de>>(
    serial: &str,
    prefix: &str,
) -> Result<Vec<T>, ConduitosError> {
    serial
        .lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|error| ConduitosError::refusal("active-rescue-sign-invalid", error.to_string()))
}

fn launch_qemu(
    paths: &Paths,
    monitor: &str,
    serial: &str,
) -> Result<std::process::Child, ConduitosError> {
    Command::new("qemu-system-x86_64")
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
            monitor,
            "-serial",
            serial,
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
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_play_must_precede_request_and_fresh_boot() {
        let xhci = |boot| format!("CONDUIT_XHCI_SIGN {{\"boot_id\":\"{boot}\"}}\n");
        let mut serial = xhci("b1");
        serial.push_str("CONDUIT_BOOT_STAGE keyboard-text-play-started\n");
        serial.push_str("CONDUIT_RESCUE_SIGN {\"schema\":\"conduit.conduitos.local-rescue-request/v1\",\"status\":\"accepted\",\"proof_class\":\"freestanding-emulator\",\"old_boot_id\":\"b1\",\"authority\":\"local-physical-input\",\"policy\":\"conduitos/local-physical-rescue@1\",\"operation\":\"conduitos.machine/reboot@1\",\"request_id\":\"local-rescue/b1/1\",\"ordinary_keyboard_plan\":true}\n");
        serial.push_str(&xhci("b2"));
        assert!(validate(&serial, true).is_ok());
        assert!(validate(&serial.replace("true", "false"), true).is_err());
        assert!(validate(&serial.replace("b2", "b1"), true).is_err());
    }
}
