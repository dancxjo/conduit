use super::*;
fn program(entry: &str) -> PortableExpressionProgram {
    let check_types = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../semantics/language/source_check_info.conduit"
        )),
        &StartupCatalog::new(),
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    for (name, path) in [
        ("SourceCheckDigest", "digest"),
        ("SourceCheckLabel", "label"),
        ("SourceCheckSnapshot", "snapshot"),
        ("SourceCheckContext", "context"),
    ] {
        let native = check_types
            .native_types
            .iter()
            .find(|n| n.name == name)
            .unwrap();
        startup
            .insert_checked_native_type_from_document(
                format!("language/source-check/{path}"),
                &check_types,
                &native.identity,
            )
            .unwrap();
    }
    let document = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../plots/body-workset/propose.conduit"
        )),
        &startup,
    )
    .unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&document, entry, &ProfileCatalog::new()).unwrap();
    let [gear] = expanded.expanded.gears.as_slice() else {
        panic!("one pure expression")
    };
    let ConfigurationValue::Text(text) = &gear
        .configuration
        .iter()
        .find(|e| e.key == "program")
        .unwrap()
        .value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(text).unwrap()
}
fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record type")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn record(
    ty: &StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn text(ty: &StructuredInfoType, value: &str) -> StructuredInfoValue {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        StructuredInfoValue::nominal(ty.clone(), text(representation, value)).unwrap()
    } else {
        StructuredInfoValue::leaf(ty.clone(), value.as_bytes().to_vec()).unwrap()
    }
}
fn plot_value(ty: &StructuredInfoType, plot: &ResidentPlot) -> StructuredInfoValue {
    record(
        ty,
        vec![
            (
                "source_document_id",
                text(
                    &field_type(ty, "source_document_id"),
                    plot.source_document_id.as_str(),
                ),
            ),
            (
                "checked_plot_id",
                text(
                    &field_type(ty, "checked_plot_id"),
                    plot.checked_plot_id.as_str(),
                ),
            ),
        ],
    )
}
fn checked_source(ty: &StructuredInfoType, report: &SourceCheckInfoV1) -> StructuredInfoValue {
    let snapshot = field_type(ty, "snapshot");
    let context = field_type(ty, "context");
    record(
        ty,
        vec![
            (
                "snapshot",
                record(
                    &snapshot,
                    vec![
                        (
                            "owner",
                            text(&field_type(&snapshot, "owner"), &report.snapshot.owner),
                        ),
                        (
                            "generation",
                            StructuredInfoValue::leaf(
                                field_type(&snapshot, "generation"),
                                report.snapshot.generation.to_le_bytes().to_vec(),
                            )
                            .unwrap(),
                        ),
                        (
                            "sha256",
                            text(&field_type(&snapshot, "sha256"), &report.snapshot.sha256),
                        ),
                        (
                            "source_document_id",
                            text(
                                &field_type(&snapshot, "source_document_id"),
                                &report.snapshot.source_document_id,
                            ),
                        ),
                    ],
                ),
            ),
            (
                "context",
                record(
                    &context,
                    [
                        ("grammar_edition", &report.context.grammar_edition),
                        ("catalog", &report.context.catalog),
                        ("kinds_fores", &report.context.kinds_fores),
                        ("types_laws", &report.context.types_laws),
                        ("backs", &report.context.backs),
                    ]
                    .into_iter()
                    .map(|(name, value)| (name, text(&field_type(&context, name), value)))
                    .collect(),
                ),
            ),
            (
                "entry",
                text(&field_type(ty, "entry"), &report.requested_entry),
            ),
            (
                "checked_plot_id",
                text(
                    &field_type(ty, "checked_plot_id"),
                    report.candidate.checked_plot_id.as_deref().unwrap(),
                ),
            ),
        ],
    )
}
fn observation(
    ty: &StructuredInfoType,
    body: &Body,
    retained: &ResidentPlot,
) -> StructuredInfoValue {
    record(
        ty,
        vec![
            (
                "body_id",
                text(&field_type(ty, "body_id"), body.body_id.as_str()),
            ),
            (
                "workload_revision",
                StructuredInfoValue::leaf(
                    field_type(ty, "workload_revision"),
                    body.workload_revision.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            (
                "retained",
                plot_value(&field_type(ty, "retained"), retained),
            ),
        ],
    )
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record value")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn bytes(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}
fn compare(output: &[u8], expected: &BodyWorksetProposalInfoV1) {
    let output = StructuredInfoValue::from_canonical_bytes(output).unwrap();
    assert_eq!(bytes(field(&output, "schema_version")), 1u16.to_le_bytes());
    assert_eq!(
        bytes(field(&output, "expected_body_id")),
        expected.expected_body_id.as_str().as_bytes()
    );
    assert_eq!(
        bytes(field(&output, "expected_workload_revision")),
        expected.expected_workload_revision.to_le_bytes()
    );
    let StructuredInfoValueShape::Collection(plots) = field(&output, "proposed_workset").shape()
    else {
        panic!("sequence")
    };
    let actual = BodyWorkset::from_plots(plots.iter().map(|value| {
        ResidentPlot::new(
            std::str::from_utf8(bytes(field(value, "source_document_id")))
                .unwrap()
                .into(),
            std::str::from_utf8(bytes(field(value, "checked_plot_id")))
                .unwrap()
                .into(),
        )
    }))
    .unwrap();
    assert_eq!(actual, expected.proposed_workset);
    let StructuredInfoValueShape::Variant { tag, payload } =
        field(&output, "candidate_source").shape()
    else {
        panic!("Source basis")
    };
    if let Some(source) = &expected.candidate_source {
        assert_eq!(tag, "checked");
        assert_eq!(bytes(field(payload, "entry")), source.entry.as_bytes());
        let snapshot = field(payload, "snapshot");
        assert_eq!(
            bytes(field(snapshot, "owner")),
            source.snapshot.owner.as_bytes()
        );
        assert_eq!(
            bytes(field(snapshot, "generation")),
            source.snapshot.generation.to_le_bytes()
        );
        assert_eq!(
            bytes(field(snapshot, "sha256")),
            source.snapshot.sha256.as_bytes()
        );
        assert_eq!(
            bytes(field(snapshot, "source_document_id")),
            source.plot.source_document_id.as_str().as_bytes()
        );
        assert_eq!(
            bytes(field(payload, "checked_plot_id")),
            source.plot.checked_plot_id.as_str().as_bytes()
        );
        let context = field(payload, "context");
        for (name, value) in [
            ("grammar_edition", &source.context.grammar_edition),
            ("catalog", &source.context.catalog),
            ("kinds_fores", &source.context.kinds_fores),
            ("types_laws", &source.context.types_laws),
            ("backs", &source.context.backs),
        ] {
            assert_eq!(bytes(field(context, name)), value.as_bytes());
        }
    } else {
        assert_eq!(tag, "none");
    }
}
#[test]
fn checked_authored_plots_emit_addition_and_later_removal_as_typed_info() {
    let a = resident(&report("first"));
    let checked = report("second");
    let b = resident(&checked);
    let observed = body(&a, 1);
    let before = observed.clone();
    let add = program("propose-add");
    let input = record(
        &add.input_type,
        vec![
            (
                "observed",
                observation(&field_type(&add.input_type, "observed"), &observed, &a),
            ),
            (
                "checked",
                checked_source(&field_type(&add.input_type, "checked"), &checked),
            ),
        ],
    )
    .canonical_bytes()
    .unwrap();
    let expected = propose_checked_workset(
        &observed,
        &observed.body_id,
        0,
        vec![a.clone(), b.clone()],
        Some(candidate(&checked)),
    )
    .unwrap();
    compare(&add.evaluate(&input).unwrap(), &expected);
    let replacement = program("propose-replace");
    let expected_replacement = propose_checked_workset(
        &observed,
        &observed.body_id,
        0,
        vec![b.clone()],
        Some(candidate(&checked)),
    )
    .unwrap();
    // The same exact typed request is accepted by both authored compositions.
    compare(
        &replacement.evaluate(&input).unwrap(),
        &expected_replacement,
    );
    assert_eq!(observed, before);
    let later = observed.admit_plot(b, "sign/later".into()).unwrap();
    let prior = later.clone();
    let remove = program("propose-retain-one");
    let input = observation(&remove.input_type, &later, &a)
        .canonical_bytes()
        .unwrap();
    let expected = propose_checked_workset(
        &later,
        &later.body_id,
        later.workload_revision,
        vec![a],
        None,
    )
    .unwrap();
    compare(&remove.evaluate(&input).unwrap(), &expected);
    assert_eq!(later, prior);
}
