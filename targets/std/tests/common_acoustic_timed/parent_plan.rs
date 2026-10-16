//! Preparation-only fixture for the authored timed parent and production factories.
use conduit_composite::KernelOperationRegistry;
use conduit_core::{CheckedFront, Plan};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
#[path = "../common_acoustic_value/literal.rs"]
mod literal;
#[path = "../common_acoustic_value/seed.rs"]
mod seed;
#[path = "../common_acoustic_value/source.rs"]
mod source;
#[path = "values.rs"]
mod values;
pub struct Parent {
    pub plan: Plan,
    pub front: CheckedFront,
    pub registry: KernelOperationRegistry,
    pub items: Vec<Vec<u8>>,
}
pub fn prepare() -> Parent {
    let child_source =
        source::composed_source() + include_str!("../fixtures/acoustic-timed-frame.conduit");
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
    let parent = format!("{}\nplot acoustic-scan-fixture (\n >> controls: AcousticStage...| <= 32768B\n states: AcousticStage...| <= 32768B >>\n) {{\n render: acoustic-timed-flow(initial = {literal})\n controls >> render.controls\n render.states >> states\n}}\n", include_str!("../fixtures/acoustic-timed-flow.conduit"));
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
        expand_canonical_plot_for_authoring(&checked, "acoustic-timed-combine", &profiles).unwrap();
    assert!(combine.expanded.gears.len() <= 64);
    let mut capabilities = vec![conduit_std_host::flow_activation::value_scan_offer(
        &contract, &contract, &initial, 7,
    )
    .unwrap()];
    for gear in combine
        .expanded
        .gears
        .iter()
        .chain(expanded.expanded.gears.iter().filter(|gear| {
            gear.configuration
                .iter()
                .any(|entry| entry.key == "program")
        }))
    {
        let offer = if gear.kind_id.as_str() == conduit_semantic_catalog::VALUE_PAIR_KIND {
            conduit_std_offers::value_pair_offer(&contract, &stage, &contract, &stage).unwrap()
        } else {
            let conduit_core::ConfigurationValue::Text(encoded) = &gear.configuration[0].value
            else {
                panic!("program")
            };
            let program =
                conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            if gear.inputs[0].temporal == conduit_core::PortTemporal::Value {
                conduit_std_offers::pure_expression_local_offer(&program).unwrap()
            } else {
                conduit_std_offers::pure_expression_local_flow_offer(&program).unwrap()
            }
        };
        if !capabilities
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            capabilities.push(offer);
        }
    }
    let mut advertisement = conduit_std_host::StdHost::new().advertisement().clone();
    advertisement.host_id = "fixture/acoustic-timed-scan".into();
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
        (conduit_core::PortDirection::Input, "controls"),
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
    assert_eq!(
        plan.source_document_id,
        expanded.expanded.source_document_id
    );
    assert_eq!(plan.checked_plot_id, expanded.expanded.checked_plot_id);
    assert_eq!(plan.expanded_plot_id, expanded.expanded.expanded_plot_id);
    println!(
        "acoustic-source={} checked={} expanded={} plan={}",
        plan.source_document_id.as_str(),
        plan.checked_plot_id.as_str(),
        plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str()
    );
    let conduit_core::PlannedActivationEntry::Scan(planned) = &plan.activations[0] else {
        panic!("scan")
    };
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
    let scan_factory = conduit_std_host::flow_activation::PreparedValueScanFactory::prepare(
        &plan,
        &mut prepared,
        &planned.activation_id,
        &mut host,
    )
    .unwrap();
    let mut registry = conduit_composite::KernelOperationRegistry::new();
    registry.install(scan_factory).unwrap();
    registry
        .install(conduit_std_host::flow_activation::LocalExpressionFactory)
        .unwrap();
    let items = [(1, 0), (2, 1), (1, 0), (1, 3), (1, 1), (1, 2), (1, 3)]
        .into_iter()
        .map(|(origin, frame)| values::tick(&stage, origin, frame))
        .collect::<Vec<_>>();
    Parent {
        plan,
        front: expanded.front,
        registry,
        items,
    }
}
