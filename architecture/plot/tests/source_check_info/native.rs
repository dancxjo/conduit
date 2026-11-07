//! Actual S1 Native projection; JSON report bytes are not a typed Cord value.
use super::*;
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn bytes(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}
fn tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("variant")
    };
    tag
}
#[test]
fn actual_valid_and_invalid_reports_encode_under_the_pinned_native_schema() {
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    for source in [VALID, "# α 🦀\nplot main {\n a: unavailable/kind\n}\n"] {
        let mut owner = projection(source);
        let request = request(&owner);
        let report = owner.project(&request).unwrap();
        let request_bytes = schema
            .encode_request(&request, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
            .unwrap();
        let encoded = schema
            .encode_report(&report, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
            .unwrap();
        let request_value = StructuredInfoValue::from_canonical_bytes(&request_bytes).unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
        assert_eq!(bytes(field(&request_value, "entry")), b"main");
        for value in [&request_value, &value] {
            let snapshot = field(value, "snapshot");
            assert_eq!(
                bytes(field(snapshot, "sha256")),
                report.snapshot.sha256.as_bytes()
            );
            assert_eq!(bytes(field(snapshot, "generation")), 7u64.to_le_bytes());
            assert_eq!(
                bytes(field(snapshot, "source_document_id")),
                report.snapshot.source_document_id.as_bytes()
            );
        }
        assert_eq!(
            tag(field(&value, "check")),
            match report.check {
                CheckOutcome::Accepted => "accepted",
                CheckOutcome::Rejected => "rejected",
            }
        );
        let candidate = field(&value, "candidate");
        for (name, expected) in [
            (
                "checked_plot_id",
                report.candidate.checked_plot_id.as_deref(),
            ),
            (
                "expanded_plot_id",
                report.candidate.expanded_plot_id.as_deref(),
            ),
        ] {
            let StructuredInfoValueShape::Variant { tag, payload } = field(candidate, name).shape()
            else {
                panic!("optional candidate")
            };
            assert_eq!(tag, if expected.is_some() { "some" } else { "none" });
            if let Some(expected) = expected {
                assert_eq!(bytes(payload), expected.as_bytes());
            }
        }
        for native in [&request_value, &value] {
            let context = field(native, "context");
            for (name, expected) in [
                ("grammar_edition", &report.context.grammar_edition),
                ("catalog", &report.context.catalog),
                ("kinds_fores", &report.context.kinds_fores),
                ("types_laws", &report.context.types_laws),
                ("backs", &report.context.backs),
            ] {
                assert_eq!(bytes(field(context, name)), expected.as_bytes());
            }
        }
        let StructuredInfoValueShape::Variant {
            tag: dependency_tag,
            payload,
        } = field(&value, "dependencies").shape()
        else {
            panic!("dependency provenance")
        };
        match &report.dependencies {
            DependencyProvenance::WholeContext => assert_eq!(dependency_tag, "whole_context"),
            DependencyProvenance::Observed {
                checking,
                expansion,
            } => {
                assert_eq!(dependency_tag, "observed");
                for (name, expected) in [("checking", checking), ("expansion", expansion)] {
                    let StructuredInfoValueShape::Collection(actual) = field(payload, name).shape()
                    else {
                        panic!("dependency reads")
                    };
                    assert_eq!(actual.len(), expected.len());
                    for (actual, expected) in actual.iter().zip(expected) {
                        assert_eq!(bytes(field(actual, "table")), expected.table.as_bytes());
                        assert_eq!(bytes(field(actual, "key")), expected.key.as_bytes());
                    }
                }
            }
        }
        assert_eq!(tag(field(&value, "plan")), "absent");
        assert_eq!(tag(field(&value, "play")), "absent");
        let StructuredInfoValueShape::Collection(diagnostics) =
            field(&value, "diagnostics").shape()
        else {
            panic!("diagnostic sequence")
        };
        assert_eq!(diagnostics.len(), report.diagnostics.len());
        for (actual, expected) in diagnostics.iter().zip(&report.diagnostics) {
            assert_eq!(bytes(field(actual, "code")), expected.code.as_bytes());
            assert_eq!(bytes(field(actual, "summary")), expected.summary.as_bytes());
            if let Some(span) = &expected.primary_span {
                let StructuredInfoValueShape::Variant { tag, payload } =
                    field(actual, "primary_span").shape()
                else {
                    panic!("optional span")
                };
                assert_eq!(tag, "some");
                for (name, expected) in [
                    ("start", span.start),
                    ("end", span.end),
                    ("line", span.line),
                    ("column", span.column),
                    ("end_line", span.end_line),
                    ("end_column", span.end_column),
                ] {
                    assert_eq!(
                        bytes(field(payload, name)),
                        u32::try_from(expected).unwrap().to_le_bytes()
                    );
                }
            } else {
                assert_eq!(tag(field(actual, "primary_span")), "none");
            }
        }
        assert!(matches!(
            schema.encode_report(&report, 1),
            Err(SourceCheckNativeRefusal::Bytes)
        ));
    }
}
#[test]
fn native_projection_refuses_out_of_schema_or_invalid_law_values() {
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let mut owner = projection("plot main {\n a: unavailable/kind\n}\n");
    let request = request(&owner);
    let report = owner.project(&request).unwrap();
    let mut invalid = report.clone();
    invalid.schema_version = 2;
    assert!(schema
        .encode_report(&invalid, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .is_err());
    invalid = report.clone();
    invalid.snapshot.generation = 0;
    assert!(schema
        .encode_report(&invalid, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .is_err());
    invalid = report.clone();
    let span = invalid.diagnostics[0].primary_span.as_mut().unwrap();
    span.start = span.end + 1;
    assert!(schema
        .encode_report(&invalid, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .is_err());
    let mut invalid_request = request.clone();
    invalid_request.entry.clear();
    assert!(schema
        .encode_request(&invalid_request, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .is_err());
    invalid_request = request;
    invalid_request.limits.dependencies = MAX_CHECK_DEPENDENCIES + 1;
    assert!(matches!(
        schema.encode_request(&invalid_request, MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        Err(SourceCheckNativeRefusal::Report(SourceCheckRefusal::Limits))
    ));
}

#[test]
fn typed_replies_preserve_checked_results_and_distinct_refusal_tags() {
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let mut owner = projection(VALID);
    let report = owner.project(&request(&owner)).unwrap();
    for (reply, expected) in [
        (SourceCheckReply::Checked(&report), "checked"),
        (SourceCheckReply::NotPrepared, "not_prepared"),
    ] {
        let encoded = schema
            .encode_reply(reply, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
        assert_eq!(tag(&value), expected);
    }
    for (reason, expected) in [
        (SourceCheckRefusal::InvalidSnapshot, "invalid_snapshot"),
        (SourceCheckRefusal::SourceBytes, "source_bytes"),
        (SourceCheckRefusal::StaleSource, "stale_source"),
        (SourceCheckRefusal::UnknownGrammar, "unknown_grammar"),
        (SourceCheckRefusal::UnknownContext, "unknown_context"),
        (SourceCheckRefusal::EntryBytes, "entry_bytes"),
        (SourceCheckRefusal::Limits, "limits"),
        (
            SourceCheckRefusal::InvalidDiagnosticSpan,
            "invalid_diagnostic_span",
        ),
        (
            SourceCheckRefusal::DiagnosticOverflow,
            "diagnostic_overflow",
        ),
        (
            SourceCheckRefusal::DependencyOverflow,
            "dependency_overflow",
        ),
    ] {
        let encoded = schema
            .encode_reply(
                SourceCheckReply::Refused(reason),
                MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            )
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
        let StructuredInfoValueShape::Variant {
            tag: outer,
            payload,
        } = value.shape()
        else {
            panic!("reply variant")
        };
        assert_eq!(outer, "refused");
        assert_eq!(tag(payload), expected);
    }
}

#[test]
fn incoming_reply_admission_uses_expected_schema_and_actual_primitive_laws() {
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let mut owner = projection(VALID);
    let request = request(&owner);
    let report = owner.project(&request).unwrap();
    let valid = schema
        .encode_reply(
            SourceCheckReply::Checked(&report),
            MAXIMUM_STRUCTURED_CANONICAL_BYTES,
        )
        .unwrap();
    schema
        .admit_reply(&valid, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .unwrap();
    let wrong = schema
        .encode_request(&request, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .unwrap();
    assert!(schema
        .admit_reply(&wrong, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .is_err());
    assert!(matches!(
        schema.admit_reply(&valid, 1),
        Err(SourceCheckNativeRefusal::Bytes)
    ));
    let value = StructuredInfoValue::from_canonical_bytes(&valid).unwrap();
    let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
        panic!("reply")
    };
    let StructuredInfoValueShape::Record(fields) = payload.shape() else {
        panic!("report")
    };
    let fields = fields
        .iter()
        .map(|field| {
            let value = if field.name() == "schema_version" {
                StructuredInfoValue::leaf(
                    field.value().value_type().clone(),
                    2u16.to_le_bytes().to_vec(),
                )
                .unwrap()
            } else {
                field.value().clone()
            };
            StructuredFieldValue::new(field.name(), value).unwrap()
        })
        .collect();
    let bad = StructuredInfoValue::record(payload.value_type().clone(), fields).unwrap();
    let bad = StructuredInfoValue::variant(value.value_type().clone(), "checked", bad)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    // This is well-shaped Core data. The independently retained S1 law refuses it.
    assert!(conduit_core::validate_canonical_structured_value(&bad).is_ok());
    assert!(schema
        .admit_reply(&bad, MAXIMUM_STRUCTURED_CANONICAL_BYTES)
        .is_err());
}
