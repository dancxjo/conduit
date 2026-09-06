use super::{host, installed_std, RecordingTimer};
use conduit_core::*;
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};
use conduit_semantic_catalog::state_value::*;

fn fixture(
    allow_retained_current: bool,
) -> (
    conduit_form::CheckedForm,
    HostAdvertisement,
    StructuredInfoValue,
) {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let next = StructuredInfoValue::leaf(ty.clone(), b"false".to_vec()).unwrap();
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    startup.insert_structured_type("Cell", ty.clone()).unwrap();
    install_state_value_kind("Cell", &ty, &next, &mut startup, &mut profile).unwrap();
    // This fixture supplies two external values and closes. The State Kind and
    // installed adapter are production paths; this is not physical input proof.
    let mut source = installed_std::test_structured_selector::offer(&ty, PortDirection::Output);
    source.startup_parameters[0].name = "values".into();
    source.host_operations = vec![wait_host_operation_requirement()];
    source.resource_requirements = vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)];
    let mut entry = installed_std::test_structured_selector::configuration(&next)
        .pop()
        .unwrap();
    entry.key = "values".into();
    if let ConfigurationValue::Text(encoded) = &mut entry.value {
        *encoded = format!("{encoded},{encoded}");
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
    profile
        .insert(KindDefinition {
            kind_id: source.kind_id.clone(),
            kind_contract_revision: source.kind_contract_revision.clone(),
            inputs: source.inputs.clone(),
            outputs: source.outputs.clone(),
            configuration: vec![ConfigurationField {
                key: entry.key,
                default_value: entry.value,
                validation: ConfigurationRule::TextBytes { maximum: 256 },
            }],
        })
        .unwrap();
    let initial = StructuredInfoValue::leaf(ty.clone(), b"true".to_vec()).unwrap();
    let encode = |value: &StructuredInfoValue| {
        let entry = installed_std::test_structured_selector::configuration(value)
            .pop()
            .unwrap();
        let ConfigurationValue::Text(text) = entry.value else {
            unreachable!()
        };
        text
    };
    let mut expected = format!("{},{},{}", encode(&initial), encode(&next), encode(&next));
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
    let mut sink = installed_std::test_structured_selector::offer(&ty, PortDirection::Input);
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
    profile
        .insert(KindDefinition {
            kind_id: sink.kind_id.clone(),
            kind_contract_revision: sink.kind_contract_revision.clone(),
            inputs: sink.inputs.clone(),
            outputs: vec![],
            configuration: vec![ConfigurationField {
                key: expectation_key.into(),
                default_value: ConfigurationValue::Text(expected),
                validation: ConfigurationRule::TextBytes { maximum: 256 },
            }],
        })
        .unwrap();
    let form = conduit_form::parse_with_startup(
        "form retained {\n source: conduit-test/structured-source\n cell: state/value(initial = true)\n sink: conduit-test/structured-sink\n source.output > cell.next\n cell.current > sink.input\n}\n", &startup, &profile,
    ).unwrap();
    let mut advertisement = host("typed-state-host").advertisement().clone();
    advertisement.capabilities.extend([
        source,
        conduit_std_offers::state_value_std_offer("Cell", &ty).unwrap(),
        sink,
    ]);
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
        crate::kernel_preparation::KernelResourceLedger::new(&advertisement).unwrap();
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
    // The Host releases old realization reservations before yielding State.
    let reservation = execution_host
        .kernel_resources
        .prepare_and_reserve_with_continuity(&advertisement, fragment, continuity)
        .unwrap();
    execution_host
        .kernel_resources
        .release(reservation)
        .unwrap();
    result
}

#[test]
fn typed_state_runs_in_the_installed_kernel_and_unsealed_state_refuses() {
    let (form, advertisement, next) = fixture(false);
    let (ordinary, sealed) = plans(&form, &advertisement, 60);

    assert!(run(&advertisement, &ordinary.fragments[0], None)
        .err()
        .unwrap()
        .contains("lacks sealed State"));
    let report = run(&advertisement, &sealed.fragments[0], None)
        .expect("typed State executes through the installed kernel");
    assert_eq!(report.states.len(), 1);
    let retained = report.states[0].provenance();
    assert_eq!(retained.current_value, next.canonical_bytes().unwrap());
    assert_eq!(retained.generation, 2);
    assert_eq!(retained.source_play.plan_id, sealed.plan_id);
    assert_eq!(retained.source_form, form.identity());
    let kernel = report.report.kernel.unwrap();
    assert_eq!(retained.source_play.active_play_id, kernel.active_play_id);
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn public_host_replaces_play_with_owned_state_and_fresh_boot_without_semantic_reset() {
    use conduit_planner::state_delay::continuity::{
        seal_state_continuity, StateContinuityApproval,
    };
    let (form, source_host, next) = fixture(true);
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
    assert_eq!(retained.current_value, next.canonical_bytes().unwrap());
    assert_eq!(retained.source_form, form.identity());
    assert_eq!(retained.source_play.plan_id, replacement.plan_id);
    assert_eq!(retained.source_play.boot_id, destination_host.boot_id);
    assert_ne!(retained.source_play.active_play_id, old_play);
    assert_eq!(second.report.kernel.unwrap().post_play_start_allocations, 0);
}
