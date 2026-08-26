use std::{fs, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, QEMU_PROFILE},
    report::{git_head, GuestXhciSign},
    run, ConduitosArch, ConduitosError,
};

const NEGATIVE_CASES: [&str; 12] = [
    "controller-absent",
    "wrong-pci-class",
    "invalid-bar",
    "impossible-register-layout",
    "reset-timeout",
    "command-ring-full",
    "unexpected-completion",
    "stale-base-identity",
    "dma-alignment-refusal",
    "unsupported-page-size",
    "scratchpad-storage-refusal",
    "command-timeout",
];

#[derive(Serialize)]
struct XhciProofRecord {
    schema: &'static str,
    base_commit: String,
    proof_class: &'static str,
    qemu_profile: &'static str,
    qemu_device: &'static str,
    positive: GuestXhciSign,
    controller_absent_refusal: String,
    deterministic_negative_command: &'static str,
    deterministic_negative_cases: &'static [&'static str],
    existing_conduitos_run_remained_green: bool,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-proof",
            "xhci-proof dry-run cannot manufacture controller evidence",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    image::execute_proof(ConduitosArch::X86_64, opts)?;
    let positive = run::boot_once(&paths, opts)?;
    let absent = run::prove_xhci_absent(&paths)?;
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "conduitos",
            "--lib",
            "arch::x86_64::xhci::tests",
        ])
        .current_dir(&paths.root)
        .status()
        .map_err(|error| {
            ConduitosError::refusal("xhci-negative-tests-unavailable", error.to_string())
        })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "xhci-negative-tests-failed",
            status.to_string(),
        ));
    }
    let record = XhciProofRecord {
        schema: "conduit.conduitos.xhci-proof/v1",
        base_commit: git_head(&paths.root)?,
        proof_class: "freestanding-emulator",
        qemu_profile: QEMU_PROFILE,
        qemu_device: "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
        positive: positive.xhci,
        controller_absent_refusal: absent,
        deterministic_negative_command: "cargo test -p conduitos --lib arch::x86_64::xhci::tests",
        deterministic_negative_cases: &NEGATIVE_CASES,
        existing_conduitos_run_remained_green: true,
    };
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.xhci_proof, bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&record).unwrap());
    } else if !opts.quiet {
        println!("ConduitOS xHCI proof: {}", paths.xhci_proof.display());
    }
    Ok(())
}
