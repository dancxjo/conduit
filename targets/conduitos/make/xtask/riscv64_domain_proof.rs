//! Ordinary RISC-V64 product execution and independent U-mode boundary checks.
use super::{
    build, image, live_media,
    profile::Paths,
    report::{git_head, sha256_file},
    target_build, ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use std::fs;

const NEGATIVES: &str = "CONDUIT_RISCV64_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry clint plic uart translation-register irq-mask loop floating-loop sret mret sbi breakpoint counter alternate-gate timer-control seed wfi code-write data-execute";
const GATES: &str = "CONDUIT_DOMAIN_GATE_NEGATIVES unknown-handle sibling-handle wrong-operation oversized-window excessive-work invalid-capacity invalid-utf8 forged-fault replay exhausted-operations revoked-lifecycle provider-loss provider-replacement";
const FLOATING: &str =
    "CONDUIT_RISCV64_DOMAIN_FLOATING restored-f0-f31-fcsr-before-rust-and-irq-handler";
const TIMER: &str = "CONDUIT_DOMAIN_TIMER_COEXISTENCE source-wake-once user-irq budget-preemption";

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-protection-proof",
            "processor evidence requires a real boot",
        ));
    }
    live_media::build(live_media::LiveHost::Riscv64, opts)?;
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    let output = paths.root.join("target/conduitos/live/riscv64-virt");
    let manifest = crate::commands::host::host_target::verify_target(&output)
        .map_err(|error| ConduitosError::refusal("proof-product-invalid", error.to_string()))?;
    // The normal image earns its own receipt before adding diagnostic entries.
    target_build::boot_profile_image(
        &paths.iso,
        &manifest.target,
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )?;
    let normal = fs::read(paths.target.join("riscv64-product-proof.json"))
        .map_err(|error| ConduitosError::refusal("product-proof-unavailable", error.to_string()))?;
    let normal: serde_json::Value = serde_json::from_slice(&normal)
        .map_err(|error| ConduitosError::refusal("product-proof-invalid", error.to_string()))?;
    build::riscv64_domain::execute(
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )?;
    image::assemble_architecture_proof(ConduitosArch::Riscv64, opts)?;
    target_build::boot_profile_image(
        &paths.iso,
        &manifest.target,
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )?;
    let transcript =
        fs::read_to_string(paths.target.join("riscv64-serial.log")).map_err(|error| {
            ConduitosError::refusal("domain-proof-log-unavailable", error.to_string())
        })?;
    validate(&transcript)?;
    let morse = super::protected_morse_proof::validate(&transcript)?;
    let receipt = serde_json::json!({
        "schema": "conduit.conduitos/riscv64-ordinary-domain-proof@1",
        "base_commit": git_head(&paths.root)?, "architecture": "riscv64",
        "privilege": "u-mode", "cpu": "rv64,zkr=true,sv57=off,sv48=off",
        "proof_class": "freestanding-emulator", "image_sha256": sha256_file(&paths.iso)?,
        "ordinary_product_play": normal, "protected_tour_morse": morse, "negative_entries": NEGATIVES,
        "capability_and_lifecycle_negatives": true,
        "floating_state_restored_before_rust_and_irq_handler": true,
        "source_timer_wake_retained_during_budget_preemption": true,
        "dma_isolation": false, "driver_isolation": false
    });
    fs::write(
        paths.target.join("riscv64-ordinary-domain-proof.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|error| {
            ConduitosError::refusal("domain-proof-receipt-invalid", error.to_string())
        })?,
    )
    .map_err(|error| {
        ConduitosError::refusal("domain-proof-receipt-unavailable", error.to_string())
    })?;
    if !opts.quiet {
        println!("PROVED ordinary RISC-V64 U-mode execution and independent boundary checks");
    }
    Ok(())
}

fn validate(transcript: &str) -> Result<(), ConduitosError> {
    if [NEGATIVES, GATES, TIMER, FLOATING]
        .iter()
        .any(|expected| !transcript.lines().any(|line| line == *expected))
        || transcript.contains("CONDUIT_RISCV64_DOMAIN_REFUSAL")
    {
        return Err(ConduitosError::refusal(
            "domain-negatives-unproven",
            transcript,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_independent_boundary_and_no_refusal_are_required() {
        let complete = format!("{NEGATIVES}\n{GATES}\n{TIMER}\n{FLOATING}\n");
        assert!(validate(&complete).is_ok());
        for missing in [NEGATIVES, GATES, TIMER, FLOATING] {
            assert!(validate(&complete.replace(missing, "")).is_err());
        }
        assert!(validate(&(complete + "CONDUIT_RISCV64_DOMAIN_REFUSAL failed\n")).is_err());
    }
}
