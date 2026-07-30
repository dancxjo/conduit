use std::collections::BTreeMap;

use conduit_core::{
    ArtifactDigest, ArtifactManifest, ArtifactProvenance, ArtifactTrustPolicy, Id, SemanticHash,
};
use conduit_package::{
    PACKAGE_MAGIC, PackageLimits, PackageManifest, decode_package, encode_package,
};
use conduit_panel::{
    MAXIMUM_PANEL_SOURCE_BYTES, MAXIMUM_PANEL_TOKENS, MAXIMUM_SOURCE_VALUE_DEPTH, parse,
};
use conduit_runtime::{
    EvidenceDecodeLimits, HostedArtifactVerificationError, NdjsonError, NdjsonLimit,
    decode_event_ndjson_with_limits, verify_artifact_owned, verify_artifact_owned_evidenced,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

const FIXTURE: &str = include_str!("../../../conformance/c5/security-boundaries-v1.json");
const EVENT: &str = include_str!("../../../conformance/c2/execution-event-v1.ndjson");

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    operation: String,
    expected: Value,
}

fn policy() -> ArtifactTrustPolicy<'static> {
    ArtifactTrustPolicy {
        require_signature: false,
        require_provenance_evidence: false,
        require_known_license: false,
        require_sbom: false,
        trusted_signers: &[],
    }
}

fn artifact_manifest(bytes: &[u8]) -> ArtifactManifest<'static> {
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&Sha256::digest(bytes));
    let mut manifest = ArtifactManifest {
        schema_version: 1,
        identity: SemanticHash::from_bytes([0; 32]),
        id: Id("fixture/security-blob"),
        digest: ArtifactDigest::from_bytes(digest),
        media_type: "application/octet-stream",
        byte_size: bytes.len() as u64,
        target: None,
        abi: None,
        provenance: ArtifactProvenance {
            builder: Id("fixture/builder"),
            source_digest: ArtifactDigest::from_bytes([1; 32]),
            build_recipe_digest: ArtifactDigest::from_bytes([2; 32]),
            reproducible: true,
        },
        signatures: &[],
        license_expressions: &[],
        notices: &[],
        sbom: None,
        source: None,
        related_artifacts: &[],
        locations: &[],
    };
    let mut scratch = [];
    manifest.identity = manifest.computed_semantic_hash(&mut scratch).unwrap();
    manifest
}

fn first_event() -> Value {
    serde_json::from_str(EVENT.lines().next().unwrap()).unwrap()
}

fn evidence_result(id: &str) -> Value {
    let mut event = first_event();
    let mut limits = EvidenceDecodeLimits::default();
    let input = match id {
        "oversized-evidence-stream" => {
            let input = serde_json::to_string(&event).unwrap();
            limits.maximum_input_bytes = input.len() - 1;
            input
        }
        "oversized-evidence-record" => {
            let input = serde_json::to_string(&event).unwrap();
            limits.maximum_record_bytes = input.len() - 1;
            input
        }
        "evidence-record-flood" => {
            limits.maximum_records = 1;
            let line = serde_json::to_string(&event).unwrap();
            format!("{line}\n{line}\n")
        }
        "oversized-inline-evidence-payload" => {
            limits.maximum_inline_payload_bytes = 4;
            serde_json::to_string(&event).unwrap()
        }
        "evidence-derivation-flood" => {
            limits.maximum_derivations = 0;
            event["relations"]["derived_from"] = json!(["event/source"]);
            serde_json::to_string(&event).unwrap()
        }
        "oversized-evidence-string" => {
            limits.maximum_string_bytes = 3;
            serde_json::to_string(&event).unwrap()
        }
        "malformed-evidence-json" => "{".to_owned(),
        _ => panic!("unknown evidence case {id}"),
    };
    match decode_event_ndjson_with_limits(&input, limits) {
        Ok(_) => json!({"accepted": true}),
        Err(NdjsonError::LimitExceeded(limit)) => {
            json!({"accepted": false, "limit": limit_name(limit)})
        }
        Err(NdjsonError::Json(_)) => json!({"accepted": false, "reason": "json"}),
        Err(error) => panic!("unexpected evidence error for {id}: {error}"),
    }
}

