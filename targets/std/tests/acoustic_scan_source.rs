//! The same checked DSP child participates in the existing authored scan syntax.
#[path = "common_acoustic_value/literal.rs"]
mod literal;
#[path = "common_acoustic_value/seed.rs"]
mod seed;
#[path = "common_acoustic_value/source.rs"]
mod source;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

#[test]
fn authored_scan_retains_the_exact_native_initial_cursor() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(check_scan)
        .unwrap()
        .join()
        .unwrap();
}
fn check_scan() {
    let child_source = source::composed_source();
    let owner = child_source
        .split("plot acoustic-frame-control (")
        .next()
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let types =
        check_syntax_document(&parse_syntax_document(&owner), &StartupCatalog::new()).unwrap();
    let stage = types
        .native_types
        .iter()
        .find(|ty| ty.name == "AcousticStage")
        .unwrap()
        .value_type
        .clone();
    let contract = conduit_core::CheckedValueContract::new(
        stage.profile().unwrap().value_kind().clone(),
        32768,
        vec![],
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_value_pair_kind(
        &contract,
        &stage,
        &contract,
        &stage,
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&child_source), &startup).unwrap();
    let (initial, _) = seed::seeds(&checked, &profiles);
    let initial_value = conduit_core::StructuredInfoValue::from_canonical_bytes(&initial).unwrap();
    let literal = literal::literal(&initial_value);
    let parent = format!(
        r#"
# Explicit identity transports the checked finite envelope into scan's state.
plot acoustic-combine (
 >> accumulator: AcousticStage <= 32768B
 >> item: AcousticStage <= 32768B
 combined: AcousticStage <= 32768B >>
) {{
 frame: acoustic-value-frame()
 accumulator >> frame.accumulator
 item >> frame.item
 frame.result >> combined
}}
plot acoustic-render (
 initial: AcousticStage
 >> ticks: AcousticStage...| <= 32768B
 states: AcousticStage...| <= 32768B >>
) {{
 samples: scan(initial, maximum-items = 4) acoustic-combine()
 ticks >> samples.item
 samples.combined >> states
}}
plot acoustic-scan-fixture (
 >> ticks: AcousticStage...| <= 32768B
 states: AcousticStage...| <= 32768B >>
) {{
 render: acoustic-render(initial = {literal})
 ticks >> render.ticks
 render.states >> states
}}
"#
    );
    let source = child_source + &parent;
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "acoustic-scan-fixture", &profiles).unwrap();
    assert_eq!(expanded.expanded.activations.len(), 1);
    assert_eq!(
        expanded.expanded.activations[0]
            .initial_accumulator_bytes
            .as_deref(),
        Some(initial.as_slice())
    );
    let combine =
        expand_canonical_plot_for_authoring(&checked, "acoustic-combine", &profiles).unwrap();
    assert_eq!(combine.expanded.gears.len(), 64);
    let mut capabilities = vec![conduit_std_host::flow_activation::value_scan_offer(
        &contract, &contract, &initial, 4,
    )
    .unwrap()];
    for gear in &combine.expanded.gears {
        let offer = if gear.kind_id.as_str() == conduit_semantic_catalog::VALUE_PAIR_KIND {
            conduit_std_offers::value_pair_offer(&contract, &stage, &contract, &stage).unwrap()
        } else {
            let conduit_core::ConfigurationValue::Text(encoded) = &gear.configuration[0].value
            else {
                panic!("program")
            };
            let program =
                conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            conduit_std_offers::pure_expression_local_offer(&program).unwrap()
        };
        if !capabilities
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            capabilities.push(offer);
        }
    }
    let mut advertisement = conduit_std_host::StdHost::new().advertisement().clone();
    advertisement.host_id = "fixture/acoustic-retained-scan".into();
    advertisement.capabilities = capabilities;
    let hosts = [advertisement];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let capacity = combine
        .expanded
        .gears
        .iter()
        .flat_map(|gear| gear.semantic_contract.value_contracts())
        .map(|v| v.contract.maximum_bytes)
        .max()
        .unwrap();
    let boundaries = [
        (conduit_core::PortDirection::Input, "ticks"),
        (conduit_core::PortDirection::Output, "states"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: conduit_core::ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 32768,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &expanded,
        &profiles,
        &conduit_plot::CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    assert!(
        conduit_std_host::flow_activation::value_scan_offer(&contract, &contract, &initial, 0)
            .is_err()
    );
    assert!(conduit_std_host::flow_activation::value_scan_offer(
        &contract, &contract, &initial, 65
    )
    .is_err());
    let different_item = conduit_core::CheckedValueContract::new(
        conduit_core::kind_id(conduit_core::BOOL_INFO_ID),
        1,
        vec![],
    )
    .unwrap();
    assert_ne!(
        conduit_std_host::flow_activation::value_scan_offer(
            &different_item,
            &contract,
            &initial,
            4
        )
        .unwrap()
        .capability_id,
        conduit_std_host::flow_activation::value_scan_offer(&contract, &contract, &initial, 4)
            .unwrap()
            .capability_id
    );
    let mut wrong_hosts = hosts.clone();
    let wrong_offer = wrong_hosts[0]
        .capabilities
        .iter_mut()
        .find(|offer| {
            offer.implementation.implementation_id.as_str()
                == conduit_std_host::flow_activation::VALUE_SCAN_IMPLEMENTATION
        })
        .unwrap();
    wrong_offer.capability_id = "fixture/uninstalled-value-scan".into();
    let wrong_placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &wrong_hosts).unwrap();
    let wrong_plan = conduit_planner::plan_expanded_authoring_with_activations(
        &checked,
        &expanded,
        &profiles,
        &conduit_plot::CanonicalBackCatalog::new(),
        &wrong_hosts,
        &wrong_placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&wrong_plan));
    let mut wrong_host = conduit_std_host::flow_activation::StdActivationHost::new(
        conduit_core::PreparationHostIdentity {
            host_id: wrong_hosts[0].host_id.clone(),
            boot_id: wrong_hosts[0].boot_id.clone(),
            offer_generation: wrong_hosts[0].offer_generation,
        },
        conduit_std_host::flow_activation::standard_child_registry().unwrap(),
    );
    let mut wrong_prepared =
        conduit_core::prepare_plan_on_hosts(&wrong_plan, &mut [&mut wrong_host]).unwrap();
    let conduit_core::PlannedActivationEntry::Scan(wrong_scan) = &wrong_plan.activations[0] else {
        panic!("scan")
    };
    let refusal = conduit_std_host::flow_activation::install_value_scan(
        &wrong_plan,
        &mut wrong_prepared,
        &wrong_scan.activation_id,
        &mut wrong_host,
    )
    .err()
    .unwrap();
    assert!(refusal.contains("owner differs"), "{refusal}");

    let conduit_core::PlannedActivationEntry::Scan(planned) = &plan.activations[0] else {
        panic!("scan")
    };
    assert_eq!(planned.selected_plan.fragments[0].placements.len(), 64);
    let mut registry = conduit_std_host::flow_activation::standard_child_registry().unwrap();
    registry
        .install(
            conduit_std_host::flow_activation::PreparedValuePairFactory::new(
                &contract, &stage, &contract, &stage,
            )
            .unwrap(),
        )
        .unwrap();
    let fragment = &plan.fragments[0];
    let mut host = conduit_std_host::flow_activation::StdActivationHost::new(
        conduit_core::PreparationHostIdentity {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
        },
        registry,
    );
    let mut prepared = conduit_core::prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let mut scan = conduit_std_host::flow_activation::install_value_scan(
        &plan,
        &mut prepared,
        &planned.activation_id,
        &mut host,
    )
    .unwrap();
    scan.bind_parent_play(&conduit_core::ActivePlayId::from(
        "fixture/acoustic-scan/play",
    ))
    .unwrap();
    let (_, tick) = seed::seeds(&checked, &profiles);
    let item = conduit_core::ValuePayload {
        value_kind: contract.value_kind.clone(),
        encoded: tick,
    };
    let mut output = conduit_core::ValuePayload {
        value_kind: contract.value_kind.clone(),
        encoded: Vec::with_capacity(32768),
    };
    for emitted in 1..=4 {
        scan.admit(&item).unwrap();
        let mut delivered = false;
        for _ in 0..1040 {
            scan.step().unwrap();
            assert!(scan.next_host_request().is_none());
            if scan.output_into(&mut output).unwrap() {
                let value =
                    conduit_core::StructuredInfoValue::from_canonical_bytes(&output.encoded)
                        .unwrap();
                let conduit_core::StructuredInfoValueShape::Variant { tag, payload } =
                    value.shape()
                else {
                    panic!("context")
                };
                assert_eq!(tag, "context");
                fn field<'a>(
                    value: &'a conduit_core::StructuredInfoValue,
                    name: &str,
                ) -> &'a conduit_core::StructuredInfoValue {
                    let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape()
                    else {
                        panic!("record")
                    };
                    fields
                        .iter()
                        .find(|field| field.name() == name)
                        .unwrap()
                        .value()
                }
                let state = field(payload, "cursor");
                for name in ["oscillator", "resonator"] {
                    let count = field(field(state, name), "emitted");
                    let conduit_core::StructuredInfoValueShape::Leaf(bytes) = count.shape() else {
                        panic!("count")
                    };
                    assert_eq!(u32::from_le_bytes(bytes.try_into().unwrap()), emitted);
                }
                scan.complete_output().unwrap();
                delivered = true;
                break;
            }
        }
        assert!(delivered);
    }
    scan.close_input().unwrap();
}
