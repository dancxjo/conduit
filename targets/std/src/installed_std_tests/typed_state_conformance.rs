use super::{host, installed_std, RecordingTimer};
use conduit_core::*;
use conduit_form::{
    KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature,
    StartupParameterSignature,
};
use conduit_semantic_catalog::state_value::*;

fn state_bytes(value: &StructuredInfoValue) -> Vec<u8> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => bytes.to_vec(),
        _ => value.canonical_bytes().unwrap(),
    }
}

fn fixture(
    allow_retained_current: bool,
    canonical_keep: bool,
    optional_keep: bool,
    keep_duration: &str,
) -> (
    conduit_form::CheckedForm,
    HostAdvertisement,
    StructuredInfoValue,
) {
    let payload_ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let ty = if optional_keep {
        optional_info_type(payload_ty.clone()).unwrap()
    } else {
        payload_ty.clone()
    };
    let next_payload =
        StructuredInfoValue::leaf(payload_ty.clone(), InfoBool::FALSE.encode().to_vec()).unwrap();
    let next = if optional_keep {
        StructuredInfoValue::variant(ty.clone(), "some", next_payload).unwrap()
    } else {
        next_payload
    };
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    startup.insert_structured_type("Cell", ty.clone()).unwrap();
    install_state_value_kind("Cell", &ty, &next, &mut startup, &mut profile).unwrap();
    startup
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    // This fixture supplies two external values and closes. The State Kind and
    // installed adapter are production paths; this is not physical input proof.
    let fixture_kind = match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind) => kind.clone(),
        _ => ty.profile().unwrap().value_kind().clone(),
    };
    let mut source = installed_std::test_structured_selector::raw_source_offer(
        installed_std::test_structured_selector::SOURCE_KIND,
        fixture_kind.as_str(),
    );
    source.startup_parameters[0].name = "values".into();
    source.host_calls = vec![wait_host_call_requirement()];
    source.resource_requirements = vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)];
    let next_bytes = state_bytes(&next);
    let mut entry = installed_std::test_structured_selector::raw_configuration(&next_bytes)
        .pop()
        .unwrap();
    entry.key = "values".into();
    if let ConfigurationValue::Text(encoded) = &mut entry.value {
        if optional_keep {
            encoded.clear();
        } else {
            *encoded = format!("{encoded},{encoded}");
        }
    }
    let ConfigurationValue::Text(default) = &entry.value else {
        panic!("fixture uses text configuration")
    };
    startup
        .insert(KindSignature {
            kind: source.kind_id.as_str().into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "values".into(),
                value_type: "Text".into(),
                default: Some(format!("\"{default}\"")),
            }],
        })
        .unwrap();
    let source_contract = KindConfigurationField {
        key: entry.key.clone(),
        default_value: entry.value.clone(),
        rule: KindConfigurationRule::TextBytes { maximum: 256 },
    };
    source.semantic_contract.configuration = vec![source_contract.clone()];
    profile
        .insert(KindProjection {
            kind_id: source.kind_id.clone(),
            kind_contract_revision: source.kind_contract_revision.clone(),
            inputs: source.inputs.clone(),
            outputs: source.outputs.clone(),
            configuration: vec![source_contract],
        })
        .unwrap();
    let initial_payload =
        StructuredInfoValue::leaf(payload_ty, InfoBool::TRUE.encode().to_vec()).unwrap();
    let initial = if optional_keep {
        StructuredInfoValue::variant(ty.clone(), "some", initial_payload).unwrap()
    } else {
        initial_payload
    };
    let encode = |value: &StructuredInfoValue| {
        let bytes = state_bytes(value);
        let entry = installed_std::test_structured_selector::raw_configuration(&bytes)
            .pop()
            .unwrap();
        let ConfigurationValue::Text(text) = entry.value else {
            unreachable!()
        };
        text
    };
    let mut expected = if optional_keep {
        encode(&initial)
    } else {
        format!("{},{},{}", encode(&initial), encode(&next), encode(&next))
    };
    let expectation_key = if allow_retained_current {
        "choices"
    } else {
        "values"
    };
    if allow_retained_current {
        expected = format!(
            "{}|{},{},{}",
            encode(&initial),
            encode(&next),
            encode(&next),
            encode(&next)
        );
    }
    let mut sink = installed_std::test_structured_selector::raw_sink_offer(
        installed_std::test_structured_selector::SINK_KIND,
        fixture_kind.as_str(),
    );
    sink.inputs[0].temporal = PortTemporal::Current;
    sink.startup_parameters[0].name = expectation_key.into();
    startup
        .insert(KindSignature {
            kind: sink.kind_id.as_str().into(),
            startup_parameters: vec![StartupParameterSignature {
                name: expectation_key.into(),
                value_type: "Text".into(),
                default: Some(format!("\"{expected}\"")),
            }],
        })
        .unwrap();
    let sink_contract = KindConfigurationField {
        key: expectation_key.into(),
        default_value: ConfigurationValue::Text(expected.clone()),
        rule: KindConfigurationRule::TextBytes { maximum: 256 },
    };
    sink.semantic_contract.configuration = vec![sink_contract.clone()];
    profile
        .insert(KindProjection {
            kind_id: sink.kind_id.clone(),
            kind_contract_revision: sink.kind_contract_revision.clone(),
            inputs: sink.inputs.clone(),
            outputs: vec![],
            configuration: vec![sink_contract],
        })
        .unwrap();
    let cell = if optional_keep {
        format!("cell: keep Boolean?(true) {keep_duration}")
    } else if canonical_keep {
        format!("cell: keep Boolean(true) {keep_duration}")
    } else {
        "cell: state/value(initial = true)".into()
    };
    let form = conduit_form::parse_with_startup(
        &format!(
            "form retained {{\n source: conduit-test/structured-source\n {cell}\n sink: conduit-test/structured-sink\n source.output >> cell.next\n cell.current >> sink.input\n}}\n"
        ),
        &startup,
        &profile,
    )
    .unwrap();
    let mut advertisement = host("typed-state-host").advertisement().clone();
    if optional_keep {
        advertisement
            .capabilities
            .retain(|offer| offer.kind_id.as_str() != STATE_VALUE_KIND);
        advertisement
            .capabilities
            .push(conduit_std_offers::state_value_std_offer("Cell", &ty, &next).unwrap());
    }
    advertisement.capabilities.extend([source, sink]);
    (form, advertisement, next)
}

