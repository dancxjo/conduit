use std::{fs, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, QEMU_PROFILE},
    report::{git_head, GuestHidSign},
    run, ConduitosArch, ConduitosError,
};

const NEGATIVE_CASES: [&str; 12] = [
    "hid-interface-absent-or-not-boot",
    "mouse-protocol",
    "missing-invalid-or-ambiguous-interrupt-in",
    "unsupported-packet-size",
    "set-protocol-stall-or-error",
    "short-or-reserved-byte-report",
    "rollover-error-report",
    "duplicate-usage",
    "wrong-device-endpoint-or-completion",
    "device-removed-while-outstanding",
    "transition-queue-overflow",
    "transfer-stall-error-or-timeout",
];

#[derive(Serialize)]
struct HidProofRecord {
    schema: &'static str,
    base_commit: String,
    proof_class: &'static str,
    qemu_profile: &'static str,
    qemu_controller: &'static str,
    qemu_device: &'static str,
    qemu_input_path: &'static str,
    positive: GuestHidSign,
    deterministic_negative_command: &'static str,
    deterministic_negative_cases: &'static [&'static str],
    report_descriptor_parser: bool,
    layout_translation: bool,
    unicode_translation: bool,
    semantic_keyboard_offer: bool,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-proof",
            "hid-proof dry-run cannot manufacture keyboard transitions",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    image::execute(ConduitosArch::X86_64, opts)?;
    let positive = run::boot_once(&paths, opts)?.hid;
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "conduitos",
            "--lib",
            "arch::x86_64::hid::tests",
        ])
        .current_dir(&paths.root)
        .status()
        .map_err(|error| {
            ConduitosError::refusal("hid-negative-tests-unavailable", error.to_string())
        })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "hid-negative-tests-failed",
            status.to_string(),
        ));
    }
    let record = HidProofRecord {
        schema: "conduit.conduitos.hid-proof/v1",
        base_commit: git_head(&paths.root)?,
        proof_class: "freestanding-emulator",
        qemu_profile: QEMU_PROFILE,
        qemu_controller: "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
        qemu_device: "usb-kbd,bus=conduitos-xhci.0,port=1",
        qemu_input_path: "QMP input-send-event qcode a down/up",
        positive,
        deterministic_negative_command: "cargo test -p conduitos --lib arch::x86_64::hid::tests",
        deterministic_negative_cases: &NEGATIVE_CASES,
        report_descriptor_parser: false,
        layout_translation: false,
        unicode_translation: false,
        semantic_keyboard_offer: false,
    };
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.hid_proof, bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&record).unwrap());
    } else if !opts.quiet {
        println!("ConduitOS HID proof: {}", paths.hid_proof.display());
    }
    Ok(())
}
