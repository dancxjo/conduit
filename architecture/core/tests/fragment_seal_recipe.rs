//! Canonical identity equality only, never an offline execution approval.
use conduit_core::*;
#[path = "common/sealed_state.rs"]
mod fixture;
fn fragment(boot: &str) -> PlanFragment {
    let mut f = fixture::fragment();
    f.states.clear();
    f.boot_id = boot.into();
    f.offer_generation = OfferGeneration(boot.len() as u64 + 1);
    let p = &mut f.placements[0];
    p.boot_id = boot.into();
    p.offer_generation = f.offer_generation;
    p.placement_id = format!("placement/{boot}").into();
    p.base = Some(BaseProviderBinding {
        base_id: "base".into(),
        provider_instance_id: format!("provider/{boot}").into(),
        provider_generation: boot.len() as u64 + 2,
        implementation_id: "base/impl".into(),
        mechanism_family: "mechanism".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
    });
    p.resources = vec![ResourceBinding {
        pool_id: format!("pool/{boot}").into(),
        class_id: "memory".into(),
        units: 64,
        protected: None,
        compute: None,
        content: None,
    }];
    p.authority = vec![AuthorityBinding {
        grant_id: format!("grant/{boot}").into(),
        contract_id: "authority".into(),
        host_call_contract_id: "call".into(),
        subject_kind: "kind".into(),
        host_id: p.host_id.clone(),
        boot_id: p.boot_id.clone(),
        capability_id: "cap".into(),
    }];
    p.pool_references = vec![p.resources[0].pool_id.as_str().into()];
    f.fore_ports = vec![PlannedForePort {
        front_port_id: "output".into(),
        direction: PortDirection::Output,
        placement_id: p.placement_id.clone(),
        gear_port_id: "current".into(),
        value_kind: "fixture/byte@1".into(),
        value_contract: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
    }];
    f.connections = vec![PlannedConnection {
        connection_id: "source/local-cord".into(),
        source_placement_id: p.placement_id.clone(),
        source_port_id: "current".into(),
        sink_placement_id: p.placement_id.clone(),
        sink_port_id: "next".into(),
        value_kind: "fixture/byte@1".into(),
        resource: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        selected_line: None,
        admitted_lines: vec![],
        item_capacity: 1,
        byte_capacity: 1,
    }];
    f.startup_order = vec![p.placement_id.clone()];
    f.expected_terminals = vec![ExpectedTerminal::PlacementCompleted(p.placement_id.clone())];
    f.expected_sign = vec![
        ExpectedSign::PlacementPrepared(p.placement_id.clone()),
        ExpectedSign::PlacementTerminal(p.placement_id.clone()),
    ];
    f
}
fn values(f: &PlanFragment) -> Vec<FragmentSealBinding<'_>> {
    use FragmentScopeRole as R;
    let p = &f.placements[0];
    let mut out = Vec::new();
    let mut text = |placement, role, entry, value| {
        out.push(FragmentSealBinding {
            field: FragmentSealField {
                placement,
                role,
                entry,
            },
            value: FragmentSealValue::Text(value),
        })
    };
    text(None, R::Host, 0, f.host_id.as_str());
    text(None, R::Boot, 0, f.boot_id.as_str());
    text(Some(0), R::PlacementId, 0, p.placement_id.as_str());
    text(Some(0), R::Host, 0, p.host_id.as_str());
    text(Some(0), R::Boot, 0, p.boot_id.as_str());
    text(
        Some(0),
        R::ProviderInstance,
        0,
        p.base.as_ref().unwrap().provider_instance_id.as_str(),
    );
    text(Some(0), R::ResourcePool, 0, p.resources[0].pool_id.as_str());
    text(Some(0), R::PoolReference, 0, p.pool_references[0].as_str());
    text(
        Some(0),
        R::AuthorityGrant,
        0,
        p.authority[0].grant_id.as_str(),
    );
    text(
        Some(0),
        R::AuthorityHost,
        0,
        p.authority[0].host_id.as_str(),
    );
    text(
        Some(0),
        R::AuthorityBoot,
        0,
        p.authority[0].boot_id.as_str(),
    );
    for (placement, role, value) in [
        (None, R::OfferGeneration, f.offer_generation.0),
        (Some(0), R::OfferGeneration, p.offer_generation.0),
        (
            Some(0),
            R::ProviderGeneration,
            p.base.as_ref().unwrap().provider_generation,
        ),
    ] {
        out.push(FragmentSealBinding {
            field: FragmentSealField {
                placement,
                role,
                entry: 0,
            },
            value: FragmentSealValue::U64(value),
        });
    }
    out
}
#[test]
fn same_encoder_recipe_matches_complete_core_sealing_on_two_actual_scopes() {
    let original = fragment("boot/original");
    let recipe = FragmentSealRecipe::capture(
        &original,
        FragmentSealLimits {
            literal_bytes: 8192,
            pieces: 256,
        },
    )
    .unwrap();
    for boot in ["boot/new-one", "boot/new-two-longer"] {
        let f = fragment(boot);
        let bindings = values(&f);
        let digest = recipe.digest(&bindings).unwrap();
        let expected = compute_fragment_id(&f);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(expected.as_str(), hex);
        let pieces: Vec<_> = recipe.pieces().collect();
        assert_eq!(
            fragment_seal_recipe_digest(&pieces, &bindings).unwrap(),
            digest
        );
    }
}
#[test]
fn missing_duplicate_extra_and_wrong_type_bindings_refuse() {
    let f = fragment("boot/original");
    let recipe = FragmentSealRecipe::capture(
        &f,
        FragmentSealLimits {
            literal_bytes: 8192,
            pieces: 256,
        },
    )
    .unwrap();
    let mut bindings = values(&f);
    bindings.pop();
    assert!(recipe.digest(&bindings).is_err());
    bindings = values(&f);
    bindings.push(bindings[0]);
    assert!(recipe.digest(&bindings).is_err());
    bindings = values(&f);
    bindings[0].value = FragmentSealValue::U64(1);
    assert!(recipe.digest(&bindings).is_err());
    bindings = values(&f);
    bindings.push(FragmentSealBinding {
        field: FragmentSealField {
            placement: Some(99),
            role: FragmentScopeRole::Boot,
            entry: 0,
        },
        value: FragmentSealValue::Text("wrong"),
    });
    assert!(recipe.digest(&bindings).is_err());
}
#[test]
fn preparation_pressure_and_unhandled_tail_fail_closed() {
    let mut f = fragment("boot/original");
    for limits in [
        FragmentSealLimits {
            literal_bytes: 1,
            pieces: 256,
        },
        FragmentSealLimits {
            literal_bytes: 8192,
            pieces: 1,
        },
    ] {
        assert!(matches!(
            FragmentSealRecipe::capture(&f, limits),
            Err(FragmentSealRefusal::Capacity)
        ));
    }
    f.states = fixture::fragment().states;
    assert!(matches!(
        FragmentSealRecipe::capture(
            &f,
            FragmentSealLimits {
                literal_bytes: 8192,
                pieces: 256
            }
        ),
        Err(FragmentSealRefusal::Unsupported)
    ));
}