const fn limit_name(limit: NdjsonLimit) -> &'static str {
    match limit {
        NdjsonLimit::InputBytes => "input-bytes",
        NdjsonLimit::RecordBytes => "record-bytes",
        NdjsonLimit::Records => "records",
        NdjsonLimit::InlinePayloadBytes => "inline-payload-bytes",
        NdjsonLimit::Derivations => "derivations",
        NdjsonLimit::StringBytes => "string-bytes",
    }
}

fn panel_result(id: &str) -> Value {
    let source = match id {
        "oversized-panel-source" => "x".repeat(MAXIMUM_PANEL_SOURCE_BYTES + 1),
        "panel-token-flood" => {
            let mut source = "panel 1\n".to_owned();
            source.push_str(&"a ".repeat(MAXIMUM_PANEL_TOKENS));
            source
        }
        "recursive-source-value" => {
            let depth = usize::from(MAXIMUM_SOURCE_VALUE_DEPTH);
            format!(
                "panel 1\nnode n : std/literal {{ value = {}0{} }}",
                "list(".repeat(depth),
                ")".repeat(depth)
            )
        }
        "hostile-diagnostic-text-is-escaped" => "panel 1\n\u{1b}".to_owned(),
        _ => panic!("unknown panel case {id}"),
    };
    match parse(&source) {
        Ok(_) => json!({"accepted": true}),
        Err(error) if id == "hostile-diagnostic-text-is-escaped" => json!({
            "accepted": false,
            "reflected_control_bytes": error.message.bytes().filter(u8::is_ascii_control).count()
        }),
        Err(error) => json!({"accepted": false, "reason": error.code}),
    }
}

fn artifact_result(id: &str) -> Value {
    let expected = b"fixture blob";
    let manifest = artifact_manifest(expected);
    match id {
        "artifact-digest-mismatch-is-terminal" => {
            let error = verify_artifact_owned_evidenced(
                &manifest,
                b"hostile bytes".to_vec(),
                None,
                None,
                policy(),
                &[],
            )
            .unwrap_err();
            json!({
                "accepted": false,
                "reason": error.evidence.reason_code,
                "terminal": error.evidence.terminal
            })
        }
        "artifact-rejection-reflects-no-payload" => {
            let error = verify_artifact_owned_evidenced(
                &manifest,
                b"secret hostile bytes".to_vec(),
                None,
                None,
                policy(),
                &[],
            )
            .unwrap_err();
            json!({
                "accepted": false,
                "reflected_payload_bytes": error.evidence.reflected_payload_bytes
            })
        }
        "verified-owned-artifact-handoff" => {
            let verified =
                verify_artifact_owned(&manifest, expected.to_vec(), None, None, policy(), &[])
                    .unwrap();
            let loader_bytes = verified
                .load_with::<_, HostedArtifactVerificationError>(|bytes| Ok(bytes.len()))
                .unwrap();
            json!({"accepted": true, "loader_bytes": loader_bytes})
        }
        _ => panic!("unknown artifact case {id}"),
    }
}

fn package_result(id: &str) -> Value {
    let limits = PackageLimits::default();
    let (bytes, expected_reason) = match id {
        "package-declared-length-overflow" => {
            let mut bytes = PACKAGE_MAGIC.to_vec();
            bytes.extend_from_slice(&u32::MAX.to_be_bytes());
            (bytes, "CND-PKG-007")
        }
        "package-trailing-bytes" => {
            let mut manifest = PackageManifest::new(Vec::new());
            manifest.seal().unwrap();
            let mut bytes = encode_package(&manifest, &BTreeMap::new(), limits).unwrap();
            bytes.push(0);
            (bytes, "CND-PKG-008")
        }
        "archive-or-decompression-input-is-not-a-package" => {
            (b"PK\x03\x04hostile archive".to_vec(), "CND-PKG-008")
        }
        _ => panic!("unknown package case {id}"),
    };
    match decode_package(&bytes, limits) {
        Ok(_) => json!({"accepted": true}),
        Err(error) => {
            assert_eq!(error.reason.code(), expected_reason);
            json!({"accepted": false, "reason": error.reason.code()})
        }
    }
}

#[test]
fn every_security_fixture_is_independently_executed() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    for case in fixture.cases {
        let actual = match case.operation.as_str() {
            "panel" => panel_result(&case.id),
            "evidence" => evidence_result(&case.id),
            "artifact" => artifact_result(&case.id),
            "package" => package_result(&case.id),
            operation => panic!("unknown security fixture operation {operation}"),
        };
        assert_eq!(actual, case.expected, "security fixture {}", case.id);
    }
}
