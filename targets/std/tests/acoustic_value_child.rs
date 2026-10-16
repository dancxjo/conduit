//! Development proof of the authored DSP child; this is not complete PCM output.
use conduit_core::{
    verify_plan, BaseImplementationId, CheckedValueContract, ConfigurationValue, ConnectionTrack,
    PortDirection, PortTemporal,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common_acoustic_value/kernel.rs"]
mod kernel;
#[path = "common_acoustic_value/seed.rs"]
mod seed;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn composed_source() -> String {
    let units = [
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/rate_types.conduit"),
        include_str!("../../../semantics/audio/decibel_types.conduit"),
        include_str!("../../../semantics/audio/exact_control_types.conduit"),
        include_str!("../../../semantics/audio/time_analysis_types.conduit"),
        include_str!("../../../semantics/audio/spectral_analysis_types.conduit"),
        include_str!("../../../semantics/audio/dsp/periodic-types.conduit"),
        conduit_audio::PERIODIC_FLOW_SOURCE,
        conduit_audio::PERIODIC_OSCILLATOR_SOURCE,
        conduit_audio::PERIODIC_VALIDATION_SOURCE,
        include_str!("../../../semantics/audio/dsp/resonator-recurrence.conduit"),
        include_str!("../../../semantics/audio/dsp/resonator-types.conduit"),
        include_str!("../../../semantics/audio/dsp/resonator-projection.conduit"),
        include_str!("../../../semantics/audio/dsp/resonator.conduit"),
        include_str!("fixtures/acoustic-value-frame.conduit"),
    ];
    let mut imports = Vec::new();
    let mut body = Vec::new();
    for unit in units {
        for line in unit.lines() {
            if line.starts_with("with ") {
                imports.push(line);
            } else {
                body.push(line);
            }
        }
    }
    imports.extend(body);
    imports.join("\n") + "\n"
}

#[test]
fn authored_oscillator_and_resonator_expand_as_one_bounded_value_child() {
    // Whole-source checking/preparation uses a main-thread-sized finite stack;
    // this does not change the child kernel's admitted Play storage.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(check_child)
        .unwrap()
        .join()
        .unwrap();
}

fn check_child() {
    let source = composed_source();
    let owner = source
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
    let contract =
        CheckedValueContract::new(stage.profile().unwrap().value_kind().clone(), 32768, vec![])
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
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let child =
        expand_canonical_plot_for_authoring(&checked, "acoustic-value-frame", &profiles).unwrap();
    assert!(!child.expanded.gears.is_empty());
    assert!(
        child.expanded.gears.len() <= 64,
        "{} child Gears",
        child.expanded.gears.len()
    );
    assert!(child.expanded.connections.len() <= 128);
    assert!(child
        .expanded
        .connections
        .iter()
        .all(|cord| cord.temporal == PortTemporal::Value));
    let mut pair_count = 0;
    let mut offers = Vec::new();
    for gear in &child.expanded.gears {
        if gear.kind_id.as_str() == conduit_semantic_catalog::VALUE_PAIR_KIND {
            pair_count += 1;
            if !offers.iter().any(|offer: &conduit_core::CapabilityOffer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::VALUE_PAIR_IMPLEMENTATION
            }) {
                offers.push(
                    conduit_std_offers::value_pair_offer(&contract, &stage, &contract, &stage)
                        .unwrap(),
                );
            }
        } else {
            let [entry] = gear.configuration.as_slice() else {
                panic!("one exact expression")
            };
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("canonical program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            let offer = conduit_std_offers::pure_expression_local_offer(&program).unwrap();
            assert!(gear.accepts_realization(&offer));
            assert!(offer.host_calls.is_empty());
            if !offers
                .iter()
                .any(|other| other.capability_id == offer.capability_id)
            {
                offers.push(offer);
            }
        }
    }
    assert_eq!(pair_count, 3);
    eprintln!(
        "acoustic Value child: {} Gears, {} Cords",
        child.expanded.gears.len(),
        child.expanded.connections.len()
    );
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    host.host_id = "fixture/acoustic-value-child".into();
    host.capabilities = offers;
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(&child.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "accumulator"),
        (PortDirection::Input, "item"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 32768,
            },
        )
    })
    .collect();
    let connection_capacity = child
        .expanded
        .gears
        .iter()
        .flat_map(|gear| gear.semantic_contract.value_contracts())
        .map(|contract| contract.contract.maximum_bytes)
        .max()
        .unwrap();
    assert!(connection_capacity as usize <= conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    eprintln!("acoustic child admitted Cord bytes: {connection_capacity}");
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &child,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: connection_capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments.len(), 1);
    let pair = conduit_std_host::flow_activation::PreparedValuePairFactory::new(
        &contract, &stage, &contract, &stage,
    )
    .unwrap();
    use conduit_composite::KernelOperationFactory;
    for gear in &plan.fragments[0].placements {
        let budget =
            if gear.implementation_id.as_str() == conduit_std_offers::VALUE_PAIR_IMPLEMENTATION {
                pair.budget(gear).unwrap()
            } else {
                conduit_std_host::flow_activation::LocalExpressionFactory
                    .budget(gear)
                    .unwrap()
            };
        assert_eq!(budget.host_requests, 0);
    }
    let (accumulator, item) = seed::seeds(&checked, &profiles);
    let cancelled_plan = plan.clone();
    let encoded = kernel::run(
        plan,
        &child.front,
        pair,
        accumulator.clone(),
        item.clone(),
        false,
    )
    .unwrap();
    let cancelled_pair = conduit_std_host::flow_activation::PreparedValuePairFactory::new(
        &contract, &stage, &contract, &stage,
    )
    .unwrap();
    assert!(kernel::run(
        cancelled_plan,
        &child.front,
        cancelled_pair,
        accumulator,
        item,
        true
    )
    .is_none());
    let output = conduit_core::StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
    let conduit_core::StructuredInfoValueShape::Variant { tag, payload } = output.shape() else {
        panic!("stage")
    };
    assert_eq!(tag, "context");
    let field = |value: &conduit_core::StructuredInfoValue,
                 name: &str|
     -> conduit_core::StructuredInfoValue {
        let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("record")
        };
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value()
            .clone()
    };
    let cursor = field(payload, "cursor");
    let sample = field(&cursor, "sample");
    let conduit_core::StructuredInfoValueShape::Leaf(bytes) = sample.shape() else {
        panic!("sample")
    };
    assert_eq!(i128::from_le_bytes(bytes.try_into().unwrap()), 24);
    let status = field(&cursor, "status");
    let conduit_core::StructuredInfoValueShape::Variant { tag, .. } = status.shape() else {
        panic!("status")
    };
    assert_eq!(tag, "sample");
    let unsigned = |value: &conduit_core::StructuredInfoValue| {
        let conduit_core::StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            panic!("u32")
        };
        u32::from_le_bytes(bytes.try_into().unwrap())
    };
    let oscillator = field(&cursor, "oscillator");
    let resonator = field(&cursor, "resonator");
    for state in [&oscillator, &resonator] {
        assert_eq!(unsigned(&field(state, "emitted")), 1);
        assert_eq!(unsigned(&field(state, "remaining")), 3);
    }
    let request = field(
        &field(&field(&oscillator, "profile"), "projection"),
        "request",
    );
    assert_eq!(
        field(&request, "basis"),
        field(&field(&resonator, "request"), "basis")
    );
    let y1 = field(&resonator, "y1");
    let conduit_core::StructuredInfoValueShape::Leaf(bytes) = y1.shape() else {
        panic!("history")
    };
    assert_eq!(i128::from_le_bytes(bytes.try_into().unwrap()), 24);
}
