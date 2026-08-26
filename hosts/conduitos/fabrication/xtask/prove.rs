use std::{fs, path::Path};

use crate::{
    cli::GlobalOpts,
    evidence::{
        EvidenceKind, EvidenceManifest, EvidenceOutput, EvidenceProvenance, EvidenceResult,
    },
};

use super::{
    build, image,
    profile::{Paths, LIMINE_ARCHIVE_SHA256, LIMINE_VERSION, QEMU_PROFILE},
    report::{git_head, ProofRecord},
    run, ConduitosArch, ConduitosError,
};

const CONSOLE_EVIDENCE_MAX_BYTES: u64 = 256 * 1024;

pub fn execute(
    arch: ConduitosArch,
    evidence_root: Option<&Path>,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    if arch == ConduitosArch::Ia32 {
        return super::ia32_a1::prove(opts);
    }
    if arch == ConduitosArch::Aarch64 {
        return super::aarch64_a1::prove(opts);
    }
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-proof",
            "prove --dry-run cannot manufacture QEMU evidence",
        ));
    }
    let paths = Paths::new(arch)?;
    let image = image::execute_proof(arch, opts)?;
    let rebuilt_image = image::execute_proof(arch, opts)?;
    let reproducible_image = image.iso_sha256 == rebuilt_image.iso_sha256;
    if !reproducible_image {
        return Err(ConduitosError::refusal(
            "non-reproducible-image",
            format!(
                "identical inputs produced {} then {}",
                image.iso_sha256, rebuilt_image.iso_sha256
            ),
        ));
    }
    let first = run::boot_once(&paths, opts)?;
    let second = run::boot_once(&paths, opts)?;
    let fresh_host_id = first.boot.host_id != second.boot.host_id;
    let fresh_boot_id = first.boot.boot_id != second.boot.boot_id;
    if !fresh_host_id || !fresh_boot_id {
        return Err(ConduitosError::refusal(
            "stale-boot-identity",
            "two independent QEMU boots reused HostId or BootId",
        ));
    }
    let stable_semantic_identities = first.kernel.source_document_id
        == second.kernel.source_document_id
        && first.kernel.checked_form_id == second.kernel.checked_form_id
        && first.kernel.expanded_form_id == second.kernel.expanded_form_id
        && first.presentation.source_document_id == second.presentation.source_document_id
        && first.presentation.checked_form_id == second.presentation.checked_form_id
        && first.presentation.expanded_form_id == second.presentation.expanded_form_id;
    let fresh_realization_identities = first.kernel.plan_id != second.kernel.plan_id
        && first.kernel.fragment_id != second.kernel.fragment_id
        && first.kernel.active_play_id != second.kernel.active_play_id
        && first.presentation.plan_id != second.presentation.plan_id
        && first.presentation.fragment_id != second.presentation.fragment_id
        && first.presentation.display_base_id != second.presentation.display_base_id;
    if !stable_semantic_identities || !fresh_realization_identities {
        return Err(ConduitosError::refusal(
            "observatory-identity-stage-collapse",
            "independent boots did not preserve semantic identities and refresh realization identities",
        ));
    }
    let base_commit = git_head(&paths.root)?;
    if arch == ConduitosArch::X86_64 {
        let expected = build::proof_manifest(arch)?;
        if first.boot.profile_id != expected.profile_id
            || second.boot.profile_id != expected.profile_id
            || first.boot.build_id != expected.build_id
            || second.boot.build_id != expected.build_id
            || first.boot.image_binding != expected.image_id
            || second.boot.image_binding != expected.image_id
        {
            return Err(ConduitosError::refusal(
                "stale-build-identity",
                "guest fabrication identity did not match the checked proof PROFILE",
            ));
        }
    } else {
        let expected_image_id = format!("conduitos-image/{base_commit}/{}/v1", arch.as_str());
        if first.boot.build_id != base_commit
            || second.boot.build_id != base_commit
            || first.boot.image_binding != expected_image_id
            || second.boot.image_binding != expected_image_id
        {
            return Err(ConduitosError::refusal(
                "stale-build-identity",
                "guest build/image identity did not match the exact checkout",
            ));
        }
    }
    let qemu_version = qemu_version(&paths)?;
    let first_serial = first.serial;
    let mut proof = ProofRecord {
        schema: "conduit.conduitos.observatory-proof/v1",
        base_commit,
        architecture: arch.as_str(),
        proof_class: "freestanding-emulator",
        limine_version: LIMINE_VERSION,
        limine_archive_sha256: LIMINE_ARCHIVE_SHA256,
        qemu_profile: QEMU_PROFILE,
        qemu_version,
        iso_sha256: image.iso_sha256,
        reproducible_image,
        first_boot: first.boot,
        first_presentation: first.presentation,
        first_kernel: first.kernel,
        first_observatory: first.observatory.clone(),
        second_boot: second.boot,
        second_presentation: second.presentation,
        second_kernel: second.kernel,
        second_observatory: second.observatory,
        fresh_host_id,
        fresh_boot_id,
        stable_semantic_identities,
        fresh_realization_identities,
        native_patchbay_consumed: false,
        native_patchbay_linear_lines: 0,
    };
    let snapshot = serde_json::to_vec_pretty(&first.observatory)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.observatory_snapshot, snapshot)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    proof.native_patchbay_linear_lines = prove_native_patchbay(&paths, &proof)?;
    proof.native_patchbay_consumed = true;
    let bytes = serde_json::to_vec_pretty(&proof)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    fs::write(&paths.proof, bytes)
        .map_err(|error| ConduitosError::refusal("proof-record-failed", error.to_string()))?;
    if let Some(root) = evidence_root {
        emit_console_evidence(root, &paths, &proof, &first_serial)?;
    }
    if opts.json {
        println!(
            "{}",
            serde_json::to_string(&proof).map_err(|error| {
                ConduitosError::refusal("proof-record-failed", error.to_string())
            })?
        );
    } else if !opts.quiet {
        println!(
            "ConduitOS P5 Observatory/Patchbay proof: {}\nConduitOS Observatory snapshot: {}",
            paths.proof.display(),
            paths.observatory_snapshot.display()
        );
    }
    Ok(())
}