fn plans(
    form: &conduit_form::CheckedForm,
    advertisement: &HostAdvertisement,
    maximum: u32,
) -> (Plan, Plan) {
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_placements(form, &hosts).unwrap();
    let ordinary = conduit_planner::plan_with_connection_limits(
        form,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        1,
        64,
    )
    .unwrap();
    let state = derive_state_boundary(form, &GearId::from("retained/cell"), maximum).unwrap();
    let sealed =
        conduit_planner::state_delay::plan::seal_state_plan(form, &ordinary, vec![state]).unwrap();
    (ordinary, sealed)
}

fn run(
    advertisement: &HostAdvertisement,
    fragment: &PlanFragment,
    sources: Option<&mut Vec<crate::state_value::RetainedTypedState>>,
) -> Result<crate::state_value::RetainedStdRun, String> {
    let mut output = Vec::with_capacity(2048);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(2),
    };
    let mut execution_host = host("typed-state-host");
    execution_host.advertisement = advertisement.clone();
    execution_host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(advertisement).unwrap();
    let continuity = sources.is_some();
    let result = if let Some(sources) = sources {
        execution_host
            .run_fragment_continuing_to(
                fragment.clone(),
                sources,
                &mut output,
                &mut timer,
                &crate::RunControl::default(),
            )
            .map_err(|failure| failure.reason)
    } else {
        execution_host.run_fragment_retaining_to(
            fragment.clone(),
            &mut output,
            &mut timer,
            &crate::RunControl::default(),
        )
    };
    // The host releases old realization reservations before yielding State.
    let reservation = execution_host
        .kernel_resources
        .prepare_and_reserve_with_continuity(advertisement, fragment, continuity)
        .unwrap();
    execution_host
        .kernel_resources
        .release(reservation)
        .unwrap();
    result
}

