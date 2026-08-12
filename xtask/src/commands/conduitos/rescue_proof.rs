//! Same-process QEMU proof for the low-level local rescue reboot path.

use std::{
    fs,
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

use crate::cli::GlobalOpts;

use super::{
    hid_qmp, image,
    profile::{Paths, QEMU_PROFILE},
    report::{git_head, GuestXhciSign},
    ConduitosArch, ConduitosError,
};

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

#[derive(Serialize)]
struct RescueProofRecord {
    schema: &'static str,
    base_commit: String,
    proof_class: &'static str,
    qemu_profile: &'static str,
    qemu_pid: u32,
    old_boot_id: String,
    new_boot_id: String,
    boot_id_changed: bool,
    request_id: String,
    authority: String,
    policy: String,
    operation: String,
    request_count: usize,
    ordinary_keyboard_plan_before_request: bool,
    same_qemu_process_observed_after_new_boot: bool,
    request_acceptance_distinct_from_completion: bool,
    active_play_case: super::active_rescue_proof::ActivePlayProof,
    physical_near_miss_cases: Vec<String>,
    deterministic_negative_cases: &'static [&'static str],
    deterministic_matcher_command: &'static str,
    deterministic_malformed_hid_command: &'static str,
    cleanup_after_completion: &'static str,
}

const DETERMINISTIC_NEGATIVE_CASES: &[&str] = &[
    "textual-or-remote-values-cannot-construct-validated-local-transition",
    "malformed-hid-report-produces-no-transition",
    "held-delete-produces-at-most-one-request",
    "disabled-profile-produces-no-request",
    "reboot-base-unavailable-is-distinct",
    "stale-old-boot-identity-is-not-completion",
];

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-rescue-proof",
            "rescue-proof requires an observed guest reset and fresh Boot identity",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    image::execute_proof(ConduitosArch::X86_64, opts)?;
    run_deterministic_negatives(&paths)?;
    let monitor_socket = paths.target.join("rescue-monitor.sock");
    let serial_path = paths.target.join("rescue-serial.log");
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
            &serial_target,
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
        .map_err(|error| {
            ConduitosError::refusal(
                "missing-qemu",
                format!("cannot launch qemu-system-x86_64: {error}"),
            )
        })?;
    let qemu_pid = child.id();
    hid_qmp::inject_rescue_and_wait_for_reboot(&monitor_socket, &serial_path, &mut child)?;
    let still_running = child
        .try_wait()
        .map_err(|error| ConduitosError::refusal("qemu-rescue-wait-failed", error.to_string()))?
        .is_none();
    let serial = fs::read_to_string(&serial_path).map_err(|error| {
        ConduitosError::refusal("rescue-transcript-unavailable", error.to_string())
    })?;
    let result = validate(&serial, qemu_pid, still_running, git_head(&paths.root)?);
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(&monitor_socket);
    let mut proof = result?;
    proof.active_play_case = super::active_rescue_proof::prove(&paths)?;
    proof.physical_near_miss_cases = [
        (
            hid_qmp::RescueNearMiss::ControlDelete,
            "control-delete-without-alt",
        ),
        (
            hid_qmp::RescueNearMiss::AltDelete,
            "alt-delete-without-control",
        ),
        (
            hid_qmp::RescueNearMiss::ControlAltBackspace,
            "control-alt-backspace",
        ),
    ]
    .into_iter()
    .map(|(case, name)| prove_near_miss(&paths, case, name))
    .collect::<Result<_, _>>()?;
    fs::write(
        &paths.rescue_proof,
        serde_json::to_vec_pretty(&proof)
            .map_err(|error| ConduitosError::refusal("rescue-proof-invalid", error.to_string()))?,
    )
    .map_err(|error| ConduitosError::refusal("rescue-proof-write-failed", error.to_string()))?;
    if !opts.quiet {
        println!("ConduitOS rescue proof: {}", paths.rescue_proof.display());
    }
    Ok(())
}