fn emit_console_evidence(
    root: &Path,
    paths: &Paths,
    proof: &ProofRecord,
    serial: &str,
) -> Result<(), ConduitosError> {
    if serial.len() as u64 > CONSOLE_EVIDENCE_MAX_BYTES {
        return Err(ConduitosError::refusal(
            "console-evidence-too-large",
            format!(
                "validated serial transcript has {} bytes; maximum is {CONSOLE_EVIDENCE_MAX_BYTES}",
                serial.len()
            ),
        ));
    }
    let mut evidence = EvidenceManifest::new(
        root,
        &paths.root,
        "conduitos-x86_64",
        "conduitos.prove.x86_64",
    )
    .map_err(|error| ConduitosError::refusal("console-evidence-failed", error))?;
    let relative = Path::new("x86_64-console.txt");
    fs::write(evidence.root().join(relative), serial)
        .map_err(|error| ConduitosError::refusal("console-evidence-failed", error.to_string()))?;
    evidence
        .declare(EvidenceOutput {
            id: "conduitos.x86_64.console".into(),
            kind: EvidenceKind::ConsoleTranscript,
            path: relative.into(),
            media_type: "text/plain; charset=utf-8".into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "conduitos.x86_64.p5-console@1".into(),
                step_id: Some("conduitos.prove.x86_64.semantic-terminal".into()),
                plan_id: Some(proof.first_kernel.plan_id.clone()),
                active_play_id: Some(proof.first_kernel.active_play_id.clone()),
                asserted_semantic_disposition: Some(
                    "validated-boot-kernel-observatory-and-terminal-debug-exit".into(),
                ),
                proof_class: Some(proof.proof_class.into()),
                architecture: Some(proof.architecture.into()),
                architecture_rung: Some("conduitos/x86_64/P5-observatory-patchbay".into()),
                emulator: Some("qemu-system-x86_64".into()),
                emulator_version: Some(proof.qemu_version.clone()),
                machine: Some(proof.qemu_profile.into()),
                firmware: Some(proof.first_boot.firmware.clone()),
                host_id: Some(proof.first_boot.host_id.clone()),
                boot_id: Some(proof.first_boot.boot_id.clone()),
                kernel_artifact_id: Some(format!("conduitos-build/{}", proof.base_commit)),
                kernel_artifact_sha256: Some(super::report::sha256_file(&paths.kernel)?),
                capture_trigger: Some(
                    "semantic-result-and-structured-terminal-signs-validated".into(),
                ),
                capture_byte_limit: Some(CONSOLE_EVIDENCE_MAX_BYTES),
                image_width: None,
                image_height: None,
                physical_evidence: Some(false),
                ..Default::default()
            },
        })
        .map_err(|error| ConduitosError::refusal("console-evidence-failed", error))?;
    evidence
        .finish(EvidenceResult::Complete)
        .map_err(|error| ConduitosError::refusal("console-evidence-failed", error))
}

