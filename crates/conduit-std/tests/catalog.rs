use conduit_std::{
    CatalogError, CatalogTypeExpression, DeterministicProvider, FixtureClass, HostedProvider,
    ProviderProfile, ReferenceProvider, STANDARD_CATALOG, STANDARD_CATALOG_SCHEMA_VERSION,
    STANDARD_TYPE_CATALOG, StandardFamily, TimeBasis, run_catalog_fixture, standard_type,
    standard_type_reference, validate_catalog, validate_entry,
};

#[test]
fn complete_catalog_is_allocator_free_typed_and_bounded() {
    assert_eq!(STANDARD_CATALOG_SCHEMA_VERSION, 1);
    assert!(STANDARD_CATALOG.len() >= 90);
    for entry in STANDARD_CATALOG {
        validate_entry(entry)
            .unwrap_or_else(|error| panic!("{}: {error:?}", entry.contract.id.as_str()));
    }
    validate_catalog(STANDARD_CATALOG).unwrap();
    assert!(STANDARD_CATALOG.iter().all(|entry| {
        !entry.contract.inputs.is_empty()
            || !entry.contract.outputs.is_empty()
            || entry.contract.id.as_str() == "flow/discard"
    }));
}

#[test]
fn standard_nodes_use_domain_oriented_canonical_paths() {
    for entry in STANDARD_CATALOG {
        assert!(
            !entry.contract.id.as_str().starts_with("conduit.std/"),
            "flat legacy identity remains for {}",
            entry.contract.id.as_str()
        );
        assert!(entry.contract.id.as_str().contains('/'));
    }
    assert!(
        STANDARD_CATALOG
            .iter()
            .any(|entry| entry.contract.id.as_str() == "net/http/serve")
    );
    assert!(
        STANDARD_CATALOG
            .iter()
            .any(|entry| entry.contract.id.as_str() == "flow/identity")
    );
}

#[test]
fn type_universe_is_richer_than_any_host_support_claim() {
    assert_eq!(
        standard_type("std/integer").unwrap().human_name,
        "mathematical signed integer"
    );
    assert!(standard_type_reference("std/integer").is_some());
    assert!(standard_type_reference("std/option").is_none());
    assert!(standard_type("net/http/request").is_some());
    assert!(STANDARD_TYPE_CATALOG.len() >= 40);
    for (index, definition) in STANDARD_TYPE_CATALOG.iter().enumerate() {
        assert!(conduit_core::Id::new(definition.id.as_str()).is_ok());
        assert!(
            !STANDARD_TYPE_CATALOG[..index]
                .iter()
                .any(|prior| prior.id == definition.id)
        );
    }
}

#[test]
fn formatter_types_have_exact_finite_descriptors() {
    let expected = [
        (
            "std/text",
            "sha256:79dd1d77e2cf6459bc3a8f96c65a915adc10db516dcac039f781bee5c1cab5ab",
        ),
        (
            "std/integer",
            "sha256:161d9106bcff3ea645da0f89570c1c43fe87a50299f2725720dc5c75f10cd12e",
        ),
        (
            "std/format-values",
            "sha256:ba23e276b70b1b0c747d2b4ada100d72fa5b3874e4fa2baa250cf07149795cc0",
        ),
    ];
    for (id, hash) in expected {
        let definition = standard_type(id).expect("formatter type is published");
        let descriptor = conduit_std::standard_type_descriptor(definition);
        assert_ne!(descriptor.body, conduit_core::CanonicalValue::Null);
        assert_eq!(
            standard_type_reference(id)
                .expect("formatter type is concrete")
                .semantic_hash
                .to_string(),
            hash,
            "{id}"
        );
    }
}