#[test]
fn typed_state_runs_in_the_installed_kernel_and_unsealed_state_refuses() {
    let (form, advertisement, next) = fixture(false, false, false, "for this play");
    let (ordinary, sealed) = plans(&form, &advertisement, 60);

    assert!(run(&advertisement, &ordinary.fragments[0], None)
        .err()
        .unwrap()
        .contains("lacks sealed State"));
    let report = run(&advertisement, &sealed.fragments[0], None)
        .expect("typed State executes through the installed kernel");
    assert_eq!(report.states.len(), 1);
    let retained = report.states[0].provenance();
    assert_eq!(retained.current_value, state_bytes(&next));
    assert_eq!(retained.generation, 2);
    assert_eq!(retained.source_play.plan_id, sealed.plan_id);
    assert_eq!(retained.source_form, form.identity());
    let kernel = report.report.kernel.unwrap();
    assert_eq!(retained.source_play.active_play_id, kernel.active_play_id);
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn initialized_keep_plans_and_runs_as_installed_typed_state() {
    let (form, advertisement, next) = fixture(false, true, false, "for this play");
    let state_gear = form
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == STATE_VALUE_KIND)
        .unwrap();
    let state_offer = advertisement
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == STATE_VALUE_KIND)
        .unwrap();
    assert_eq!(state_gear.checked_front(), state_offer.checked_front());
    assert!(state_gear.accepts_semantic_contract(state_offer));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        1,
        64,
    )
    .unwrap();
    let [state] = plan.fragments[0].states.as_slice() else {
        panic!("canonical KEEP must seal exactly one State boundary")
    };
    assert_eq!(state.lifetime, StateLifetime::Play);
    assert_eq!(state.value_kind.as_str(), BOOL_INFO_ID);
    conduit_semantic_catalog::state_value::validate_state_placement(
        plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.gear_id == state.gear_id)
            .unwrap(),
        state,
    )
    .unwrap();

    let report = run(&advertisement, &plan.fragments[0], None)
        .expect("canonical KEEP executes through the production typed State Back");
    assert_eq!(
        report.states[0].provenance().current_value,
        state_bytes(&next)
    );
    assert_eq!(
        report.states[0].provenance().source_play.plan_id,
        plan.plan_id
    );
}

#[test]
fn keep_duration_requires_exact_back_support_before_planning() {
    let plan = |duration: &str, maximum_lifetime: StateLifetime| {
        let (form, mut advertisement, _) = fixture(false, true, false, duration);
        let offer = advertisement
            .capabilities
            .iter_mut()
            .find(|offer| offer.kind_id.as_str() == STATE_VALUE_KIND)
            .unwrap();
        offer.state_retention = Some(StateRetentionSupport { maximum_lifetime });
        let hosts = [advertisement];
        let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
        let result = conduit_planner::plan_with_connection_limits(
            &form,
            &hosts,
            &placements,
            &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
            1,
            100,
        );
        (form, hosts, result)
    };

    for duration in ["for this step", "for this play"] {
        let (_, _, result) = plan(duration, StateLifetime::Play);
        assert!(result.is_ok(), "{duration} must fit the std Back");
    }
    for duration in [
        "for this wake",
        "for this boot",
        "for this body",
        "for life",
    ] {
        let (_, _, result) = plan(duration, StateLifetime::Play);
        assert!(matches!(
            result,
            Err(conduit_planner::PlannerError::StateRetentionUnsupported(_))
        ));
    }

    let (body_form, _, body_refusal) = plan("for this body", StateLifetime::Play);
    let (life_form, _, life_refusal) = plan("for life", StateLifetime::Play);
    assert_eq!(body_form.checked_form_id, life_form.checked_form_id);
    assert_eq!(body_refusal, life_refusal);

    let (form, hosts, result) = plan("for this body", StateLifetime::Body);
    let admitted = result.unwrap();
    let [state] = admitted.fragments[0].states.as_slice() else {
        panic!("Body-lived keep must seal one State boundary")
    };
    assert_eq!(state.lifetime, StateLifetime::Body);
    let selected = admitted.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.gear_id == state.gear_id)
        .unwrap();
    let offered = hosts[0]
        .capabilities
        .iter()
        .find(|offer| offer.capability_id == selected.capability_id)
        .unwrap();
    assert_eq!(
        selected.implementation_id,
        offered.implementation.implementation_id
    );
    assert_eq!(selected.artifact_id, offered.implementation.artifact_id);
    assert_eq!(admitted.checked_form_id, form.checked_form_id);
}

