use std::collections::BTreeMap;

use conduit_core::{
    ContainmentReason, DiagnosticCode, ImplementationError, PlanCollection, PlanDiagnosticCode,
    PlanValidationError, SemanticHash, ValidationError,
};
use conduit_diagnostics::{
    CompatibilityDiagnosticContext, DiagnosticSource, FixStatus, KnownAdapterFix,
    OwnedDiagnosticArgument, OwnedDiagnosticArgumentValue, OwnedDiagnosticEdit, OwnedDiagnosticFix,
    OwnedDiagnosticSeverity, OwnedDiagnosticSpan, OwnedFixApplicability, PlanDiagnosticContext,
    TerminalColor, TerminalVerbosity, check_fix, from_implementation_error, from_lowering_error,
    from_module_error, from_parse_error, from_plan_error, from_source_schema_error,
    from_validation_error, render_terminal,
};
use conduit_panel::{
    LoadedModule, ModuleLoader, SourceSpan, parse, resolve_modules, semantic_source_hash_version,
};
use conduit_runtime::{LoweringDiagnostic, OwnedTypeReference, SourceOrigin};
use serde_json::Value;

struct MemoryLoader(BTreeMap<String, String>);

impl ModuleLoader for MemoryLoader {
    fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
        Ok(self.0.get(canonical_uri).map(|source| LoadedModule {
            canonical_uri: canonical_uri.to_owned(),
            source: source.clone(),
        }))
    }
}

#[test]
fn resolver_and_plan_failures_have_structured_adapters() {
    let root = DiagnosticSource::new(
        "mem://fixture/root.panel",
        b"panel 1\nimport \"./missing.panel\" as missing\n".as_slice(),
    );
    let module_error = resolve_modules(
        "mem://fixture/root.panel",
        None,
        &MemoryLoader(BTreeMap::from([(
            root.document_id.clone(),
            String::from_utf8(root.bytes.clone()).unwrap(),
        )])),
    )
    .unwrap_err();
    let module = from_module_error(&module_error, std::slice::from_ref(&root));
    assert_eq!(module.code, "CND-SRC-003");
    assert!(
        module
            .related
            .iter()
            .any(|related| related.subject.as_deref() == Some("mem://fixture/root.panel"))
    );

    let plan = from_plan_error(
        PlanValidationError {
            code: PlanDiagnosticCode::UnresolvedSelection,
            collection: PlanCollection::Unresolved,
            subject_index: Some(0),
        },
        PlanDiagnosticContext {
            primary: None,
            semantic_path: Some("root/node/value".to_owned()),
        },
    );
    assert_eq!(plan.code, "CND-PLN-005");
    assert_eq!(plan.semantic_path.as_deref(), Some("root/node/value"));
    assert!(
        plan.arguments
            .iter()
            .any(|argument| argument.name == "collection")
    );

    let containment = from_plan_error(
        PlanValidationError {
            code: PlanDiagnosticCode::Containment(ContainmentReason::ApprovalMissing),
            collection: PlanCollection::Authorities,
            subject_index: Some(0),
        },
        PlanDiagnosticContext {
            primary: None,
            semantic_path: Some("root/admin".to_owned()),
        },
    );
    assert_eq!(containment.code, "CND-CTN-007");
    assert_eq!(
        containment.message,
        "administrative containment failed: independent-approval-proof-missing"
    );

    let implementation = from_implementation_error(ImplementationError::FalseProgress);
    assert_eq!(implementation.code, "CND-IMP-006");
    implementation.validate().unwrap();
}

#[test]
fn unsupported_source_schema_has_a_structured_adapter() {
    let panel = parse("panel 1\nnode app : fixture/handler\n").unwrap();
    let error = semantic_source_hash_version(&panel, 99).unwrap_err();
    let diagnostic = from_source_schema_error(&error);
    assert_eq!(diagnostic.code, "CND-SRC-011");
    assert_eq!(diagnostic.arguments[0].name, "schema_version");
}