#[test]
fn polymorphic_flow_contracts_publish_type_relationships_not_byte_placeholders() {
    for id in [
        "flow/identity",
        "flow/tee",
        "flow/merge",
        "flow/first",
        "flow/count",
        "time/delay",
        "state/cell",
    ] {
        let entry = STANDARD_CATALOG
            .iter()
            .find(|entry| entry.contract.id.as_str() == id)
            .unwrap();
        let signature = entry
            .generic_signature
            .unwrap_or_else(|| panic!("{id} is not generic"));
        assert_eq!(signature.parameters[0].as_str(), "value");
        assert!(signature.ports.iter().any(|port| {
            matches!(
                port.value_type,
                CatalogTypeExpression::Parameter(parameter) if parameter.as_str() == "value"
            )
        }));
    }

    let first = STANDARD_CATALOG
        .iter()
        .find(|entry| entry.contract.id.as_str() == "flow/first")
        .unwrap()
        .generic_signature
        .unwrap();
    assert!(first.ports.iter().any(|port| {
        matches!(
            port.value_type,
            CatalogTypeExpression::Apply { constructor, arguments }
                if constructor.as_str() == "std/option"
                    && matches!(
                        arguments,
                        [CatalogTypeExpression::Parameter(parameter)]
                            if parameter.as_str() == "value"
                    )
        )
    }));

    let count = STANDARD_CATALOG
        .iter()
        .find(|entry| entry.contract.id.as_str() == "flow/count")
        .unwrap()
        .generic_signature
        .unwrap();
    assert!(count.ports.iter().any(|port| {
        matches!(
            port.value_type,
            CatalogTypeExpression::Named(id) if id.as_str() == "std/natural"
        )
    }));
}

#[test]
fn http_contracts_use_domain_types_without_claiming_a_provider() {
    let serve = STANDARD_CATALOG
        .iter()
        .find(|entry| entry.contract.id.as_str() == "net/http/serve")
        .unwrap();
    assert_eq!(
        serve.contract.inputs[0].value_type.contract_id.as_str(),
        "net/http/response"
    );
    assert_eq!(
        serve.contract.outputs[0].value_type.contract_id.as_str(),
        "net/http/request"
    );
    assert_eq!(
        serve
            .contract
            .config
            .fields
            .iter()
            .find(|field| field.key.as_str() == "listen")
            .unwrap()
            .value_type
            .contract_id
            .as_str(),
        "net/socket/address"
    );
    assert_eq!(serve.host_service.unwrap().as_str(), "host/http-server");
    assert!(serve.required_support.hosted);
}

#[test]
fn every_time_state_boundary_and_adapter_fact_is_enforced() {
    for entry in STANDARD_CATALOG {
        let mut broken = *entry;
        broken.limits.work_per_step = 0;
        assert_eq!(validate_entry(&broken), Err(CatalogError::UnboundedWork));

        if entry.time_basis != TimeBasis::None {
            broken = *entry;
            broken.limits.timers = 0;
            assert_eq!(validate_entry(&broken), Err(CatalogError::MissingTimer));
        }
        if matches!(
            entry.family,
            StandardFamily::Boundary | StandardFamily::Network
        ) {
            broken = *entry;
            broken.host_service = None;
            assert_eq!(
                validate_entry(&broken),
                Err(CatalogError::MissingHostService)
            );
        }
    }
}

#[test]
fn catalog_manifest_maps_every_contract_to_required_fixture_classes() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c4/standard-catalog-v1.json"
    ))
    .unwrap();
    assert_eq!(fixture["schema"], "conduit.standard-catalog/v1");
    let classes = fixture["required_fixture_classes"].as_array().unwrap();
    assert_eq!(
        classes,
        &[
            "positive",
            "negative",
            "pressure",
            "cancellation",
            "terminal"
        ]
    );
    let contracts = fixture["contracts"].as_object().unwrap();
    assert_eq!(contracts.len(), STANDARD_CATALOG.len());
    for entry in STANDARD_CATALOG {
        let requirements = contracts
            .get(entry.contract.id.as_str())
            .unwrap_or_else(|| panic!("missing {}", entry.contract.id.as_str()));
        assert!(requirements.as_array().unwrap().len() >= 2);
    }
}

#[test]
fn deterministic_and_hosted_profiles_emit_equivalent_normalized_evidence() {
    let classes = [
        FixtureClass::Positive,
        FixtureClass::Negative,
        FixtureClass::Pressure,
        FixtureClass::Cancellation,
        FixtureClass::Terminal,
    ];
    let mut deterministic = DeterministicProvider;
    let mut hosted = HostedProvider;
    for entry in STANDARD_CATALOG {
        for class in classes {
            assert_eq!(
                deterministic.run(entry, class).unwrap(),
                hosted.run(entry, class).unwrap(),
                "{} {class:?}",
                entry.contract.id.as_str()
            );
            let constrained =
                run_catalog_fixture(entry, class, ProviderProfile::Constrained).unwrap();
            if !entry.required_support.constrained {
                assert_eq!(
                    constrained.outcome,
                    conduit_std::FixtureOutcome::Unsupported
                );
            }
        }
    }
}