#[test]
fn body_durable_keep_selects_and_executes_only_the_sealed_durable_back() {
    let (form, mut advertisement, initial) = fixture(false, true, false, "for life");
    advertisement
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != STATE_VALUE_KIND);
    let value_type = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    advertisement.capabilities.push(
        conduit_std_offers::state_value_durable_std_offer("Cell", &value_type, &initial).unwrap(),
    );
    advertisement.resources.push(ResourceOffer {
        pool_id: ResourcePoolId::from("pool/body-durable-state"),
        class_id: ResourceClassId::from(conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS),
        capacity_units: 1,
        compute: None,
        content: None,
    });
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));

    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        1,
        48,
    )
    .unwrap();
    let [state] = plan.fragments[0].states.as_slice() else {
        panic!("Body-lived keep must seal one State boundary")
    };
    assert_eq!(state.lifetime, StateLifetime::Body);
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.gear_id == state.gear_id)
        .unwrap();
    assert_eq!(
        placement.implementation_id.as_str(),
        conduit_std_offers::STATE_VALUE_DURABLE_STD_IMPLEMENTATION
    );
    assert_eq!(placement.host_calls.len(), 2);
    assert_eq!(placement.resources.len(), 1);

    let root = std::env::temp_dir().join(format!(
        "conduit-installed-durable-state-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let mut execution_host = host("typed-state-host");
    execution_host.advertisement = advertisement.clone();
    execution_host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&advertisement).unwrap();
    let mut output = Vec::with_capacity(2048);
    let mut timer = RecordingTimer {
        waits: Vec::with_capacity(2),
    };
    let body: conduit_body::BodyId = serde_json::from_str("\"body/durable-notebook\"").unwrap();
    let report = execution_host
        .run_body_durable_fragment_to(
            &body,
            &root,
            plan.fragments[0].clone(),
            &mut output,
            &mut timer,
            &crate::RunControl::default(),
        )
        .expect("Body-durable State executes through its exact installed Back");
    assert!(report.kernel.is_some());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn canonical_four_kib_text_keep_plans_on_the_durable_back_and_larger_refuses() {
    let text_type = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).unwrap();
    let initial = StructuredInfoValue::leaf(text_type.clone(), Vec::new()).unwrap();
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_state_value_kind("Text", &text_type, &initial, &mut startup, &mut profile).unwrap();
    startup
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let source = |maximum| {
        format!(
            "form durable-note (\n >> write: Text <= {maximum}B\n current: $Text <= {maximum}B >>\n) {{\n note: keep Text(\"\") <= {maximum}B for life\n write >> note\n note >> current\n}}\n"
        )
    };
    let checked = conduit_form::parse_with_startup(&source(4096), &startup, &profile).unwrap();
    let mut advertisement = host("durable-text-host").advertisement().clone();
    advertisement
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != STATE_VALUE_KIND);
    advertisement.capabilities.push(
        conduit_std_offers::state_value_durable_std_offer("Text", &text_type, &initial).unwrap(),
    );
    advertisement.resources.push(ResourceOffer {
        pool_id: ResourcePoolId::from("pool/durable-text-state"),
        class_id: ResourceClassId::from(conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS),
        capacity_units: 1,
        compute: None,
        content: None,
    });
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let hosts = [advertisement];
    let placements = conduit_planner::default_placements(&checked, &hosts).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &checked,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        1,
        conduit_data::MAXIMUM_DATA_TEXT_BYTES,
    )
    .unwrap();
    assert_eq!(plan.fragments[0].states[0].maximum_value_bytes, 4096);
    assert_eq!(
        plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.gear_id.as_str() == "durable-note/note")
            .unwrap()
            .implementation_id
            .as_str(),
        conduit_std_offers::STATE_VALUE_DURABLE_STD_IMPLEMENTATION
    );
    assert_eq!(
        crate::installed_std::state_storage_profile()
            .state_storage()
            .unwrap()
            .1,
        conduit_data::MAXIMUM_DATA_TEXT_BYTES
    );

    let oversized = conduit_form::parse_with_startup(&source(4097), &startup, &profile).unwrap();
    let placements = conduit_planner::default_placements(&oversized, &hosts).unwrap();
    assert!(conduit_planner::plan_with_connection_limits(
        &oversized,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        1,
        conduit_data::MAXIMUM_DATA_TEXT_BYTES + 1,
    )
    .is_err());
}

#[test]
fn optional_keep_plans_runs_and_retains_canonical_some_value() {
    let (form, advertisement, _next) = fixture(false, true, true, "for this play");
    let state_gear = form
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == STATE_VALUE_KIND)
        .unwrap();
    let optional_kind =
        optional_info_type(StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap())
            .unwrap()
            .profile()
            .unwrap()
            .value_kind()
            .clone();
    assert_eq!(state_gear.inputs[0].value_kind, optional_kind);
    assert_eq!(state_gear.outputs[0].value_kind, optional_kind);

    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        1,
        100,
    )
    .unwrap();
    let [state] = plan.fragments[0].states.as_slice() else {
        panic!("optional KEEP must seal exactly one State boundary")
    };
    assert_eq!(state.value_kind, optional_kind);
    assert_eq!(state.lifetime, StateLifetime::Play);

    let report = run(&advertisement, &plan.fragments[0], None)
        .expect("optional KEEP executes through the production typed State Back");
    assert_eq!(
        report.states[0].provenance().current_value.as_slice(),
        state.initial_value.as_deref().unwrap()
    );
    assert_eq!(report.states[0].provenance().generation, 0);
}