#[test]
fn every_normative_diagnostic_vector_is_valid_lossless_and_renderable() {
    let suite: Value =
        serde_json::from_str(include_str!("../../../conformance/c3/diagnostics-v1.json")).unwrap();
    for case in suite["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let diagnostic: conduit_diagnostics::OwnedDiagnostic =
            serde_json::from_value(case["diagnostic"].clone()).unwrap();
        diagnostic.validate().unwrap();
        let json = diagnostic.to_json().unwrap();
        assert_eq!(
            conduit_diagnostics::OwnedDiagnostic::from_json(&json).unwrap(),
            diagnostic,
            "{id}: JSON round trip"
        );
        let sources = case["source_hex"].as_str().map_or_else(Vec::new, |hex| {
            vec![DiagnosticSource::new(
                "mem://fixture/root.panel",
                decode_hex(hex),
            )]
        });
        let plain = render_terminal(
            &diagnostic,
            &sources,
            TerminalColor::Never,
            TerminalVerbosity::Concise,
        );
        let verbose = render_terminal(
            &diagnostic,
            &sources,
            TerminalColor::Never,
            TerminalVerbosity::Verbose,
        );
        let color = render_terminal(
            &diagnostic,
            &sources,
            TerminalColor::Always,
            TerminalVerbosity::Verbose,
        );
        let expected = &case["expected"];
        assert!(
            plain.contains(expected["plain_contains"].as_str().unwrap()),
            "{id}: plain output"
        );
        assert!(
            verbose.contains(expected["verbose_contains"].as_str().unwrap()),
            "{id}: verbose output"
        );
        assert_eq!(
            color.contains("\u{1b}["),
            expected["color_ansi"].as_bool().unwrap(),
            "{id}: color"
        );
        if let Some(forbidden) = expected["forbidden"].as_str() {
            assert!(!json.contains(forbidden), "{id}: JSON leaked");
            assert!(!verbose.contains(forbidden), "{id}: terminal leaked");
        }
        let status =
            diagnostic
                .fixes
                .first()
                .map_or("none", |fix| match check_fix(fix, &sources) {
                    FixStatus::Applicable => "applicable",
                    FixStatus::StalePrecondition => "stale-precondition",
                    FixStatus::MissingDocument => "missing-document",
                    FixStatus::InvalidRange => "invalid-range",
                });
        assert_eq!(
            status,
            expected["fix_status"].as_str().unwrap(),
            "{id}: fix"
        );
    }
}

#[test]
fn common_mistakes_have_five_explicit_unapplied_fix_contracts() {
    let arrow_source = DiagnosticSource::new(
        "mem://fixture/arrow.panel",
        b"panel 1\ncord microphone.audio tts.text\n".as_slice(),
    );
    let arrow = from_parse_error(
        &parse(std::str::from_utf8(&arrow_source.bytes).unwrap()).unwrap_err(),
        &arrow_source,
    );
    assert_eq!(arrow.fixes[0].id, "insert-cord-arrow");

    let version_source =
        DiagnosticSource::new("mem://fixture/version.panel", b"panel 2\n".as_slice());
    let version = from_parse_error(
        &parse(std::str::from_utf8(&version_source.bytes).unwrap()).unwrap_err(),
        &version_source,
    );
    assert_eq!(version.fixes[0].id, "use-panel-version-1");

    let comma_source = DiagnosticSource::new(
        "mem://fixture/comma.panel",
        b"panel 1\nnode value : fixture/all { items = list(true, ) }\n".as_slice(),
    );
    let comma = from_parse_error(
        &parse(std::str::from_utf8(&comma_source.bytes).unwrap()).unwrap_err(),
        &comma_source,
    );
    assert_eq!(comma.fixes[0].id, "remove-trailing-comma");

    let secret_source = DiagnosticSource::new(
        "mem://fixture/secret.panel",
        b"panel 1\nnode value : fixture/secret { token = \"do-not-echo\" }\n".as_slice(),
    );
    let secret_error = LoweringDiagnostic {
        code: "CND-LWR-009",
        semantic_path: "mem://fixture/secret.panel/node/value/config/token".to_owned(),
        expected_contract: Some(Box::new(OwnedTypeReference {
            id: "fixture/secret-ref".to_owned(),
            schema_version: 1,
            semantic_hash: SemanticHash::from_bytes([0x11; 32]),
        })),
        origin: Some(Box::new(SourceOrigin {
            module_uri: secret_source.document_id.clone(),
            module_hash: secret_source.content_hash.clone(),
            span: SourceSpan {
                line: 2,
                column: 47,
                end_line: 2,
                end_column: 60,
            },
        })),
        message: "protected source configuration requires an unresolved secret reference"
            .to_owned(),
    };
    let secret = from_lowering_error(&secret_error, &[secret_source]);
    assert_eq!(secret.fixes[0].id, "replace-with-secret-reference");
    assert!(!format!("{secret:?}").contains("do-not-echo"));

    let source = DiagnosticSource::new(
        "mem://fixture/mismatch.panel",
        b"panel 1\ncord microphone.audio -> tts.text\n".as_slice(),
    );
    let adapter = from_validation_error(
        ValidationError {
            code: DiagnosticCode::TypeMismatch,
            subject_index: Some(0),
        },
        CompatibilityDiagnosticContext {
            cord: span(&source, 8, 41, 2, 1, 2, 34),
            writer: span(&source, 13, 29, 2, 6, 2, 22),
            reader: span(&source, 33, 41, 2, 26, 2, 34),
            writer_contract: "audio.pcm",
            reader_contract: "text.utf8",
            semantic_path: Some("root/cord/0"),
            cause_code: "CND-TYP-001",
            known_adapter: Some(KnownAdapterFix {
                adapter_id: "fixture/transcribe",
                edit: OwnedDiagnosticEdit {
                    document_id: source.document_id.clone(),
                    precondition_hash: source.content_hash.clone(),
                    byte_start: 8,
                    byte_end: 41,
                    replacement: "node adapter : fixture/transcribe\ncord microphone.audio -> adapter.in\ncord adapter.out -> tts.text".to_owned(),
                },
            }),
        },
    );
    assert_eq!(adapter.fixes[0].id, "insert-known-adapter");
}

