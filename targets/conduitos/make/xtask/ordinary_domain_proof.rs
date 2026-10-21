use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::cli::GlobalOpts;

use super::{ConduitosArch, ConduitosError, build, image, profile::Paths, report::ArtifactRole};

mod compositor;
mod compositor_hostile;
mod compositor_recovery;
mod service_memory;

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
            // This appliance also owns the 19 MiB private compositor-service
            // slot. Limine retains the loaded image while reserving the 16 MiB
            // runtime arena; the former 64 MiB guest refuses before entry.
            // This is an explicit fixture profile, not a product minimum.
            "128M",
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
    let timer = super::protected_timer_proof::validate(&transcript, "x86_64")?;
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
    let mut sign: serde_json::Value = serde_json::from_str(signs[0]).map_err(|error| {
        ConduitosError::refusal("ordinary-domain-proof-sign-invalid", error.to_string())
    })?;
    sign["protected_standing_timer"] = timer;
    sign["service_memory_fixture"] = service_memory::validate(&transcript)?;
    let compositor_guest = compositor::validate(&transcript)?;
    sign["compositor_hostile_fixture"] = compositor_hostile::validate(&transcript)?;
    sign["compositor_copy_recovery_fixture"] = compositor_recovery::validate(&transcript)?;
    if sign["status"] != "completed"
        || sign["proof_class"] != "freestanding-emulator"
        || sign["architecture"] != "x86_64"
        || sign["privilege"] != "ring3"
        || sign["ordinary_source"] != true
        || sign["protected_computation"] != true
        || sign["effect_capability_gates"] != true
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
    if !transcript.lines().any(|line| line == "CONDUIT_DOMAIN_NEGATIVES root-memory capability-memory sibling-memory root-entry mmio ports cli loop direction-flag fp syscall sysenter divide breakpoint single-step rdtsc code-write data-execute") {
        return Err(ConduitosError::refusal("ordinary-domain-negatives-absent", "all hostile entries must fault or return within the timer bound"));
    }
    if !transcript.lines().any(|line| line == "CONDUIT_DOMAIN_GATE_NEGATIVES unknown-handle sibling-handle wrong-operation oversized-window excessive-work invalid-capacity invalid-utf8 forged-fault replay exhausted-operations revoked-lifecycle provider-loss provider-replacement") {
        return Err(ConduitosError::refusal("ordinary-domain-gate-negatives-absent",
            "hostile capability requests and lifecycle events must be independently refused"));
    }
    if !transcript.lines().any(|line| {
        line == "CONDUIT_DOMAIN_TIMER_COEXISTENCE source-wake-once user-irq budget-preemption"
    }) {
        return Err(ConduitosError::refusal(
            "ordinary-domain-source-timer-lost",
            "a Source timer must wake once while the independent domain budget preempts",
        ));
    }
    if !transcript.lines().any(|line| {
        line == "CONDUIT_DOMAIN_KEYMAP retained-compose canonical-sdk protected-uppercase"
    }) {
        return Err(ConduitosError::refusal(
            "ordinary-domain-keymap-absent",
            "the portable keymap must preserve Compose state across protected entries",
        ));
    }
    if !transcript
        .lines()
        .any(|line| line == "CONDUIT_DOMAIN_CHAIN one-entry unicode-expansion")
    {
        return Err(ConduitosError::refusal(
            "ordinary-domain-chain-absent",
            "the selected pure chain must perform Unicode expansion in one entry",
        ));
    }
    if !transcript.lines().any(|line| {
        line == "CONDUIT_DOMAIN_EDITOR_FIXTURE bounded-capacity retained-state edit-refusal"
    }) {
        return Err(ConduitosError::refusal(
            "ordinary-domain-editor-fixture-absent",
            "the diagnostic editor must retain state across a bounded capacity refusal",
        ));
    }
    super::protected_morse_proof::validate(&transcript)?;
    let costs = transcript
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_DOMAIN_COST "))
        .filter(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .map(|value| value["fixture"] != true && value["entries"] == 3)
                .unwrap_or(true)
        })
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
    if cost["entries"] != 3
        || cost["gate_transitions"] != 3
        || cost["address_space_switches"] != 6
        || cost["scheduler_returns"] != 3
        || cost["base_gate_transitions"] != 1
        || cost["privilege_transitions"]
            .as_u64()
            .zip(cost["interrupt_entries"].as_u64())
            .is_none_or(|(value, interrupts)| {
                interrupts
                    .checked_add(3)
                    .and_then(|entries| entries.checked_mul(2))
                    != Some(value)
            })
        || cost["copied_bytes"]
            != (conduitos::ordinary_plan::TEXT_LITERAL.len()
                + 3 * conduitos::ordinary_plan::TEXT_RESULT.len()) as u64
        || cost["tlb_flushes"] != 6
        || cost["teardown_zeroed_bytes"] != 217088
        || cost["shared_peak_bytes"]
            != (conduitos::ordinary_plan::TEXT_LITERAL.len()
                + conduitos::ordinary_plan::TEXT_RESULT.len()) as u64
        || cost["ring_slots"] != 0
        || cost["state"] != "Revoked(PlayCompleted)"
        || cost["reserved_bytes"] != 217088
        || cost["dma_isolation"] != false
        || cost["driver_isolation"] != false
    {
        return Err(ConduitosError::refusal(
            "ordinary-domain-cost-mismatch",
            cost.to_string(),
        ));
    }
    let build: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.target.join("build.json")).map_err(|error| {
            ConduitosError::refusal("service-memory-build-record-unavailable", error.to_string())
        })?)
        .map_err(|error| {
            ConduitosError::refusal("service-memory-build-record-invalid", error.to_string())
        })?;
    let elf_sha256 = super::report::sha256_file(&paths.kernel)?;
    if build["elf_sha256"] != elf_sha256 || build["artifact_role"] != "architecture-proof-appliance"
    {
        return Err(ConduitosError::refusal(
            "service-memory-build-record-mismatch",
            "requires the actual fixture appliance ELF",
        ));
    }
    let receipt = serde_json::json!({
        "schema": "conduit.conduitos/service-memory-evidence@1",
        "proof_class": "freestanding-emulator", "fixture": true,
        "machine": {"emulator": "qemu-system-x86_64", "model": "q35",
            "cpu": "max", "memory_mib": 128, "vcpus": 1},
        "build": build, "observations": sign["service_memory_fixture"],
        "transcript_sha256": super::report::sha256_file(&serial_path)?,
    });
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| {
        ConduitosError::refusal("service-memory-receipt-invalid", error.to_string())
    })?;
    fs::write(
        paths.target.join("service-memory-fixture-proof.json"),
        bytes,
    )
    .map_err(|error| {
        ConduitosError::refusal("service-memory-receipt-write-failed", error.to_string())
    })?;
    let compositor_receipt = serde_json::json!({
        "schema": "conduit.conduitos/compositor-guest-evidence@2",
        "proof_class": "freestanding-emulator", "fixture": true,
        "machine": {"emulator": "qemu-system-x86_64", "model": "q35",
            "cpu": "max", "memory_mib": 128, "vcpus": 1},
        "build": build, "observations": compositor_guest,
        "transcript_sha256": super::report::sha256_file(&serial_path)?,
    });
    fs::write(
        paths.target.join("compositor-guest-fixture-proof.json"),
        serde_json::to_vec_pretty(&compositor_receipt).map_err(|error| {
            ConduitosError::refusal("compositor-guest-receipt-invalid", error.to_string())
        })?,
    )
    .map_err(|error| {
        ConduitosError::refusal("compositor-guest-receipt-write-failed", error.to_string())
    })?;
    let recovery_receipt = serde_json::json!({
        "schema": "conduit.conduitos/compositor-copy-recovery-evidence@1",
        "proof_class": "freestanding-emulator", "fixture": true,
        "machine": {"emulator": "qemu-system-x86_64", "model": "q35",
            "cpu": "max", "memory_mib": 128, "vcpus": 1},
        "build": build, "observations": sign["compositor_copy_recovery_fixture"],
        "transcript_sha256": super::report::sha256_file(&serial_path)?,
    });
    fs::write(
        paths
            .target
            .join("compositor-copy-recovery-fixture-proof.json"),
        serde_json::to_vec_pretty(&recovery_receipt).map_err(|error| {
            ConduitosError::refusal(
                "compositor-copy-recovery-receipt-invalid",
                error.to_string(),
            )
        })?,
    )
    .map_err(|error| {
        ConduitosError::refusal(
            "compositor-copy-recovery-receipt-write-failed",
            error.to_string(),
        )
    })?;
    let hostile_receipt = serde_json::json!({
        "schema": "conduit.conduitos/compositor-hostile-broker-evidence@1",
        "proof_class": "freestanding-emulator", "fixture": true,
        "machine": {"emulator": "qemu-system-x86_64", "model": "q35",
            "cpu": "max", "memory_mib": 128, "vcpus": 1},
        "build": build, "observations": sign["compositor_hostile_fixture"],
        "transcript_sha256": super::report::sha256_file(&serial_path)?,
    });
    fs::write(
        paths
            .target
            .join("compositor-hostile-broker-fixture-proof.json"),
        serde_json::to_vec_pretty(&hostile_receipt).map_err(|error| {
            ConduitosError::refusal("compositor-hostile-receipt-invalid", error.to_string())
        })?,
    )
    .map_err(|error| {
        ConduitosError::refusal("compositor-hostile-receipt-write-failed", error.to_string())
    })?;
    if !opts.quiet && !opts.json {
        println!(
            "PROVED checked ordinary text computation and service-memory/compositor guest fixtures in x86_64 CPL3"
        );
    }
    Ok(())
}