#[test]
fn text_format_fixture_and_contract_freeze_the_final_typed_shape() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../conformance/c4/text-format-v1.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let ids = cases
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    for required in [
        "indexed-success",
        "named-success",
        "empty-template",
        "escaped-delimiters",
        "missing-value",
        "extra-value",
        "malformed-placeholder",
        "supported-text",
        "supported-bool",
        "supported-integer-boundaries",
        "unsupported-value-kind",
        "maximum-output",
        "output-overflow",
        "cancellation",
        "stale-provider",
        "missing-provider",
        "constrained-unsupported-host",
        "no-provider-known-contract",
        "evidence-bounds",
    ] {
        assert!(ids.contains(required), "fixture covers {required}");
    }
    for case in cases {
        match case["class"].as_str().unwrap() {
            "semantic" => run_text_format_semantic_case(case),
            "generated-boundary" => run_text_format_boundary_case(case),
            "exact-execution" | "resolution" | "availability" => {
                assert!(
                    case["proof"]
                        .as_str()
                        .is_some_and(|proof| !proof.is_empty()),
                    "{} names its executable higher-layer proof",
                    case["id"]
                );
                assert!(
                    case["expected"]
                        .as_str()
                        .is_some_and(|expected| !expected.is_empty())
                );
            }
            other => panic!("unknown formatter fixture class {other}"),
        }
    }

    let format = STANDARD_CATALOG
        .iter()
        .find(|entry| entry.contract.id.as_str() == "std/text/format")
        .unwrap();
    assert!(format.contract.config.fields.is_empty());
    assert_eq!(format.contract.inputs.len(), 2);
    assert_eq!(format.contract.inputs[0].id.as_str(), "template");
    assert_eq!(
        format.contract.inputs[0].value_type.contract_id.as_str(),
        "std/text"
    );
    assert_eq!(format.contract.inputs[1].id.as_str(), "values");
    assert_eq!(
        format.contract.inputs[1].value_type.contract_id.as_str(),
        "std/format-values"
    );
    assert_eq!(format.contract.outputs.len(), 1);
    assert_eq!(
        format.contract.outputs[0].value_type.contract_id.as_str(),
        "std/text"
    );
    assert_eq!(format.limits.retained_values, 3);
    assert_eq!(
        format.limits.retained_bytes,
        conduit_std::FORMAT_MAX_RETAINED_BYTES as u64
    );
    assert_eq!(
        format.limits.work_per_step,
        conduit_std::FORMAT_MAX_WORK as u32
    );
    assert!(!format.required_support.constrained);
    assert!(
        STANDARD_CATALOG
            .iter()
            .all(|entry| entry.contract.id.as_str() != "std/format")
    );
}

fn run_text_format_semantic_case(case: &serde_json::Value) {
    let values = case["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let name = value.get("name").and_then(serde_json::Value::as_str);
            let scalar = match value["kind"].as_str().unwrap() {
                "text" => conduit_std::FormatScalarRef::Text(value["value"].as_str().unwrap()),
                "boolean" => {
                    conduit_std::FormatScalarRef::Boolean(value["value"].as_bool().unwrap())
                }
                "integer" => conduit_std::FormatScalarRef::Integer(
                    value["value"].as_str().unwrap().parse().unwrap(),
                ),
                "future" => conduit_std::FormatScalarRef::Unsupported(0xff),
                kind => panic!("unknown formatter fixture value kind {kind}"),
            };
            conduit_std::FormatValueRef {
                name,
                value: scalar,
            }
        })
        .collect::<Vec<_>>();
    let mut output = [0; conduit_std::FORMAT_MAX_OUTPUT_BYTES];
    let result =
        conduit_std::format_text_into(case["template"].as_str().unwrap(), &values, &mut output);
    if let Some(expected) = case.get("expected_output") {
        let length =
            result.unwrap_or_else(|error| panic!("{} unexpectedly failed: {error:?}", case["id"]));
        assert_eq!(
            core::str::from_utf8(&output[..length]).unwrap(),
            expected.as_str().unwrap(),
            "{}",
            case["id"]
        );
    } else {
        assert_eq!(
            result.unwrap_err().code(),
            case["expected_error"].as_str().unwrap(),
            "{}",
            case["id"]
        );
    }
}

