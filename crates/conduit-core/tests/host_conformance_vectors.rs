use conduit_core::{
    CompatibilityOutcome, DescriptorRef, ExplicitSatisfactionRequirement,
    HOST_CONFORMANCE_PROFILE_SCHEMA_VERSION, HostClass, HostConformanceProfile,
    HostConformanceReason, HostExecutionMode, HostExtension, HostExtensionKind, Id,
    PinnedDescriptor, ProviderBindingRequest, ProviderBoundary, ProviderBounds,
    ProviderConformanceOutcome, ProviderConformanceResult, ProviderInventory,
    ProviderInventoryState, ProviderObservation, ProviderObservationState, SatisfactionMethod,
    SatisfactionObligation, SatisfactionPin, SatisfactionProof, SatisfactionReason,
    SatisfactionRole, SemanticHash, bind_provider, validate_host_conformance_profile,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn descriptor(id: &'static str, byte: u8) -> DescriptorRef<'static> {
    DescriptorRef {
        kind: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn satisfaction<'a>(
    contract: PinnedDescriptor<'a>,
    obligations: &'a [SatisfactionObligation<'a>],
) -> SatisfactionProof<'a> {
    let mut proof = SatisfactionProof {
        schema_version: 0,
        identity: hash(0),
        role: SatisfactionRole::Implementation,
        method: SatisfactionMethod::ProviderRule,
        required: DescriptorRef {
            kind: contract.id,
            schema_version: contract.schema_version,
            semantic_hash: contract.semantic_hash,
        },
        offered: descriptor("acme/implementation/weather", 12),
        provider: Some(SatisfactionPin {
            descriptor: descriptor("acme/provider/rules", 13),
        }),
        provider_rule: Some(Id("acme/rules/complete-v1")),
        policy: None,
        facets: &[],
        obligations,
        outcome: CompatibilityOutcome::Compatible,
        reason: SatisfactionReason::Satisfied,
        explanation: Id("acme/rules/all-obligations-proven"),
        explicit_requirement: ExplicitSatisfactionRequirement::None,
    };
    let mut scratch = [hash(0); 9];
    proof.identity = proof.semantic_hash(&mut scratch).unwrap();
    proof
}

fn implementation_obligations() -> [SatisfactionObligation<'static>; 9] {
    [
        "semantic-contract",
        "ports",
        "configuration",
        "representation",
        "ownership-lifetime",
        "lifecycle",
        "authority",
        "resources",
        "boundedness",
    ]
    .map(|id| SatisfactionObligation {
        id: Id(id),
        required_hash: hash(21),
        offered_hash: hash(21),
        outcome: CompatibilityOutcome::Compatible,
        reason: Id("acme/rules/proven"),
    })
}

const BOUNDS: ProviderBounds = ProviderBounds {
    maximum_in_flight: 4,
    maximum_foreign_queue: 0,
    maximum_memory_bytes: 65_536,
    maximum_cancellation_ticks: 10,
    maximum_evidence_events: 32,
};

fn profile<'a>(
    class: HostClass,
    mode: HostExecutionMode,
    _identity: SemanticHash,
    inventory: &'a [ProviderInventory<'a>],
    extensions: &'a [HostExtension<'a>],
) -> HostConformanceProfile<'a> {
    let mandatory = Box::leak(Box::new([pin("conduit/host/minimal-execution", 1)]));
    let mut profile = HostConformanceProfile {
        schema_version: HOST_CONFORMANCE_PROFILE_SCHEMA_VERSION,
        identity: hash(0),
        id: Id(class.as_str()),
        class,
        execution_mode: mode,
        mandatory_facts: mandatory,
        optional_providers: inventory,
        extensions,
    };
    let mut scratch = [hash(0); 80];
    profile.identity = profile.computed_semantic_hash(&mut scratch).unwrap();
    profile
}

fn observation(
    profile: PinnedDescriptor<'static>,
    bundle: PinnedDescriptor<'static>,
    state: ProviderObservationState,
) -> ProviderObservation<'static> {
    let mut observation = ProviderObservation {
        id: Id("acme/observation/weather"),
        identity: hash(0),
        profile,
        provider_bundle: bundle,
        host_report: pin("acme/host-report/linux", 32),
        state,
        time_basis: Id("clock/test"),
        observed_at_tick: 10,
        valid_until_tick: 20,
    };
    observation.identity = observation.computed_semantic_hash().unwrap();
    observation
}

fn result<'a>(
    contract: PinnedDescriptor<'a>,
    profile: PinnedDescriptor<'a>,
    satisfaction: SemanticHash,
    adapter: PinnedDescriptor<'a>,
    boundary: ProviderBoundary,
) -> ProviderConformanceResult<'a> {
    let facets = Box::leak(Box::new([pin("acme/facet/weather-reading", 16)]));
    let mut result = ProviderConformanceResult {
        schema_version: 0,
        identity: hash(0),
        required_contract: contract,
        implementation: pin("acme/implementation/weather", 12),
        artifact: pin("acme/artifact/weather", 14),
        adapter,
        profile,
        fixture_suite: pin("acme/fixtures/weather-v1", 15),
        offered_facets: facets,
        satisfaction_proof: satisfaction,
        boundary,
        outcome: ProviderConformanceOutcome::Passed,
        bounds: BOUNDS,
        time_basis: Id("clock/test"),
        observed_at_tick: 10,
        valid_until_tick: 30,
    };
    let mut scratch = [hash(0); 32];
    result.identity = result.computed_semantic_hash(&mut scratch).unwrap();
    result
}

