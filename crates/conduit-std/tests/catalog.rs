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
