use std::{fs, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    prepared_proof_image,
    profile::{Paths, QEMU_PROFILE},
    report::{git_head, GuestUsbSign},
    run, usb_run, ConduitosArch, ConduitosError,
};

mod configuration_probe;
mod device_probe;
mod kernel;
mod ring;

const NEGATIVE_CASES: [&str; 14] = [
    "device-absent",
    "port-reset-timeout-or-failure",
    "malformed-descriptor-chain",
    "oversized-configuration",
    "too-many-interfaces",
    "too-many-endpoints",
    "too-many-descriptor-records",
    "control-stall-error-timeout",
    "wrong-controller-slot-endpoint-completion",
    "unsupported-topology",
    "device-vanished",
    "stale-device-instance",
    "malformed-transfer-completion-residual",
    "native-control-dma-buffer-envelope",
];

#[derive(Serialize)]
struct UsbProofRecord {
    schema: &'static str,
    base_commit: String,
    proof_class: &'static str,
    qemu_profile: &'static str,
    qemu_controller: &'static str,
    qemu_device: &'static str,
    positive: GuestUsbSign,
    control_ring_reuse: ring::RingProofSign,
    control_kernel: kernel::KernelProofSign,
    device_probe: device_probe::DeviceProbeSign,
    configuration_probe: device_probe::DeviceProbeSign,
    device_absent_refusal: String,
    deterministic_negative_command: &'static str,
    deterministic_negative_cases: &'static [&'static str],
    semantic_keyboard_offer: bool,
    existing_conduitos_run_remained_green: bool,
}

pub fn execute(prepared_image: bool, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-proof",
            "usb-proof dry-run cannot manufacture device evidence",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    prepared_proof_image::ensure(prepared_image, opts)?;
    let positive = run::boot_once(&paths, opts)?;
    if positive.usb.short_packets == 0 {
        return Err(ConduitosError::refusal(
            "usb-short-control-proof-missing",
            "the bounded descriptor request must retain a short Data Stage followed by successful Status Stage",
        ));
    }
    let control_ring_reuse =
        ring::extract(&positive.serial, positive.usb.root_port, positive.usb.slot)?;
    let control_kernel = kernel::extract(
        &positive.serial,
        &conduitos::usb_base::control_proof_plan::ControlProofSubject {
            host_id: &positive.boot.host_id,
            boot_id: &positive.boot.boot_id,
            controller_base_id: &positive.usb.controller_base_id,
            device_instance_id: &positive.usb.device_instance_id,
            root_port: positive.usb.root_port,
            slot: positive.usb.slot,
            attachment_epoch: positive.usb.attachment_epoch,
        },
        control_ring_reuse.final_position(),
    )?;
    let device_probe = device_probe::extract(
        &positive.serial,
        &conduitos::usb_base::control_proof_plan::ControlProofSubject {
            host_id: &positive.boot.host_id,
            boot_id: &positive.boot.boot_id,
            controller_base_id: &positive.usb.controller_base_id,
            device_instance_id: &positive.usb.device_instance_id,
            root_port: positive.usb.root_port,
            slot: positive.usb.slot,
            attachment_epoch: positive.usb.attachment_epoch,
        },
        control_kernel.final_position(),
    )?;
    let configuration_probe = configuration_probe::extract(
        &positive.serial,
        &conduitos::usb_base::control_proof_plan::ControlProofSubject {
            host_id: &positive.boot.host_id,
            boot_id: &positive.boot.boot_id,
            controller_base_id: &positive.usb.controller_base_id,
            device_instance_id: &positive.usb.device_instance_id,
            root_port: positive.usb.root_port,
            slot: positive.usb.slot,
            attachment_epoch: positive.usb.attachment_epoch,
        },
        device_probe.final_position(),
    )?;
    let absent = usb_run::prove_absent(&paths)?;
    let status = Command::new("cargo")
        .args(["test", "-p", "conduitos", "--lib", "arch::x86_64::usb"])
        .current_dir(&paths.root)
        .status()
        .map_err(|error| {
            ConduitosError::refusal("usb-negative-tests-unavailable", error.to_string())
        })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "usb-negative-tests-failed",
            status.to_string(),
        ));
    }
    let record = UsbProofRecord {
        schema: "conduit.conduitos.usb-proof/v3",
        base_commit: git_head(&paths.root)?,
        proof_class: "freestanding-emulator",
        qemu_profile: QEMU_PROFILE,
        qemu_controller: "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
        qemu_device: "usb-kbd,bus=conduitos-xhci.0,port=1",
        positive: positive.usb,
        control_ring_reuse,
        control_kernel,
        device_probe,
        configuration_probe,
        device_absent_refusal: absent,
        deterministic_negative_command: "cargo test -p conduitos --lib arch::x86_64::usb",
        deterministic_negative_cases: &NEGATIVE_CASES,
        semantic_keyboard_offer: false,
        existing_conduitos_run_remained_green: true,
    };
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.usb_proof, bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&record).unwrap());
    } else if !opts.quiet {
        println!("ConduitOS USB proof: {}", paths.usb_proof.display());
    }
    Ok(())
}