fn bind<'a>(
    profile_value: HostConformanceProfile<'a>,
    profile_pin: PinnedDescriptor<'a>,
    observation: ProviderObservation<'a>,
    conformance: ProviderConformanceResult<'a>,
    required_type: PinnedDescriptor<'a>,
    offered_type: PinnedDescriptor<'a>,
    adapter: Option<PinnedDescriptor<'a>>,
) -> Result<conduit_core::ExactProviderBinding<'a>, HostConformanceReason> {
    let obligations = Box::leak(Box::new(implementation_obligations()));
    let proof = Box::leak(Box::new(satisfaction(
        conformance.required_contract,
        obligations,
    )));
    let request = ProviderBindingRequest {
        required_contract: conformance.required_contract,
        required_type,
        offered_type,
        explicit_adapter: adapter,
        provider_bundle: observation.provider_bundle,
        implementation: conformance.implementation,
        artifact: conformance.artifact,
        satisfaction: proof,
    };
    let mut scratch = [hash(0); 9];
    bind_provider(
        profile_pin,
        profile_value,
        observation,
        ProviderConformanceResult {
            satisfaction_proof: proof.identity,
            ..conformance
        },
        request,
        Id("clock/test"),
        12,
        &mut scratch,
    )
}

#[test]
fn required_cross_host_fixture_inventory_is_frozen() {
    let fixture = include_str!("../../../conformance/c5/cross-host-provider-conformance.json");
    let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(value["suite"], "conduit.cross-host-provider-conformance");
    assert_eq!(value["cases"].as_array().unwrap().len(), 24);
    for required in [
        "provider-fixture-alpha-pass",
        "provider-fixture-beta-pass",
        "firmware-honest-unsupported",
        "deterministic-pass",
        "describe-only-not-executable",
        "empty-provider-set",
        "linked-uninitialized",
        "provider-lost",
        "stale-observation",
        "fabricated-observation",
        "label-only-false-satisfaction",
        "custom-type-incompatible",
        "explicit-adapter-fixture-pass",
        "adapter-not-published",
        "wrong-artifact-digest",
        "protocol-mismatch",
        "interpreter-unavailable",
        "non-cancellable-worker",
        "hidden-foreign-queue",
        "discovery-no-install",
        "discovery-no-authority",
        "profile-schema-version-rejected",
    ] {
        assert!(
            value["cases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|case| case["id"] == required),
            "missing {required}"
        );
    }
}

#[test]
fn shared_host_matrix_keeps_optional_provider_states_distinct() {
    let contract = pin("acme/contract/weather", 2);
    let bundle = pin("acme/provider/weather", 3);
    let linked = [ProviderInventory {
        contract,
        provider_bundle: bundle,
        state: ProviderInventoryState::Linked,
    }];
    let unsupported = [ProviderInventory {
        state: ProviderInventoryState::Unsupported,
        ..linked[0]
    }];
    let classes = [
        HostClass::LinuxHosted,
        HostClass::BrowserWasm,
        HostClass::ConstrainedFirmware,
        HostClass::DeterministicTest,
    ];
    for (index, class) in classes.into_iter().enumerate() {
        assert_eq!(
            validate_host_conformance_profile(profile(
                class,
                HostExecutionMode::Executable,
                hash(50 + index as u8),
                if class == HostClass::ConstrainedFirmware {
                    &unsupported
                } else {
                    &linked
                },
                &[],
            )),
            Ok(())
        );
    }
    assert_eq!(
        validate_host_conformance_profile(profile(
            HostClass::DescribeOnly,
            HostExecutionMode::DescribeOnly,
            hash(60),
            &[],
            &[],
        )),
        Ok(())
    );
}

#[test]
fn unavailable_stale_lost_and_describe_only_fail_differently() {
    let contract = pin("acme/contract/weather", 2);
    let bundle = pin("acme/provider/weather", 3);
    let native = pin("acme/adapter/native", 4);
    let exact_type = pin("acme/type/weather", 5);
    let inventory = [ProviderInventory {
        contract,
        provider_bundle: bundle,
        state: ProviderInventoryState::Linked,
    }];
    let mut profile_pin = pin("acme/profile/linux", 6);
    let profile_value = profile(
        HostClass::LinuxHosted,
        HostExecutionMode::Executable,
        profile_pin.semantic_hash,
        &inventory,
        &[],
    );
    profile_pin.semantic_hash = profile_value.identity;
    let obligations = implementation_obligations();
    let proof = satisfaction(contract, &obligations);
    let conformance = result(
        contract,
        profile_pin,
        proof.identity,
        native,
        ProviderBoundary::Native,
    );
    for (state, expected) in [
        (
            ProviderObservationState::Uninitialized,
            HostConformanceReason::ProviderUninitialized,
        ),
        (
            ProviderObservationState::Lost,
            HostConformanceReason::ProviderLost,
        ),
    ] {
        assert_eq!(
            bind(
                profile_value,
                profile_pin,
                observation(profile_pin, bundle, state),
                conformance,
                exact_type,
                exact_type,
                None,
            ),
            Err(expected)
        );
    }
    let mut stale = observation(profile_pin, bundle, ProviderObservationState::Available);
    stale.valid_until_tick = 12;
    stale.identity = stale.computed_semantic_hash().unwrap();
    assert_eq!(
        bind(
            profile_value,
            profile_pin,
            stale,
            conformance,
            exact_type,
            exact_type,
            None,
        ),
        Err(HostConformanceReason::ObservationStale)
    );

    let mut describe_pin = pin("acme/profile/describe", 7);
    let describe_profile = profile(
        HostClass::DescribeOnly,
        HostExecutionMode::DescribeOnly,
        describe_pin.semantic_hash,
        &inventory,
        &[],
    );
    describe_pin.semantic_hash = describe_profile.identity;
    let mut describe_observation = ProviderObservation {
        profile: describe_pin,
        ..observation(profile_pin, bundle, ProviderObservationState::Available)
    };
    describe_observation.identity = describe_observation.computed_semantic_hash().unwrap();
    let mut describe_conformance = ProviderConformanceResult {
        profile: describe_pin,
        ..conformance
    };
    let mut conformance_scratch = [hash(0); 32];
    describe_conformance.identity = describe_conformance
        .computed_semantic_hash(&mut conformance_scratch)
        .unwrap();
    assert_eq!(
        bind(
            describe_profile,
            describe_pin,
            describe_observation,
            describe_conformance,
            exact_type,
            exact_type,
            None,
        ),
        Err(HostConformanceReason::DescribeOnly)
    );
}

#[test]
fn custom_type_requires_exact_identity_or_published_explicit_adapter() {
    let contract = pin("acme/contract/weather", 2);
    let bundle = pin("acme/provider/weather", 3);
    let adapter = pin("acme/adapter/celsius-to-kelvin", 4);
    let inventory = [ProviderInventory {
        contract,
        provider_bundle: bundle,
        state: ProviderInventoryState::Linked,
    }];
    let extensions = [
        HostExtension {
            kind: HostExtensionKind::Type,
            descriptor: pin("acme/type/celsius", 5),
        },
        HostExtension {
            kind: HostExtensionKind::Node,
            descriptor: contract,
        },
        HostExtension {
            kind: HostExtensionKind::Implementation,
            descriptor: pin("acme/implementation/weather", 12),
        },
        HostExtension {
            kind: HostExtensionKind::Adapter,
            descriptor: adapter,
        },
    ];
    let mut profile_pin = pin("acme/profile/linux", 6);
    let profile_value = profile(
        HostClass::LinuxHosted,
        HostExecutionMode::Executable,
        profile_pin.semantic_hash,
        &inventory,
        &extensions,
    );
    profile_pin.semantic_hash = profile_value.identity;
    let observation = observation(profile_pin, bundle, ProviderObservationState::Available);
    let obligations = implementation_obligations();
    let proof = satisfaction(contract, &obligations);
    let conformance = result(
        contract,
        profile_pin,
        proof.identity,
        adapter,
        ProviderBoundary::Native,
    );
    let celsius = pin("acme/type/celsius", 5);
    let kelvin = pin("acme/type/kelvin", 8);
    assert_eq!(
        bind(
            profile_value,
            profile_pin,
            observation,
            conformance,
            kelvin,
            celsius,
            None,
        ),
        Err(HostConformanceReason::AdapterAbsent)
    );
    let bound = bind(
        profile_value,
        profile_pin,
        observation,
        conformance,
        kelvin,
        celsius,
        Some(adapter),
    )
    .unwrap();
    assert_eq!(bound.adapter, adapter);
    assert_eq!(bound.bounds.maximum_foreign_queue, 0);
}

#[test]
fn provider_boundaries_do_not_change_the_shared_semantic_contract() {
    let contract = pin("acme/contract/weather", 2);
    let profile = pin("acme/profile/matrix", 6);
    let adapter = pin("acme/adapter/protocol-v1", 4);
    let obligations = implementation_obligations();
    let proof = satisfaction(contract, &obligations);
    for boundary in [
        ProviderBoundary::Native,
        ProviderBoundary::SupervisedProcess,
        ProviderBoundary::WasmBrowser,
        ProviderBoundary::FirmwareFfi,
    ] {
        let item = result(contract, profile, proof.identity, adapter, boundary);
        assert_eq!(item.required_contract, contract);
        assert!(item.bounds.maximum_in_flight > 0);
        assert!(item.bounds.maximum_cancellation_ticks > 0);
    }
}
