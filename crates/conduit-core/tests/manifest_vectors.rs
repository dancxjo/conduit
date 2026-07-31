use conduit_core::{
    ArtifactDigest, ArtifactLocation, ArtifactLocationKind, ArtifactManifest, ArtifactProvenance,
    ArtifactSignature, ArtifactTrustPolicy, ArtifactVerificationReason, ExecutorKind, Id,
    ImplementationManifest, ManifestArtifactRef, ManifestEntrypoint, ManifestInterface,
    ManifestReason, PinnedDescriptor, ReplacementSupport, ReproducibilityClaim, SemanticHash,
    SignatureVerification, validate_artifact_manifest, validate_implementation_manifest,
    verify_artifact_candidate,
};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../conformance/c5/manifests.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const CONTRACT: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("fixture/node-contract"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([1; 32]),
};
const PROFILE: PinnedDescriptor<'static> = PinnedDescriptor {
    id: Id("fixture/execution-profile"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([2; 32]),
};
const ARTIFACT_DIGEST: ArtifactDigest = ArtifactDigest::from_bytes([3; 32]);
const EFFECT: SemanticHash = SemanticHash::from_bytes([5; 32]);
const AUTHORITY: SemanticHash = SemanticHash::from_bytes([9; 32]);
const ARTIFACT: ManifestArtifactRef<'static> = ManifestArtifactRef {
    id: Id("fixture/artifact"),
    digest: ARTIFACT_DIGEST,
    role: Id("executable"),
    required: true,
};
const BACKEND: ManifestInterface<'static> = ManifestInterface {
    interface: PinnedDescriptor {
        id: Id("conduit/host.clock"),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([4; 32]),
    },
    entrypoint: Id("clock-v1"),
};

