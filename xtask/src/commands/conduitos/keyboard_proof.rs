use std::{fs, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, QEMU_PROFILE},
    report::{git_head, GuestKeyboardSign, GuestKeyboardTextSign},
    run, usb_run, ConduitosArch, ConduitosError,
};

const NEGATIVE_CASES: [&str; 10] = [
    "device-absent-no-offer",
    "not-a-boot-keyboard",
    "ambiguous-compatible-device",
    "stale-boot-identity",
    "artifact-build-mismatch",
    "resource-capacity-exhausted",
    "portable-value-invalid",
    "cord-pressure",
    "transfer-failure-distinct-from-pressure",
    "device-loss-distinct-from-closure",
];

#[derive(Serialize)]
struct KeyboardProofRecord {
    schema: &'static str,
    base_commit: String,
    proof_class: &'static str,
    qemu_profile: &'static str,
    qemu_device: &'static str,
    positive: GuestKeyboardSign,
    keyboard_text: GuestKeyboardTextSign,
    device_absent_refusal: String,
    deterministic_device_command: &'static str,
    deterministic_kernel_command: &'static str,
    deterministic_negative_cases: &'static [&'static str],
    ordinary_plan: bool,
    production_kernel: bool,
    observatory_exact_offer: bool,
    patchbay_native_projection: bool,
    layout_translation: bool,
    unicode_translation: bool,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-proof",
            "keyboard-proof dry-run cannot manufacture a realized Plan or Play",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    image::execute(ConduitosArch::X86_64, opts)?;
    let positive_run = run::boot_once(&paths, opts)?;
    let absent = usb_run::prove_absent(&paths)?;
    let device_status = Command::new("cargo")
        .args(["test", "-p", "conduitos", "--lib"])
        .current_dir(&paths.root)
        .status()
        .map_err(|error| {
            ConduitosError::refusal("keyboard-tests-unavailable", error.to_string())
        })?;
    if !device_status.success() {
        return Err(ConduitosError::refusal(
            "keyboard-tests-failed",
            device_status.to_string(),
        ));
    }
    let kernel_status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "conduit-kernel",
            "multi_value_port_graph_handles_pressure_closure_and_uneven_consumers",
        ])
        .current_dir(&paths.root)
        .status()
        .map_err(|error| {
            ConduitosError::refusal("keyboard-kernel-test-unavailable", error.to_string())
        })?;
    if !kernel_status.success() {
        return Err(ConduitosError::refusal(
            "keyboard-kernel-test-failed",
            kernel_status.to_string(),
        ));
    }
    let record = KeyboardProofRecord {
        schema: "conduit.conduitos.keyboard-proof/v1",
        base_commit: git_head(&paths.root)?,
        proof_class: "freestanding-emulator",
        qemu_profile: QEMU_PROFILE,
        qemu_device: "usb-kbd,bus=conduitos-xhci.0,port=1",
        positive: positive_run.keyboard,
        keyboard_text: positive_run.keyboard_text,
        device_absent_refusal: absent,
        deterministic_device_command: "cargo test -p conduitos --lib",
        deterministic_kernel_command: "cargo test -p conduit-kernel multi_value_port_graph_handles_pressure_closure_and_uneven_consumers",
        deterministic_negative_cases: &NEGATIVE_CASES,
        ordinary_plan: true,
        production_kernel: true,
        observatory_exact_offer: true,
        patchbay_native_projection: true,
        layout_translation: false,
        unicode_translation: false,
    };
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.keyboard_proof, bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&record).unwrap());
    } else if !opts.quiet {
        println!(
            "ConduitOS keyboard proof: {}",
            paths.keyboard_proof.display()
        );
    }
    Ok(())
}