fn validate(
    serial: &str,
    qemu_pid: u32,
    still_running: bool,
    base_commit: String,
) -> Result<RescueProofRecord, ConduitosError> {
    let xhci: Vec<GuestXhciSign> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_XHCI_SIGN "))
        .map(|value| {
            serde_json::from_str(value).map_err(|error| {
                ConduitosError::refusal("malformed-rescue-boot-sign", error.to_string())
            })
        })
        .collect::<Result<_, _>>()?;
    let requests: Vec<RescueRequestSign> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_RESCUE_SIGN "))
        .map(|value| {
            serde_json::from_str(value).map_err(|error| {
                ConduitosError::refusal("malformed-rescue-request-sign", error.to_string())
            })
        })
        .collect::<Result<_, _>>()?;
    if xhci.len() != 2 || requests.len() != 1 {
        return Err(ConduitosError::refusal(
            "rescue-correlation-incomplete",
            format!(
                "expected two boot-scoped xHCI Signs and one request, found {} and {}",
                xhci.len(),
                requests.len()
            ),
        ));
    }
    let request = &requests[0];
    let request_offset = serial
        .find("CONDUIT_RESCUE_SIGN ")
        .ok_or_else(|| ConduitosError::refusal("rescue-request-absent", "missing request"))?;
    let second_boot_offset = serial
        .match_indices("CONDUIT_XHCI_SIGN ")
        .nth(1)
        .map(|(offset, _)| offset)
        .ok_or_else(|| ConduitosError::refusal("rescue-new-boot-absent", "missing B2"))?;
    let ordinary_plan_before_request = serial[..request_offset].contains("keyboard-plan-ready");
    if request.schema != "conduit.conduitos.local-rescue-request/v1"
        || request.status != "accepted"
        || request.proof_class != "freestanding-emulator"
        || request.authority != "local-physical-input"
        || request.policy != conduitos::local_rescue::LOCAL_RESCUE_POLICY
        || request.operation != conduitos::local_rescue::LOCAL_REBOOT_OPERATION
        || request.old_boot_id != xhci[0].boot_id
        || request.request_id != format!("local-rescue/{}/1", xhci[0].boot_id)
        || request.ordinary_keyboard_plan
        || ordinary_plan_before_request
        || xhci[0].boot_id == xhci[1].boot_id
        || request_offset >= second_boot_offset
        || !still_running
    {
        return Err(ConduitosError::refusal(
            "rescue-correlation-invalid",
            "request provenance, ordering, fresh identity, or same-process witness disagreed",
        ));
    }
    Ok(RescueProofRecord {
        schema: "conduit.conduitos.local-rescue-proof/v1",
        base_commit,
        proof_class: "freestanding-emulator",
        qemu_profile: QEMU_PROFILE,
        qemu_pid,
        old_boot_id: xhci[0].boot_id.clone(),
        new_boot_id: xhci[1].boot_id.clone(),
        boot_id_changed: true,
        request_id: request.request_id.clone(),
        authority: request.authority.clone(),
        policy: request.policy.clone(),
        operation: request.operation.clone(),
        request_count: requests.len(),
        ordinary_keyboard_plan_before_request: ordinary_plan_before_request,
        same_qemu_process_observed_after_new_boot: still_running,
        request_acceptance_distinct_from_completion: true,
        active_play_case: super::active_rescue_proof::ActivePlayProof::default(),
        physical_near_miss_cases: Vec::new(),
        deterministic_negative_cases: DETERMINISTIC_NEGATIVE_CASES,
        deterministic_matcher_command: "cargo test -p conduitos local_rescue",
        deterministic_malformed_hid_command:
            "cargo test -p conduitos malformed_rollover_and_duplicates_never_make_transitions",
        cleanup_after_completion: "runner-terminated-qemu-after-observing-b2",
    })
}