fn implementation(executor: ExecutorKind) -> ImplementationManifest<'static> {
    let mut manifest = ImplementationManifest {
        schema_version: 0,
        identity: ZERO,
        id: Id(match executor {
            ExecutorKind::NativeInProcess => "fixture/native",
            ExecutorKind::WasmComponent => "fixture/wasm",
            ExecutorKind::FfiDynamicLibrary => "fixture/ffi",
            ExecutorKind::Process => "fixture/process",
            ExecutorKind::Firmware => "fixture/firmware",
            ExecutorKind::RemoteEndpoint => "fixture/remote",
        }),
        implementation_version: "1.2.3",
        semantic_contract: CONTRACT,
        executor,
        entrypoint: ManifestEntrypoint {
            name: Id("run"),
            adapter: Id("conduit-step"),
            abi: Id("component-v1"),
            protocol_version: 0,
        },
        execution_profile: PROFILE,
        artifacts: &[ARTIFACT],
        required_interfaces: &[BACKEND],
        provided_interfaces: &[BACKEND],
        required_authorities: &[AUTHORITY],
        required_effects: &[EFFECT],
        minimum_plan_version: 0,
        maximum_plan_version: 8,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 2,
        replacement: ReplacementSupport::Stateful {
            state_contract: PinnedDescriptor {
                id: Id("fixture/state"),
                schema_version: 0,
                semantic_hash: SemanticHash::from_bytes([6; 32]),
            },
            maximum_export_bytes: 1024,
            maximum_import_bytes: 1024,
            maximum_ticks: 10,
        },
        coexistence_memory_bytes: 4096,
        reproducibility: Some(ReproducibilityClaim {
            source_digest: ArtifactDigest::from_bytes([7; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([8; 32]),
            expected_artifact_digest: ARTIFACT_DIGEST,
        }),
    };
    let mut scratch = [ZERO; 8];
    manifest.identity = manifest.computed_semantic_hash(&mut scratch).unwrap();
    manifest
}

fn artifact() -> ArtifactManifest<'static> {
    const SIGNATURES: &[ArtifactSignature<'static>] = &[ArtifactSignature {
        scheme: Id("ed25519"),
        signer: Id("fixture/release"),
        signature_artifact: ArtifactDigest::from_bytes([10; 32]),
        provenance_evidence: Some(ArtifactDigest::from_bytes([11; 32])),
    }];
    const LOCATIONS: &[ArtifactLocation<'static>] = &[
        ArtifactLocation {
            kind: ArtifactLocationKind::BundlePath,
            locator: "bin/fixture",
        },
        ArtifactLocation {
            kind: ArtifactLocationKind::RemoteUri,
            locator: "https://artifacts.invalid/fixture",
        },
    ];
    let mut manifest = ArtifactManifest {
        schema_version: 0,
        identity: ZERO,
        id: Id("fixture/artifact"),
        digest: ARTIFACT_DIGEST,
        media_type: "application/wasm",
        byte_size: 12,
        target: Some(Id("wasm32-wasip2")),
        abi: Some(Id("component-v1")),
        provenance: ArtifactProvenance {
            builder: Id("fixture/builder"),
            source_digest: ArtifactDigest::from_bytes([7; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([8; 32]),
            reproducible: true,
        },
        signatures: SIGNATURES,
        license_expressions: &["Apache-2.0", "MIT"],
        notices: &[],
        sbom: Some(ManifestArtifactRef {
            id: Id("fixture/sbom"),
            digest: ArtifactDigest::from_bytes([12; 32]),
            role: Id("spdx"),
            required: true,
        }),
        source: Some(ManifestArtifactRef {
            id: Id("fixture/source"),
            digest: ArtifactDigest::from_bytes([7; 32]),
            role: Id("source"),
            required: true,
        }),
        related_artifacts: &[],
        locations: LOCATIONS,
    };
    let mut scratch = [ZERO; 8];
    manifest.identity = manifest.computed_semantic_hash(&mut scratch).unwrap();
    manifest
}

#[test]
fn one_contract_advertises_all_capability_oriented_executor_kinds() {
    let kinds = [
        ExecutorKind::NativeInProcess,
        ExecutorKind::WasmComponent,
        ExecutorKind::FfiDynamicLibrary,
        ExecutorKind::Process,
        ExecutorKind::Firmware,
        ExecutorKind::RemoteEndpoint,
    ];
    let mut identities = [ZERO; 6];
    for (index, kind) in kinds.into_iter().enumerate() {
        let manifest = implementation(kind);
        let mut scratch = [ZERO; 8];
        assert_eq!(
            validate_implementation_manifest(&manifest, &mut scratch),
            Ok(())
        );
        assert_eq!(manifest.semantic_contract, CONTRACT);
        identities[index] = manifest.identity;
    }
    assert!(identities.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn implementation_identity_entrypoint_and_rebuild_claim_fail_closed() {
    let valid = implementation(ExecutorKind::WasmComponent);
    let mut scratch = [ZERO; 8];
    assert_eq!(
        validate_implementation_manifest(&valid, &mut scratch),
        Ok(())
    );

    let missing_entrypoint = ImplementationManifest {
        entrypoint: ManifestEntrypoint {
            name: Id(""),
            ..valid.entrypoint
        },
        ..valid
    };
    assert_eq!(
        validate_implementation_manifest(&missing_entrypoint, &mut scratch),
        Err(ManifestReason::InvalidDescriptor)
    );
    let unsupported = ImplementationManifest {
        minimum_runtime_protocol: 3,
        maximum_runtime_protocol: 2,
        ..valid
    };
    assert_eq!(
        validate_implementation_manifest(&unsupported, &mut scratch),
        Err(ManifestReason::UnsupportedVersion)
    );
    let substituted = ImplementationManifest {
        artifacts: &[ManifestArtifactRef {
            digest: ArtifactDigest::from_bytes([99; 32]),
            ..ARTIFACT
        }],
        ..valid
    };
    assert_ne!(
        substituted.computed_semantic_hash(&mut scratch).unwrap(),
        valid.identity
    );
    assert_eq!(
        substituted.semantic_contract.semantic_hash,
        CONTRACT.semantic_hash
    );
    assert_eq!(
        validate_implementation_manifest(&substituted, &mut scratch),
        Err(ManifestReason::InvalidDescriptor)
    );
}

#[test]
fn artifact_policy_reports_integrity_target_abi_license_sbom_and_signature() {
    let manifest = artifact();
    let mut scratch = [ZERO; 8];
    assert_eq!(validate_artifact_manifest(&manifest, &mut scratch), Ok(()));
    let alternate_location = [ArtifactLocation {
        kind: ArtifactLocationKind::RemoteUri,
        locator: "https://mirror.invalid/fixture",
    }];
    let relocated = ArtifactManifest {
        locations: &alternate_location,
        ..manifest
    };
    assert_eq!(
        relocated.computed_semantic_hash(&mut scratch).unwrap(),
        manifest.identity
    );
    let policy = ArtifactTrustPolicy {
        require_signature: true,
        require_provenance_evidence: true,
        require_known_license: true,
        require_sbom: true,
        trusted_signers: &[Id("fixture/release")],
    };
    let verified = [SignatureVerification {
        signer: Id("fixture/release"),
        scheme: Id("ed25519"),
        verified: true,
        verifier: Id("fixture/verifier"),
        evidence_digest: ArtifactDigest::from_bytes([11; 32]),
    }];
    let check = |digest, size, target, abi, policy, signatures| {
        verify_artifact_candidate(&manifest, digest, size, target, abi, policy, signatures)
    };
    assert_eq!(
        check(
            ARTIFACT_DIGEST,
            12,
            Some(Id("wasm32-wasip2")),
            Some(Id("component-v1")),
            policy,
            &verified
        ),
        Ok(())
    );
    assert_eq!(
        check(
            ArtifactDigest::from_bytes([90; 32]),
            12,
            Some(Id("wasm32-wasip2")),
            Some(Id("component-v1")),
            policy,
            &verified
        ),
        Err(ArtifactVerificationReason::DigestMismatch)
    );
    assert_eq!(
        check(
            ARTIFACT_DIGEST,
            13,
            Some(Id("wasm32-wasip2")),
            Some(Id("component-v1")),
            policy,
            &verified
        ),
        Err(ArtifactVerificationReason::SizeMismatch)
    );
    assert_eq!(
        check(
            ARTIFACT_DIGEST,
            12,
            Some(Id("x86_64-unknown-linux-gnu")),
            Some(Id("component-v1")),
            policy,
            &verified
        ),
        Err(ArtifactVerificationReason::WrongTarget)
    );
    assert_eq!(
        check(
            ARTIFACT_DIGEST,
            12,
            Some(Id("wasm32-wasip2")),
            Some(Id("component-v2")),
            policy,
            &verified
        ),
        Err(ArtifactVerificationReason::UnsupportedAbi)
    );
    let invalid = [SignatureVerification {
        verified: false,
        ..verified[0]
    }];
    assert_eq!(
        check(
            ARTIFACT_DIGEST,
            12,
            Some(Id("wasm32-wasip2")),
            Some(Id("component-v1")),
            policy,
            &invalid
        ),
        Err(ArtifactVerificationReason::SignatureInvalid)
    );

    let unknown_license = ArtifactManifest {
        license_expressions: &[],
        ..manifest
    };
    assert_eq!(
        verify_artifact_candidate(
            &unknown_license,
            ARTIFACT_DIGEST,
            12,
            Some(Id("wasm32-wasip2")),
            Some(Id("component-v1")),
            policy,
            &verified
        ),
        Err(ArtifactVerificationReason::LicenseRequired)
    );
    let no_sbom = ArtifactManifest {
        sbom: None,
        ..manifest
    };
    assert_eq!(
        verify_artifact_candidate(
            &no_sbom,
            ARTIFACT_DIGEST,
            12,
            Some(Id("wasm32-wasip2")),
            Some(Id("component-v1")),
            policy,
            &verified
        ),
        Err(ArtifactVerificationReason::SbomRequired)
    );
}

#[test]
fn c5_fixture_freezes_required_manifest_and_verification_boundaries() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["suite"], "conduit.manifests");
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 27);
    for required in [
        "same-contract-native-wasm-ffi",
        "firmware-entrypoint-and-abi",
        "remote-provided-backend-interface",
        "digest-mismatch-before-load",
        "wrong-target",
        "missing-entrypoint",
        "unsupported-abi",
        "unknown-license-policy",
        "signature-valid-under-explicit-policy",
        "signature-invalid",
        "reproducible-rebuild-match",
        "artifact-substitution-changes-implementation-and-plan",
        "license-provenance-inspection-no-execution",
    ] {
        assert!(
            cases.iter().any(|case| case["id"] == required),
            "missing {required}"
        );
    }
}
