//! Ordinary AArch64 product execution and independent EL0 boundary checks.
use super::{
    build, image, live_media,
    profile::Paths,
    report::{git_head, sha256_file},
    target_build, ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use std::fs;

const NEGATIVES: &str = "CONDUIT_AARCH64_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry gic uart translation-register irq-mask loop eret hvc smc breakpoint counter alternate-gate timer-control code-write data-execute";
const GATES: &str = "CONDUIT_DOMAIN_GATE_NEGATIVES unknown-handle sibling-handle wrong-operation oversized-window excessive-work invalid-capacity invalid-utf8 forged-fault replay exhausted-operations revoked-lifecycle provider-loss provider-replacement";
const TIMER: &str = "CONDUIT_DOMAIN_TIMER_COEXISTENCE source-wake-once user-irq budget-preemption";

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-protection-proof",
            "processor evidence requires a real boot",
        ));
    }
    live_media::build(live_media::LiveHost::Aarch64, opts)?;
    let paths = Paths::new(ConduitosArch::Aarch64)?;
    let output = paths.root.join("target/conduitos/live/aarch64-virt");
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
    let normal = fs::read(paths.target.join("aarch64-product-proof.json"))
        .map_err(|error| ConduitosError::refusal("product-proof-unavailable", error.to_string()))?;
    let normal: serde_json::Value = serde_json::from_slice(&normal)
        .map_err(|error| ConduitosError::refusal("product-proof-invalid", error.to_string()))?;
    build::aarch64_domain::execute(
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )?;
    image::assemble_architecture_proof(ConduitosArch::Aarch64, opts)?;
    target_build::boot_profile_image(
        &paths.iso,
        &manifest.target,
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )?;
    let transcript =
        fs::read_to_string(paths.target.join("aarch64-product-boot.log")).map_err(|error| {
            ConduitosError::refusal("domain-proof-log-unavailable", error.to_string())
        })?;
    validate(&transcript)?;
    let receipt = serde_json::json!({
        "schema": "conduit.conduitos/aarch64-ordinary-domain-proof@1",
        "base_commit": git_head(&paths.root)?, "architecture": "aarch64",
        "privilege": "el0", "cpu": "neoverse-n2",
        "proof_class": "freestanding-emulator", "image_sha256": sha256_file(&paths.iso)?,
        "ordinary_product_play": normal, "negative_entries": NEGATIVES,
        "capability_and_lifecycle_negatives": true,
        "source_timer_wake_retained_during_budget_preemption": true,
        "dma_isolation": false, "driver_isolation": false
    });
    fs::write(
        paths.target.join("aarch64-ordinary-domain-proof.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|error| {
            ConduitosError::refusal("domain-proof-receipt-invalid", error.to_string())
        })?,
    )
    .map_err(|error| {
        ConduitosError::refusal("domain-proof-receipt-unavailable", error.to_string())
    })?;
    if !opts.quiet {
        println!("PROVED ordinary AArch64 EL0 execution and independent boundary checks");
    }
    Ok(())
}

fn validate(transcript: &str) -> Result<(), ConduitosError> {
    if [NEGATIVES, GATES, TIMER]
        .iter()
        .any(|expected| !transcript.lines().any(|line| line == *expected))
        || transcript.contains("CONDUIT_AARCH64_DOMAIN_REFUSAL")
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
        let complete = format!("{NEGATIVES}\n{GATES}\n{TIMER}\n");
        assert!(validate(&complete).is_ok());
        for missing in [NEGATIVES, GATES, TIMER] {
            assert!(validate(&complete.replace(missing, "")).is_err());
        }
        assert!(validate(&(complete + "CONDUIT_AARCH64_DOMAIN_REFUSAL failed\n")).is_err());
    }
}
