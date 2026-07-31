use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use conduit_package::{
    PackageLimits, PackageManifest, PackageObject, PackageSignatureObservation, PackageTrustPolicy,
    decode_package, encode_package,
};
use sha2::{Digest as _, Sha256};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduct"));
    for variable in [
        "NO_COLOR",
        "CLICOLOR",
        "CLICOLOR_FORCE",
        "TERM",
        "CI",
        "COLUMNS",
    ] {
        command.env_remove(variable);
    }
    command
}

fn temporary_directory() -> PathBuf {
    let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "conduct-package-cli-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[test]
fn create_inspect_and_extract_keep_results_and_diagnostics_separate() {
    let root = temporary_directory();
    let blob = b"not an executable operation".to_vec();
    let digest = digest(&blob);
    let blob_path = root.join("blob.bin");
    std::fs::write(&blob_path, &blob).unwrap();
    let mut manifest = PackageManifest::new(vec![PackageObject {
        digest: digest.clone(),
        media_type: "application/octet-stream".to_owned(),
        byte_size: blob.len() as u64,
        role: "linux-native".to_owned(),
        embedded: true,
        identity: None,
        license_expressions: vec!["MIT".to_owned()],
        license_objects: Vec::new(),
        sbom: None,
        signatures: Vec::new(),
        attestations: Vec::new(),
        provenance: None,
        retrieval_hints: Vec::new(),
    }]);
    manifest.seal().unwrap();
    let manifest_path = root.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let package_path = root.join("bundle.cndpkg");

    let created = command()
        .args(["package", "create", "--format=json", "--manifest"])
        .arg(&manifest_path)
        .arg("--blob")
        .arg(format!("{digest}={}", blob_path.display()))
        .arg("--output")
        .arg(&package_path)
        .output()
        .unwrap();
    assert!(created.status.success());
    assert!(created.stderr.is_empty());
    let result: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(result["schema"], "conduit.result");
    assert_eq!(result["operation"], "package-create");
    assert_eq!(result["result"]["identity"], manifest.identity);
    let decoded = decode_package(
        &std::fs::read(&package_path).unwrap(),
        PackageLimits::default(),
    )
    .unwrap();
    assert_eq!(
        decoded.embedded_blobs,
        BTreeMap::from([(digest.clone(), blob)])
    );

    let inspected = command()
        .args(["inspect", "--type=package", "--format=json"])
        .arg(&package_path)
        .output()
        .unwrap();
    assert!(inspected.status.success());
    assert!(inspected.stderr.is_empty());
    let result: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(result["result"]["kind"], "package");
    assert_eq!(result["result"]["counts"]["embedded_objects"], 1);

    let extraction = root.join("extracted");
    let extracted = command()
        .args(["package", "extract", "--format=json"])
        .arg(&package_path)
        .arg("--output-dir")
        .arg(&extraction)
        .output()
        .unwrap();
    assert!(extracted.status.success());
    assert!(extracted.stderr.is_empty());
    let result: serde_json::Value = serde_json::from_slice(&extracted.stdout).unwrap();
    assert_eq!(result["operation"], "package-extract");
    assert_eq!(result["result"]["extracted_objects"], 1);
    assert!(
        extraction
            .join("blobs/sha256")
            .join(digest.strip_prefix("sha256:").unwrap())
            .is_file()
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn package_rejections_emit_only_structured_stderr() {
    let root = temporary_directory();
    let package = root.join("bad.cndpkg");
    std::fs::write(&package, b"CNDPKG1\n\0\0\0\xff").unwrap();
    let output = command()
        .args([
            "package",
            "extract",
            "--format=json",
            "--diagnostic-format=json",
        ])
        .arg(&package)
        .arg("--output-dir")
        .arg(root.join("output"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-PKG-008");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn verify_requires_explicit_trusted_signature_observation() {
    let root = temporary_directory();
    let payload_bytes = b"payload";
    let signature_bytes = b"signature";
    let evidence_bytes = b"verification receipt";
    let payload_digest = digest(payload_bytes);
    let signature_digest = digest(signature_bytes);
    let evidence_digest = digest(evidence_bytes);
    let mut manifest = PackageManifest::new(vec![
        PackageObject {
            digest: payload_digest.clone(),
            media_type: "application/octet-stream".to_owned(),
            byte_size: payload_bytes.len() as u64,
            role: "linux-native".to_owned(),
            embedded: true,
            identity: None,
            license_expressions: vec!["MIT".to_owned()],
            license_objects: Vec::new(),
            sbom: None,
            signatures: vec![signature_digest.clone()],
            attestations: vec![evidence_digest.clone()],
            provenance: None,
            retrieval_hints: Vec::new(),
        },
        PackageObject {
            digest: signature_digest.clone(),
            media_type: "application/octet-stream".to_owned(),
            byte_size: signature_bytes.len() as u64,
            role: "signature".to_owned(),
            embedded: true,
            identity: None,
            license_expressions: Vec::new(),
            license_objects: Vec::new(),
            sbom: None,
            signatures: Vec::new(),
            attestations: Vec::new(),
            provenance: None,
            retrieval_hints: Vec::new(),
        },
        PackageObject {
            digest: evidence_digest.clone(),
            media_type: "application/json".to_owned(),
            byte_size: evidence_bytes.len() as u64,
            role: "attestation".to_owned(),
            embedded: true,
            identity: None,
            license_expressions: Vec::new(),
            license_objects: Vec::new(),
            sbom: None,
            signatures: Vec::new(),
            attestations: Vec::new(),
            provenance: None,
            retrieval_hints: Vec::new(),
        },
    ]);
    manifest.seal().unwrap();
    let package = encode_package(
        &manifest,
        &BTreeMap::from([
            (payload_digest.clone(), payload_bytes.to_vec()),
            (signature_digest.clone(), signature_bytes.to_vec()),
            (evidence_digest.clone(), evidence_bytes.to_vec()),
        ]),
        PackageLimits::default(),
    )
    .unwrap();
    let package_path = root.join("bundle.cndpkg");
    std::fs::write(&package_path, package).unwrap();
    let policy_path = root.join("policy.json");
    std::fs::write(
        &policy_path,
        serde_json::to_vec(&PackageTrustPolicy {
            roles: vec!["linux-native".to_owned()],
            require_known_license: true,
            require_sbom: false,
            require_signature: true,
            require_attestation: true,
            require_provenance: false,
            trusted_signers: vec!["fixture/signer".to_owned()],
        })
        .unwrap(),
    )
    .unwrap();
    let observations_path = root.join("observations.json");
    let observation = PackageSignatureObservation {
        object_digest: payload_digest,
        signature_digest,
        signer: "fixture/signer".to_owned(),
        scheme: "fixture/signature-v1".to_owned(),
        verifier: "fixture/verifier".to_owned(),
        verified: true,
        evidence_digest,
    };
    std::fs::write(
        &observations_path,
        serde_json::to_vec(&[&observation]).unwrap(),
    )
    .unwrap();

    let accepted = command()
        .args(["package", "verify", "--format=json"])
        .arg(&package_path)
        .arg("--policy")
        .arg(&policy_path)
        .arg("--observations")
        .arg(&observations_path)
        .output()
        .unwrap();
    assert!(accepted.status.success());
    assert!(accepted.stderr.is_empty());
    let result: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(result["operation"], "package-verify");
    assert_eq!(result["result"]["selected_objects"], 1);
    assert_eq!(result["result"]["verified_observations"], 1);

    let mut rejected = observation;
    rejected.verified = false;
    std::fs::write(&observations_path, serde_json::to_vec(&[rejected]).unwrap()).unwrap();
    let rejected = command()
        .args([
            "package",
            "verify",
            "--format=json",
            "--diagnostic-format=json",
        ])
        .arg(&package_path)
        .arg("--policy")
        .arg(&policy_path)
        .arg("--observations")
        .arg(&observations_path)
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    assert!(rejected.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-PKG-006");

    std::fs::remove_dir_all(root).unwrap();
}