fn run_text_format_boundary_case(case: &serde_json::Value) {
    let scalar = "x".repeat(case["scalar_bytes"].as_u64().unwrap() as usize);
    let template = "{0}".repeat(case["references"].as_u64().unwrap() as usize);
    let values = [conduit_std::FormatValueRef {
        name: None,
        value: conduit_std::FormatScalarRef::Text(&scalar),
    }];
    let mut output = [0; conduit_std::FORMAT_MAX_OUTPUT_BYTES];
    let result = conduit_std::format_text_into(&template, &values, &mut output);
    if let Some(expected) = case.get("expected_output_bytes") {
        assert_eq!(
            result.unwrap(),
            expected.as_u64().unwrap() as usize,
            "{}",
            case["id"]
        );
    } else {
        assert_eq!(
            result.unwrap_err().code(),
            case["expected_error"].as_str().unwrap(),
            "{}",
            case["id"]
        );
    }
}

#[test]
fn text_lines_join_fixture_freezes_semantics_and_boundaries() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c4/text-lines-join-v1.json"
    ))
    .unwrap();
    let ids = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    for required in [
        "lf-crlf-and-empty-lines",
        "delimiter-split-across-chunks",
        "chunk-boundary-independence",
        "empty-input",
        "final-unterminated-line",
        "invalid-utf8",
        "maximum-line",
        "oversized-line",
        "join-zero",
        "join-one",
        "join-many-and-separator-boundaries",
        "join-maximum-output",
        "join-output-overflow",
        "open-ended-input-rejected",
        "cancel-retained-line",
        "cancel-retained-items",
        "unsupported-provider",
        "hosted-deterministic-equivalence",
    ] {
        assert!(ids.contains(required), "fixture covers {required}");
    }

    fn split(chunks: &[&[u8]]) -> Result<Vec<String>, conduit_std::LineError> {
        let mut state = conduit_std::LinesState::new();
        let mut result = Vec::new();
        let mut output = [0; conduit_std::LINES_MAX_LINE_BYTES];
        for chunk in chunks {
            for byte in *chunk {
                if state.push_byte(*byte)? {
                    let length = state.take_ready(&mut output)?.unwrap();
                    result.push(core::str::from_utf8(&output[..length]).unwrap().to_owned());
                }
            }
        }
        if state.finish()? {
            let length = state.take_ready(&mut output)?.unwrap();
            result.push(core::str::from_utf8(&output[..length]).unwrap().to_owned());
        }
        Ok(result)
    }

    let expected = vec!["alpha".to_owned(), "".to_owned(), "beta".to_owned()];
    assert_eq!(split(&[b"alpha\r\n\nbeta\n"]).unwrap(), expected);
    assert_eq!(
        split(&[b"alpha\r", b"\n", b"\nbe", b"ta\n"]).unwrap(),
        expected
    );
    assert_eq!(split(&[]).unwrap(), Vec::<String>::new());
    assert_eq!(split(&[b"tail"]).unwrap(), vec!["tail".to_owned()]);
    assert_eq!(
        split(&[&[0xff, b'\n']]),
        Err(conduit_std::LineError::InvalidUtf8)
    );

    let mut output = [0; conduit_std::JOIN_MAX_OUTPUT_BYTES];
    assert_eq!(conduit_std::join_text_into(&[], ",", &mut output), Ok(0));
    assert_eq!(
        conduit_std::join_text_into(&["one"], ",", &mut output),
        Ok(3)
    );
    let length = conduit_std::join_text_into(&["one", "two", "three"], " / ", &mut output).unwrap();
    assert_eq!(&output[..length], b"one / two / three");

    let lines = STANDARD_CATALOG
        .iter()
        .find(|entry| entry.contract.id.as_str() == "std/text/lines")
        .unwrap();
    let join = STANDARD_CATALOG
        .iter()
        .find(|entry| entry.contract.id.as_str() == "std/text/join")
        .unwrap();
    assert_eq!(lines.contract.inputs[0].delivery.as_str(), "finite-batch");
    assert_eq!(lines.contract.outputs[0].delivery.as_str(), "stream");
    assert_eq!(join.contract.inputs[0].terminal.as_str(), "finite");
    assert_eq!(
        join.limits.retained_values,
        conduit_std::JOIN_MAX_ITEMS as u32
    );
    assert_eq!(
        join.limits.retained_bytes,
        (conduit_std::JOIN_MAX_ITEMS * conduit_std::JOIN_MAX_ITEM_BYTES) as u64
    );
    assert!(!lines.required_support.constrained);
    assert!(!join.required_support.constrained);
}
