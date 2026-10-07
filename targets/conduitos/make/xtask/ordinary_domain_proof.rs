use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

use super::{build, image, profile::Paths, report::ArtifactRole, ConduitosArch, ConduitosError};

const PREFIX: &str = "CONDUIT_ORDINARY_DOMAIN_SIGN ";

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    build::execute_ordinary_domain_proof(opts)?
        .artifact_role
        .require(ArtifactRole::ArchitectureProofAppliance)?;
    image::assemble_architecture_proof(ConduitosArch::X86_64, opts)?;
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-protection-proof",
            "dry-run cannot manufacture processor-enforced evidence",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let serial_path = paths.target.join("ordinary-domain-proof.log");
    let _ = fs::remove_file(&serial_path);
    let serial = format!("file:{}", serial_path.to_string_lossy());
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
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            ConduitosError::refusal("ordinary-domain-proof-wait-failed", error.to_string())
        })? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ConduitosError::refusal(
                "ordinary-domain-proof-timeout",
                fs::read_to_string(&serial_path).unwrap_or_default(),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let transcript = fs::read_to_string(&serial_path).unwrap_or_default();
    if status.code() != Some(33) {
        return Err(ConduitosError::refusal(
            "ordinary-domain-proof-guest-failed",
            format!("{status}; {transcript}"),
        ));
    }
    let signs = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    if signs.len() != 1 {
        return Err(ConduitosError::refusal(
            "ordinary-domain-proof-sign-count",
            format!("expected one Sign, found {}; {transcript}", signs.len()),
        ));
    }
    let sign: serde_json::Value = serde_json::from_str(signs[0]).map_err(|error| {
        ConduitosError::refusal("ordinary-domain-proof-sign-invalid", error.to_string())
    })?;
    if sign["status"] != "completed"
        || sign["proof_class"] != "freestanding-emulator"
        || sign["architecture"] != "x86_64"
        || sign["privilege"] != "ring3"
        || sign["ordinary_source"] != true
        || sign["protected_computation"] != true
        || sign["effect_capability_gates"] != false
        || sign["dma_isolation"] != false
        || sign["driver_isolation"] != false
        || sign["bounded"] != true
    {
        return Err(ConduitosError::refusal(
            "ordinary-domain-proof-sign-mismatch",
            sign.to_string(),
        ));
    }
    if !transcript.contains("CONDUIT_SERIAL_PRESENT HELLO, CONDUITOS") {
        return Err(ConduitosError::refusal(
            "ordinary-domain-proof-effect-absent",
            "the ordinary text result was not independently observed",
        ));
    }
    if !transcript.lines().any(|line| line == "CONDUIT_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry mmio ports cli loop fp code-write data-execute") {
        return Err(ConduitosError::refusal("ordinary-domain-negatives-absent", "all hostile entries must fault or return within the timer bound"));
    }
    let costs = transcript
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_DOMAIN_COST "))
        .collect::<Vec<_>>();
    if costs.len() != 1 {
        return Err(ConduitosError::refusal(
            "ordinary-domain-cost-count",
            "expected one ordinary Source region cost record",
        ));
    }
    let cost: serde_json::Value = serde_json::from_str(costs[0]).map_err(|error| {
        ConduitosError::refusal("ordinary-domain-cost-invalid", error.to_string())
    })?;
    if cost["entries"] != 1
        || cost["gate_transitions"] != 1
        || cost["address_space_switches"] != 2
        || cost["scheduler_returns"] != 1
        || cost["state"] != "Revoked(PlayCompleted)"
        || cost["reserved_bytes"] != 118784
        || cost["dma_isolation"] != false
        || cost["driver_isolation"] != false
    {
        return Err(ConduitosError::refusal(
            "ordinary-domain-cost-mismatch",
            cost.to_string(),
        ));
    }
    if !opts.quiet && !opts.json {
        println!("PROVED checked ordinary text computation in x86_64 CPL3");
    }
    Ok(())
}
