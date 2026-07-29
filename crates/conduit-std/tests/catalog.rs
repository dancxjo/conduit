use conduit_std::{
    CatalogError, DeterministicProvider, FixtureClass, HostedProvider, ProviderProfile,
    ReferenceProvider, STANDARD_CATALOG, STANDARD_CATALOG_SCHEMA_VERSION, StandardFamily,
    TimeBasis, run_catalog_fixture, validate_catalog, validate_entry,
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
            || entry.contract.id.as_str() == "conduit.std/discard"
    }));
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
