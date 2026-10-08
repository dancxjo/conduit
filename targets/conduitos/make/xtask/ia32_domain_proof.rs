//! Normal IA-32 Source/Play followed by architecture-specific hostile entries.
use super::{
    ConduitosArch, ConduitosError, build, ia32_product_boot, image, live_media,
    profile::Paths,
    report::{git_head, sha256_file},
};
use crate::cli::GlobalOpts;
use std::fs;

mod normal_product;

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-protection-proof",
            "processor evidence requires a real boot",
        ));
    }
    live_media::build(live_media::LiveHost::Ia32, opts)?;
    let paths = Paths::new(ConduitosArch::Ia32)?;
    let output = paths.root.join("target/conduitos/live/ia32-pc");
    let manifest = crate::commands::host::host_target::verify_target(&output)
        .map_err(|error| ConduitosError::refusal("proof-product-invalid", error.to_string()))?;
    let normal =
        normal_product::capture(&paths, &output.join(&manifest.image.file), &manifest, opts)?;
    build::ia32_domain::execute(
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )?;
    image::assemble_architecture_proof(ConduitosArch::Ia32, opts)?;
    ia32_product_boot::boot_legacy_bios(
        &paths.iso,
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.resolved_description_binding,
        opts,
    )?;
    let transcript = fs::read_to_string(paths.target.join("ia32-product-legacy-bios-only.log"))
        .map_err(|error| {
            ConduitosError::refusal("domain-proof-log-unavailable", error.to_string())
        })?;
    let morse = super::protected_morse_proof::validate(&transcript)?;
    let timer = super::protected_timer_proof::validate(&transcript, "ia32")?;
    let negatives = "CONDUIT_IA32_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry mmio ports cli loop direction-flag syscall sysenter divide breakpoint single-step rdtsc code-write data-execute";
    if !transcript.lines().any(|line| line == negatives)
        || !transcript
            .lines()
            .any(|line| line == "CONDUIT_IA32_DOMAIN_FLOATING restored-x87-mxcsr-xmm-before-rust")
        || !transcript.lines().any(|line| line == "CONDUIT_DOMAIN_GATE_NEGATIVES unknown-handle sibling-handle wrong-operation oversized-window excessive-work invalid-capacity invalid-utf8 forged-fault replay exhausted-operations revoked-lifecycle provider-loss provider-replacement")
        || !transcript.lines().any(|line| line == "CONDUIT_DOMAIN_TIMER_COEXISTENCE source-wake-once user-irq budget-preemption")
        || transcript.contains("CONDUIT_IA32_DOMAIN_REFUSAL")
    {
        return Err(ConduitosError::refusal(
            "domain-negatives-unproven",
            transcript,
        ));
    }
    let product: serde_json::Value = serde_json::from_slice(
        &fs::read(paths.target.join("ia32-legacy-bios-product-proof.json")).map_err(|error| {
            ConduitosError::refusal("domain-proof-receipt-unavailable", error.to_string())
        })?,
    )
    .map_err(|error| ConduitosError::refusal("domain-proof-receipt-invalid", error.to_string()))?;
    let receipt = serde_json::json!({
        "schema": "conduit.conduitos/ia32-ordinary-domain-proof@2",
        "base_commit": git_head(&paths.root)?,
        "architecture": "ia32", "privilege": "ring3",
        "proof_class": "freestanding-ia32-legacy-bios-emulator",
        "image_sha256": sha256_file(&paths.iso)?,
        "ordinary_product_play": normal, "protected_tour_morse": morse, "protected_standing_timer": timer, "instrumented_product_play": product,
        "negative_entries": negatives,
        "floating_state_restored_before_rust": true,
        "capability_and_lifecycle_negatives": true,
        "source_timer_wake_retained_during_budget_preemption": true,
        "dma_isolation": false, "driver_isolation": false
    });
    fs::write(
        paths.target.join("ia32-ordinary-domain-proof.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|error| {
            ConduitosError::refusal("domain-proof-receipt-invalid", error.to_string())
        })?,
    )
    .map_err(|error| {
        ConduitosError::refusal("domain-proof-receipt-unavailable", error.to_string())
    })?;
    if !opts.quiet {
        println!("PROVED ordinary IA-32 CPL3 execution and independent hostile entries");
    }
    Ok(())
}