#[test]
fn terminal_color_and_plain_snapshots_are_exact() {
    let source = DiagnosticSource::new(
        "mem://fixture/root.panel",
        b"panel 1\ncord microphone.audio -> tts.text\n".as_slice(),
    );
    let diagnostic = from_validation_error(
        ValidationError {
            code: DiagnosticCode::TypeMismatch,
            subject_index: Some(0),
        },
        CompatibilityDiagnosticContext {
            cord: span(&source, 8, 41, 2, 1, 2, 34),
            writer: span(&source, 13, 29, 2, 6, 2, 22),
            reader: span(&source, 33, 41, 2, 26, 2, 34),
            writer_contract: "audio.pcm",
            reader_contract: "text.utf8",
            semantic_path: Some("root/cord/0"),
            cause_code: "CND-TYP-001",
            known_adapter: None,
        },
    );
    let plain = render_terminal(
        &diagnostic,
        std::slice::from_ref(&source),
        TerminalColor::Never,
        TerminalVerbosity::Concise,
    );
    assert_eq!(
        plain,
        concat!(
            "error[CND-TYP-001]: writer port is not accepted by reader port\n",
            "--> mem://fixture/root.panel:2:1 (bytes 8..41)\n",
            "2 | cord microphone.audio -> tts.text\n",
            "  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
        )
    );
    let color = render_terminal(
        &diagnostic,
        &[source],
        TerminalColor::Always,
        TerminalVerbosity::Concise,
    );
    assert_eq!(
        color,
        concat!(
            "\u{1b}[1;31merror[CND-TYP-001]\u{1b}[0m: ",
            "writer port is not accepted by reader port\n",
            "\u{1b}[1;34m-->\u{1b}[0m mem://fixture/root.panel:2:1 ",
            "(bytes 8..41)\n",
            "2 | cord microphone.audio -> tts.text\n",
            "  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
        )
    );
}

#[test]
fn json_redaction_fix_freshness_and_non_utf8_offsets_are_lossless() {
    let source = DiagnosticSource::new("mem://fixture/raw.panel", vec![b'a', 0xff, b'\n']);
    let fix = OwnedDiagnosticFix {
        id: "replace-byte".to_owned(),
        message: "replace invalid byte".to_owned(),
        applicability: OwnedFixApplicability::MachineApplicable,
        edits: vec![OwnedDiagnosticEdit {
            document_id: source.document_id.clone(),
            precondition_hash: source.content_hash.clone(),
            byte_start: 1,
            byte_end: 2,
            replacement: "?".to_owned(),
        }],
    };
    assert_eq!(
        check_fix(&fix, std::slice::from_ref(&source)),
        FixStatus::Applicable
    );
    let mut stale = fix.clone();
    stale.edits[0].precondition_hash = format!("sha256:{}", "0".repeat(64));
    assert_eq!(
        check_fix(&stale, std::slice::from_ref(&source)),
        FixStatus::StalePrecondition
    );

    let diagnostic = conduit_diagnostics::OwnedDiagnostic {
        schema_version: 1,
        code: "CND-SRC-001".to_owned(),
        severity: OwnedDiagnosticSeverity::Error,
        message: "input contains an invalid byte".to_owned(),
        primary: Some(span(&source, 1, 2, 1, 2, 1, 3)),
        related: Vec::new(),
        arguments: vec![OwnedDiagnosticArgument {
            name: "payload".to_owned(),
            value: OwnedDiagnosticArgumentValue::Redacted {
                sensitivity: "secret".to_owned(),
                value_type: "fixture/token".to_owned(),
                byte_len: Some(17),
            },
        }],
        notes: Vec::new(),
        help: None,
        fixes: vec![fix],
        semantic_path: None,
        causes: Vec::new(),
    };
    let json = diagnostic.to_json().unwrap();
    assert!(!json.contains("actual-secret"));
    assert!(json.contains("\"byte_start\":1"));
    assert_eq!(
        conduit_diagnostics::OwnedDiagnostic::from_json(&json).unwrap(),
        diagnostic
    );
    let verbose = render_terminal(
        &diagnostic,
        &[source],
        TerminalColor::Never,
        TerminalVerbosity::Verbose,
    );
    assert!(verbose.contains("argument payload: [REDACTED]"));
    assert!(verbose.contains("a�"));
}

fn span(
    source: &DiagnosticSource,
    byte_start: u64,
    byte_end: u64,
    line: u64,
    column: u64,
    end_line: u64,
    end_column: u64,
) -> OwnedDiagnosticSpan {
    OwnedDiagnosticSpan {
        document_id: source.document_id.clone(),
        content_hash: Some(source.content_hash.clone()),
        byte_start,
        byte_end,
        line,
        column,
        end_line,
        end_column,
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
