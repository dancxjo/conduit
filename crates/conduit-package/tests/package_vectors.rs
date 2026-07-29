use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use conduit_inspect::{InspectLimits, RequestedKind, inspect_bytes};
use conduit_package::{
    PackageLimits, PackageManifest, PackageObject, PackageObjectIdentity,
    PackageSignatureObservation, PackageTrustPolicy, decode_package, encode_package,
    validate_package_trust,
};
use sha2::{Digest as _, Sha256};

const FIXTURE: &str = include_str!("../../../conformance/c5/compile-package-v1.json");
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn object(role: &str, bytes: &[u8], embedded: bool) -> PackageObject {
    PackageObject {
        digest: digest(bytes),
        media_type: "application/octet-stream".to_owned(),
        byte_size: bytes.len() as u64,
        role: role.to_owned(),
        embedded,
        identity: None,
        license_expressions: vec!["MIT".to_owned()],
        license_objects: Vec::new(),
        sbom: None,
        signatures: Vec::new(),
        attestations: Vec::new(),
        provenance: None,
        retrieval_hints: if embedded {
            Vec::new()
        } else {
            vec!["https://invalid.example/object".to_owned()]
        },
    }
}

fn sealed(objects: Vec<PackageObject>) -> PackageManifest {
    let mut manifest = PackageManifest::new(objects);
    manifest.seal().unwrap();
    manifest
}

fn package_case(id: &str) {
    match id {
        "thick-package-round-trip" => {
            let bytes = b"thick";
            let object = object("linux-native", bytes, true);
            let manifest = sealed(vec![object.clone()]);
            let encoded = encode_package(
                &manifest,
                &BTreeMap::from([(object.digest, bytes.to_vec())]),
                PackageLimits::default(),
            )
            .unwrap();
            let decoded = decode_package(&encoded, PackageLimits::default()).unwrap();
            assert_eq!(decoded.manifest, manifest);
            assert_eq!(decoded.embedded_blobs.len(), 1);
        }
        "thin-package-round-trip" => {
            let object = object("wasm-component", b"thin", false);
            let manifest = sealed(vec![object]);
            let encoded =
                encode_package(&manifest, &BTreeMap::new(), PackageLimits::default()).unwrap();
            let decoded = decode_package(&encoded, PackageLimits::default()).unwrap();
            assert_eq!(decoded.manifest, manifest);
            assert!(decoded.embedded_blobs.is_empty());
        }
        "heterogeneous-linux-wasm-pico-package" => {
            let objects = [
                ("linux-native", b"linux".as_slice()),
                ("wasm-component", b"wasm".as_slice()),
                ("embedded-firmware", b"pico".as_slice()),
            ]
            .map(|(role, bytes)| object(role, bytes, false));
            let manifest = sealed(objects.into());
            let encoded =
                encode_package(&manifest, &BTreeMap::new(), PackageLimits::default()).unwrap();
            let decoded = decode_package(&encoded, PackageLimits::default()).unwrap();
            assert_eq!(
                decoded
                    .manifest
                    .objects
                    .iter()
                    .map(|object| object.role.as_str())
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["embedded-firmware", "linux-native", "wasm-component"])
            );
        }
        "missing-tampered-or-oversized-blob" => {
            let object = object("linux-native", b"expected", true);
            let manifest = sealed(vec![object.clone()]);
            assert_eq!(
                encode_package(
                    &manifest,
                    &BTreeMap::from([(object.digest, b"tampered".to_vec())]),
                    PackageLimits::default()
                )
                .unwrap_err()
                .code(),
                "CND-PKG-005"
            );
        }
        "license-sbom-signature-provenance-mismatch" => {
            let object = object("linux-native", b"payload", false);
            let manifest = sealed(vec![object]);
            let policy = PackageTrustPolicy {
                roles: vec!["linux-native".to_owned()],
                require_known_license: true,
                require_sbom: true,
                require_signature: true,
                require_attestation: true,
                require_provenance: true,
                trusted_signers: vec!["fixture/signer".to_owned()],
            };
            assert_eq!(
                validate_package_trust(
                    &manifest,
                    &policy,
                    &[] as &[PackageSignatureObservation],
                    PackageLimits::default()
                )
                .unwrap_err()
                .code(),
                "CND-PKG-006"
            );
        }
        "unsupported-media-plan-package-version" => {
            let mut manifest = sealed(vec![object("linux-native", b"payload", false)]);
            manifest.schema_version = 2;
            assert_eq!(
                manifest
                    .validate(PackageLimits::default())
                    .unwrap_err()
                    .code(),
                "CND-PKG-001"
            );
        }
        "extraction-limit-and-traversal-attacks" => {
            let bytes = b"bounded";
            let object = object("linux-native", bytes, true);
            let manifest = sealed(vec![object.clone()]);
            let encoded = encode_package(
                &manifest,
                &BTreeMap::from([(object.digest, bytes.to_vec())]),
                PackageLimits::default(),
            )
            .unwrap();
            let limits = PackageLimits {
                maximum_extracted_bytes: 1,
                ..PackageLimits::default()
            };
            assert_eq!(
                decode_package(&encoded, limits).unwrap_err().code(),
                "CND-PKG-007"
            );

            let decoded = decode_package(&encoded, PackageLimits::default()).unwrap();
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "conduit-package-vector-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("blobs"), b"not a directory").unwrap();
            assert_eq!(
                decoded
                    .extract_to(&root, PackageLimits::default())
                    .unwrap_err()
                    .code(),
                "CND-PKG-009"
            );
            std::fs::remove_dir_all(root).unwrap();
        }
        "identity-preserved-across-package-round-trip" => {
            let mut object = object("execution-plan", b"plan", false);
            object.identity = Some(PackageObjectIdentity {
                kind: "execution-plan".to_owned(),
                schema_version: 2,
                semantic_identity: digest(b"semantic plan"),
            });
            let manifest = sealed(vec![object]);
            let encoded =
                encode_package(&manifest, &BTreeMap::new(), PackageLimits::default()).unwrap();
            let decoded = decode_package(&encoded, PackageLimits::default()).unwrap();
            assert_eq!(decoded.manifest, manifest);
            assert_ne!(
                decoded.manifest.identity,
                decoded.manifest.objects[0]
                    .identity
                    .as_ref()
                    .unwrap()
                    .semantic_identity
            );
        }
        "package-inspection-never-executes" => {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let marker = std::env::temp_dir().join(format!(
                "conduit-package-never-executes-{}-{sequence}",
                std::process::id()
            ));
            let script = format!("#!/bin/sh\ntouch {}\n", marker.display());
            let object = object("linux-native", script.as_bytes(), true);
            let manifest = sealed(vec![object.clone()]);
            let encoded = encode_package(
                &manifest,
                &BTreeMap::from([(object.digest, script.into_bytes())]),
                PackageLimits::default(),
            )
            .unwrap();
            let report = inspect_bytes(
                &encoded,
                RequestedKind::Package,
                None,
                InspectLimits::default(),
            )
            .unwrap();
            assert_eq!(report.identity.as_deref(), Some(manifest.identity.as_str()));
            assert!(!marker.exists());
        }
        other => panic!("unhandled package vector {other}"),
    }
}

#[test]
fn every_package_vector_executes_independently() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let package_ids = cases
        .iter()
        .filter(|case| case["runner"] == "package")
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(package_ids.len(), 9);
    assert_eq!(
        package_ids.iter().copied().collect::<BTreeSet<_>>().len(),
        package_ids.len()
    );
    for id in package_ids {
        package_case(id);
    }
}
