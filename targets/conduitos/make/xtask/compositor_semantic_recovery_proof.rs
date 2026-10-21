//! Separate native-profile semantic fixture. The ordinary proof keeps its budget.
use super::{build, image, profile::Paths, report::ArtifactRole, ConduitosArch, ConduitosError};
use crate::cli::GlobalOpts;
use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
mod profile_receipt;
#[path = "ordinary_domain_proof/compositor_semantic_recovery.rs"]
mod receipt;

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let (built, manifest) = build::compositor_semantic_recovery::execute(opts)?;
    built
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    image::assemble_architecture_proof(ConduitosArch::X86_64, opts)?;
    if opts.dry_run {
        return Err(refusal("semantic-recovery-dry-run", "no emulator evidence"));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let log = paths.target.join("compositor-semantic-recovery-proof.log");
    let _ = fs::remove_file(&log);
    let serial = format!("file:{}", log.display());
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            "512M",
            "-smp",
            "1",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
            &serial,
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-cdrom",
        ])
        .arg(&paths.iso)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| refusal("semantic-recovery-qemu-unavailable", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| refusal("semantic-recovery-wait", error.to_string()))?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(refusal(
                "semantic-recovery-timeout",
                fs::read_to_string(&log).unwrap_or_default(),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let transcript = fs::read_to_string(&log).unwrap_or_default();
    if status.code() != Some(33) {
        return Err(refusal(
            "semantic-recovery-guest-failed",
            format!("{status}; {transcript}"),
        ));
    }
    let budget = profile_receipt::validate(
        &transcript,
        &manifest.profile_id,
        &manifest.build_id,
        &manifest.image_id,
        manifest.bounds.heap_arena_bytes,
    )?;
    let observed = receipt::validate(&transcript)?;
    let record = serde_json::json!({
        "schema":"conduit.conduitos/compositor-semantic-copy-recovery-evidence@2",
        "proof_class":"freestanding-emulator","fixture":true,
        "proof_feature":"compositor-semantic-recovery-proof",
        "same_uninstrumented_product_binary":false,
        "machine":{"emulator":"qemu-system-x86_64","model":"q35","cpu":"max",
            "memory_mib":512,"vcpus":1},
        "budget_basis":"existing conduitos-native.host.conduit; actual admitted product profile, not minimum memory",
        "build":built,"native_make_manifest":manifest,"admitted_profile":budget,
        "observations":observed,"transcript_sha256":super::report::sha256_file(&log)?,
    });
    fs::write(
        paths
            .target
            .join("compositor-semantic-copy-recovery-fixture-proof.json"),
        serde_json::to_vec_pretty(&record)
            .map_err(|error| refusal("semantic-recovery-receipt", error.to_string()))?,
    )
    .map_err(|error| refusal("semantic-recovery-receipt-write", error.to_string()))?;
    if !opts.quiet && !opts.json {
        println!(
            "PROVED semantic compositor copy recovery in a separate native Make-profile appliance"
        );
    }
    Ok(())
}
fn refusal(reason: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(reason, detail)
}