fn prove_native_patchbay(paths: &Paths, proof: &ProofRecord) -> Result<usize, ConduitosError> {
    let snapshot_path = paths
        .observatory_snapshot
        .to_str()
        .ok_or_else(|| ConduitosError::refusal("patchbay-rejected-report", "non-UTF-8 path"))?;
    let output = super::profile::command(
        "cargo",
        &[
            "run",
            "--quiet",
            "-p",
            "patchbay-native",
            "--",
            "--linear-observatory-snapshot",
            snapshot_path,
        ],
        &paths.root,
        "patchbay-rejected-report",
    )?;
    let linear = String::from_utf8(output.stdout)
        .map_err(|error| ConduitosError::refusal("patchbay-rejected-report", error.to_string()))?;
    for required in [
        proof.first_boot.host_id.as_str(),
        proof.first_boot.boot_id.as_str(),
        proof.first_kernel.plan_id.as_str(),
        proof.first_kernel.fragment_id.as_str(),
        proof.first_kernel.active_play_id.as_str(),
        proof.first_kernel.source_document_id.as_str(),
        proof.first_kernel.checked_form_id.as_str(),
        proof.first_kernel.expanded_form_id.as_str(),
        "BASES 8",
        "kind=conduitos.base/framebuffer@1",
        "SIGNS 19",
        "items=1 bytes=64",
        "implementation=conduitos/kernel-time-tick@1",
        "implementation=conduitos/kernel-serial-tick@1",
        "implementation=conduitos/kernel-text-literal@1",
        "implementation=conduitos/kernel-text-upper@1",
        "implementation=conduitos/kernel-serial-text@1",
        "profile=conduitos/cooperative-bounded-step@1",
        "REGION region/text",
        "REGION region/timer",
        "ExecutionRegionOverlap",
        "runtime-memory=12288",
        "runtime-memory=8192",
        "timer-slots=0",
        "timer-slots=1",
        "lifecycle=Completed",
        "visible_gaps=0",
        "history=current",
        "history=historical",
        "BOOT PROVENANCE [SEALED] 1",
        "proof=FreestandingEmulator",
    ] {
        if !linear.contains(required) {
            return Err(ConduitosError::refusal(
                "patchbay-linear-projection-incomplete",
                format!("native Patchbay output omitted {required}"),
            ));
        }
    }
    Ok(linear.lines().count())
}

fn qemu_version(paths: &Paths) -> Result<String, ConduitosError> {
    let output = super::profile::command(
        "qemu-system-x86_64",
        &["--version"],
        &paths.root,
        "missing-qemu",
    )?;
    String::from_utf8(output.stdout)
        .map(|value| value.lines().next().unwrap_or_default().to_owned())
        .map_err(|error| ConduitosError::refusal("missing-qemu", error.to_string()))
}