fn run_deterministic_negatives(paths: &Paths) -> Result<(), ConduitosError> {
    for (filter, reason) in [
        ("local_rescue", "rescue-matcher-tests-failed"),
        (
            "malformed_rollover_and_duplicates_never_make_transitions",
            "rescue-malformed-hid-test-failed",
        ),
    ] {
        let status = Command::new("cargo")
            .args(["test", "-p", "conduitos", filter])
            .current_dir(&paths.root)
            .status()
            .map_err(|error| ConduitosError::refusal(reason, error.to_string()))?;
        if !status.success() {
            return Err(ConduitosError::refusal(reason, status.to_string()));
        }
    }
    Ok(())
}

fn prove_near_miss(
    paths: &Paths,
    case: hid_qmp::RescueNearMiss,
    name: &str,
) -> Result<String, ConduitosError> {
    let monitor_socket = paths.target.join(format!("rescue-negative-{name}.sock"));
    let serial_path = paths.target.join(format!("rescue-negative-{name}.log"));
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
    hid_qmp::inject_near_miss(&monitor_socket, &serial_path, &mut child, case)?;
    let serial = fs::read_to_string(&serial_path).map_err(|error| {
        ConduitosError::refusal("rescue-negative-transcript-unavailable", error.to_string())
    })?;
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(&monitor_socket);
    if serial.contains("CONDUIT_RESCUE_SIGN ")
        || serial.matches("CONDUIT_XHCI_SIGN ").count() != 1
        || !serial.contains("CONDUIT_KERNEL_SIGN")
    {
        return Err(ConduitosError::refusal(
            "rescue-near-miss-triggered",
            format!("physical near miss {name} did not remain in the old boot"),
        ));
    }
    Ok(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xhci(boot: &str) -> String {
        format!(
            "CONDUIT_XHCI_SIGN {{\"schema\":\"conduit.conduitos.xhci-base/v1\",\"status\":\"ready\",\"proof_class\":\"freestanding-emulator\",\"base_id\":\"base\",\"boot_id\":\"{boot}\",\"segment\":0,\"bus\":0,\"device\":1,\"function\":0,\"vendor\":1,\"device_id\":1,\"bar_physical\":1,\"hardware_slots\":1,\"admitted_slots\":1,\"command_trbs\":1,\"event_trbs\":1,\"dma_bytes\":1,\"dma_alignment\":1,\"maximum_pending_commands\":1,\"poll_steps\":1,\"sign_slots\":1,\"semantic_keyboard_offer\":false}}\n"
        )
    }

    #[test]
    fn correlation_requires_one_request_between_distinct_boots() {
        let mut serial = xhci("b1");
        serial.push_str("CONDUIT_BOOT_STAGE local-rescue-ready\n");
        serial.push_str("CONDUIT_RESCUE_SIGN {\"schema\":\"conduit.conduitos.local-rescue-request/v1\",\"status\":\"accepted\",\"proof_class\":\"freestanding-emulator\",\"old_boot_id\":\"b1\",\"authority\":\"local-physical-input\",\"policy\":\"conduitos/local-physical-rescue@1\",\"operation\":\"conduitos.machine/reboot@1\",\"request_id\":\"local-rescue/b1/1\",\"ordinary_keyboard_plan\":false}\n");
        serial.push_str(&xhci("b2"));
        assert!(validate(&serial, 7, true, "head".into()).is_ok());
        assert!(validate(&serial, 7, false, "head".into()).is_err());
        assert!(validate(&serial.replace("b2", "b1"), 7, true, "head".into()).is_err());
        let stale = format!(
            "{serial}CONDUIT_RESCUE_SIGN {{\"schema\":\"conduit.conduitos.local-rescue-request/v1\",\"status\":\"accepted\",\"proof_class\":\"freestanding-emulator\",\"old_boot_id\":\"b1\",\"authority\":\"local-physical-input\",\"policy\":\"conduitos/local-physical-rescue@1\",\"operation\":\"conduitos.machine/reboot@1\",\"request_id\":\"local-rescue/b1/2\",\"ordinary_keyboard_plan\":false}}\n"
        );
        assert!(validate(&stale, 7, true, "head".into()).is_err());
    }
}
