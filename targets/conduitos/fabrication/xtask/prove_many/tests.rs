use std::{fs, path::Path};

use clap::Parser;

use crate::cli::Cli;

use super::*;

#[test]
fn bounded_selection_refuses_duplicates() {
    let error = validated_proofs(&[X86Proof::Kernel, X86Proof::Kernel]).unwrap_err();
    assert!(error.to_string().contains("proof-batch-duplicate"));
}

#[test]
fn cli_accepts_exact_selected_proofs_and_parallel_bound() {
    let parsed = Cli::try_parse_from([
        "xtask",
        "conduitos",
        "prove-many",
        "--proof",
        "kernel",
        "--proof",
        "xhci",
        "--max-parallel",
        "2",
    ]);
    assert!(parsed.is_ok(), "{parsed:?}");
}

#[test]
fn every_proof_has_one_explicit_command() {
    let evidence = Path::new("evidence");
    for proof in [
        X86Proof::Kernel,
        X86Proof::Xhci,
        X86Proof::Usb,
        X86Proof::Hid,
        X86Proof::Keyboard,
        X86Proof::FrontDoor,
        X86Proof::ProductJourney,
        X86Proof::Rescue,
    ] {
        let command = proof.arguments(evidence);
        assert_eq!(command.first().map(String::as_str), Some("conduitos"));
        assert_eq!(command.last().map(String::as_str), Some("--locked"));
    }
}

#[test]
fn kernel_verifier_binds_the_exact_commit_and_evidence_root() {
    let command = kernel_verification_arguments(
        Path::new("batch/runs/kernel/conduitos"),
        "0123456789012345678901234567890123456789",
    );
    assert!(command
        .windows(2)
        .any(|pair| { pair == ["--commit", "0123456789012345678901234567890123456789"] }));
    assert!(command
        .windows(2)
        .any(|pair| pair == ["--root", "batch/runs/kernel/evidence"]));
}

#[test]
fn a_nonempty_batch_root_is_refused_without_deleting_history() {
    let directory = std::env::temp_dir().join(format!(
        "conduit-proof-batch-root-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("old-receipt.json"), b"historical").unwrap();
    let error = refuse_nonempty_root(&directory).unwrap_err();
    assert!(error.to_string().contains("proof-batch-root-not-empty"));
    assert_eq!(
        fs::read(directory.join("old-receipt.json")).unwrap(),
        b"historical"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn one_failure_does_not_erase_a_completed_sibling_result() {
    let result = |proof, status| ProofResult {
        schema: "conduit.conduitos.prove-many-result/v1",
        proof,
        status,
        command: Vec::new(),
        verification_command: None,
        isolated_target_root: String::new(),
        stdout_log: String::new(),
        stderr_log: String::new(),
        started_order: 1,
        finished_order: 1,
    };
    let results = [
        result(X86Proof::Usb, "failure"),
        result(X86Proof::Keyboard, "success"),
    ];
    assert_eq!(failure_names(&results), ["usb"]);
    assert_eq!(results[1].status, "success");
}
