use std::{fs, process::Command};

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{
    image,
    profile::{Paths, QEMU_PROFILE},
    report::{git_head, GuestPcSpeakerSign},
    run, ConduitosArch, ConduitosError,
};

const NEGATIVE_CASES: [&str; 7] = [
    "base-absent",
    "stale-boot-identity",
    "richer-pcm-semantics",
    "unrepresentable-low-pitch",
    "unrepresentable-high-pitch",
    "malformed-active-play",
    "late-completion-after-cancellation",
];

#[derive(Serialize)]
struct PcSpeakerProofRecord {
    schema: &'static str,
    base_commit: String,
    proof_class: &'static str,
    qemu_profile: &'static str,
    mechanism: &'static str,
    timer_preservation: &'static str,
    positive: GuestPcSpeakerSign,
    deterministic_negative_command: &'static str,
    deterministic_negative_cases: &'static [&'static str],
    physical_audio_observation_claimed: bool,
    existing_conduitos_run_remained_green: bool,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-proof",
            "pc-speaker-proof dry-run cannot manufacture machine effects",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    image::execute(ConduitosArch::X86_64, opts)?;
    let positive = run::boot_once(&paths, opts)?;
    let status = Command::new("cargo")
        .args(["test", "-p", "conduitos", "--lib", "pc_speaker"])
        .current_dir(&paths.root)
        .status()
        .map_err(|error| {
            ConduitosError::refusal("pc-speaker-negative-tests-unavailable", error.to_string())
        })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "pc-speaker-negative-tests-failed",
            status.to_string(),
        ));
    }
    let record = PcSpeakerProofRecord {
        schema: "conduit.conduitos.pc-speaker-proof/v1",
        base_commit: git_head(&paths.root)?,
        proof_class: "freestanding-emulator",
        qemu_profile: QEMU_PROFILE,
        mechanism: "pit-channel-2+system-control-b-gate",
        timer_preservation: "pit-channel-0-untouched",
        positive: positive.pc_speaker,
        deterministic_negative_command: "cargo test -p conduitos --lib pc_speaker",
        deterministic_negative_cases: &NEGATIVE_CASES,
        physical_audio_observation_claimed: false,
        existing_conduitos_run_remained_green: true,
    };
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.pc_speaker_proof, bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if opts.json {
        println!("{}", serde_json::to_string(&record).unwrap());
    } else if !opts.quiet {
        println!(
            "ConduitOS PC-speaker proof: {}",
            paths.pc_speaker_proof.display()
        );
    }
    Ok(())
}