#[test]
fn optional_keep_continuity_preserves_exact_variant_and_generation() {
    use conduit_planner::state_delay::continuity::{
        seal_state_continuity, StateContinuityApproval,
    };
    let (form, source_host, _next) = fixture(false, true, true, "for this play");
    let plan_for = |advertisement: &HostAdvertisement| {
        let hosts = [advertisement.clone()];
        let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
        conduit_planner::plan_with_connection_limits(
            &form,
            &hosts,
            &placements,
            &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
            1,
            100,
        )
        .unwrap()
    };
    let source = plan_for(&source_host);
    let first = run(&source_host, &source.fragments[0], None).unwrap();
    let mut states = first.states;
    assert_eq!(states[0].provenance().generation, 0);
    let preserved = states[0].provenance().current_value.clone();

    let mut destination_host = source_host.clone();
    destination_host.boot_id = "optional-replacement-boot".into();
    let candidate = plan_for(&destination_host);
    let replacement = seal_state_continuity(
        &source,
        &candidate,
        states[0].provenance().clone(),
        &StateContinuityApproval {
            source_plan: source.plan_id.clone(),
            destination_plan: candidate.plan_id.clone(),
            state: states[0].provenance().source_state.clone(),
            maximum_value_bytes: candidate.fragments[0].states[0].maximum_value_bytes,
        },
    )
    .unwrap();
    let second = run(
        &destination_host,
        &replacement.fragments[0],
        Some(&mut states),
    )
    .unwrap();
    assert!(states.is_empty());
    assert_eq!(second.states[0].provenance().generation, 0);
    assert_eq!(second.states[0].provenance().current_value, preserved);
    assert_eq!(
        second.states[0].provenance().source_play.plan_id,
        replacement.plan_id
    );
}

#[test]
fn public_host_replaces_play_with_owned_state_and_fresh_boot_without_semantic_reset() {
    use conduit_planner::state_delay::continuity::{
        seal_state_continuity, StateContinuityApproval,
    };
    let (form, source_host, next) = fixture(true, false, false, "for this play");
    let (_, source) = plans(&form, &source_host, 60);
    let first = run(&source_host, &source.fragments[0], None).unwrap();
    let old_play = first.report.kernel.as_ref().unwrap().active_play_id.clone();
    let mut states = first.states;
    assert_eq!(states[0].provenance().generation, 2);
    let mut destination_host = source_host.clone();
    destination_host.boot_id = "replacement-boot".into();
    let (_, candidate) = plans(&form, &destination_host, 64);
    let replacement = seal_state_continuity(
        &source,
        &candidate,
        states[0].provenance().clone(),
        &StateContinuityApproval {
            source_plan: source.plan_id.clone(),
            destination_plan: candidate.plan_id.clone(),
            state: states[0].provenance().source_state.clone(),
            maximum_value_bytes: 64,
        },
    )
    .unwrap();
    // A structurally valid forged snapshot cannot consume the actual owner.
    let mut fragments = replacement.fragments.clone();
    fragments[0].states[0].retained.as_mut().unwrap().generation += 1;
    let forged = seal_plan(form.identity(), fragments);
    assert!(run(&destination_host, &forged.fragments[0], Some(&mut states)).is_err());
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].provenance().generation, 2);
    let second = run(
        &destination_host,
        &replacement.fragments[0],
        Some(&mut states),
    )
    .unwrap();
    assert!(states.is_empty());
    let retained = second.states[0].provenance();
    assert_eq!(
        retained.generation, 4,
        "replacement must not renew State generation"
    );
    assert_eq!(retained.current_value, state_bytes(&next));
    assert_eq!(retained.source_form, form.identity());
    assert_eq!(retained.source_play.plan_id, replacement.plan_id);
    assert_eq!(retained.source_play.boot_id, destination_host.boot_id);
    assert_ne!(retained.source_play.active_play_id, old_play);
    assert_eq!(second.report.kernel.unwrap().post_play_start_allocations, 0);
}
