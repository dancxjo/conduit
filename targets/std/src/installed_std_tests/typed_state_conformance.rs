use super::{host, installed_std, RecordingTimer};
use conduit_core::*;
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};
use conduit_semantic_catalog::state_value::*;

#[test]
fn typed_state_runs_in_the_installed_kernel_and_unsealed_state_refuses() {
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
    let expected = format!("{},{},{}", encode(&initial), encode(&next), encode(&next));
    let mut sink = installed_std::test_structured_selector::offer(&ty, PortDirection::Input);
    sink.inputs[0].temporal = PortTemporal::Current;
    sink.startup_parameters[0].name = "values".into();
    startup
        .insert(KindSignature {
            kind: sink.kind_id.as_str().into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "values".into(),
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
                key: "values".into(),
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
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let ordinary = conduit_planner::plan_with_connection_limits(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        1,
        64,
    )
    .unwrap();
    let state = derive_state_boundary(&form, &GearId::from("retained/cell"), 64).unwrap();
    let sealed =
        conduit_planner::state_delay::plan::seal_state_plan(&form, &ordinary, vec![state]).unwrap();
    let run = |fragment: &PlanFragment| {
        let mut output = Vec::with_capacity(2048);
        let mut timer = RecordingTimer {
            waits: Vec::with_capacity(2),
        };
        installed_std::run_fragment(
            installed_std::InstalledRunHost {
                advertisement: &advertisement,
                playback: None,
                midi_input: None,
                midi_output: None,
                keyboard: None,
                local_model: None,
                vector_search: None,
                calendar: None,
            },
            fragment,
            0,
            &mut 0,
            &mut output,
            &mut timer,
            &crate::RunControl::default(),
        )
    };
    assert!(run(&ordinary.fragments[0])
        .unwrap_err()
        .contains("lacks sealed State"));
    let report =
        run(&sealed.fragments[0]).expect("typed State executes through the installed kernel");
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}
