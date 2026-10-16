//! The same checked DSP child participates in the existing authored scan syntax.
#[path = "common_acoustic_value/literal.rs"]
mod literal;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

#[test]
fn closing_trajectory_flow_preserves_exact_queries_under_delivery_partitions_and_pressure() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let immediate = check_scan(0, 1);
            let pressured = check_scan(7, 4);
            assert_eq!(immediate, pressured);
        })
        .unwrap()
        .join()
        .unwrap();
}
fn check_scan(gap: usize, hold: usize) -> Vec<Vec<u8>> {
    let child_source = [
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../semantics/audio/trajectory.conduit"),
        include_str!("fixtures/acoustic-trajectory-value.conduit"),
        include_str!("fixtures/acoustic-trajectory-replay.conduit"),
    ]
    .join("\n");
    let mut imports = Vec::new();
    let mut body = Vec::new();
    for line in child_source.lines() {
        if line.starts_with("with ") {
            imports.push(line);
        } else {
            body.push(line);
        }
    }
    imports.extend(body);
    let child_source = imports.join("\n");
    let declarations = include_str!("fixtures/acoustic-trajectory-value.conduit")
        .lines()
        .take_while(|line| !line.starts_with("plot "))
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let owner = format!(
        "{}\n{}\n{}",
        include_str!("../../../semantics/audio/types.conduit"),
        include_str!("../../../semantics/audio/trajectory_types.conduit"),
        declarations
    );
    let types =
        check_syntax_document(&parse_syntax_document(&owner), &StartupCatalog::new()).unwrap();
    let stage = types
        .native_types
        .iter()
        .find(|ty| ty.name == "AcousticTrajectoryStage")
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
    check_syntax_document(&parse_syntax_document(&child_source), &startup).unwrap();
    let items = requests(&stage);
    let initial = items[0].clone();
    let initial_value = conduit_core::StructuredInfoValue::from_canonical_bytes(&initial).unwrap();
    let literal = literal::literal(&initial_value);
    let parent = format!(
        r#"
plot acoustic-scan-fixture (
 >> queries: AcousticTrajectoryStage...| <= 32768B
 observations: AcousticTrajectoryStage...| <= 32768B >>
) {{
 replay: acoustic-trajectory-replay(initial = {literal})
 queries >> replay.queries
 replay.observations >> observations
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
        expand_canonical_plot_for_authoring(&checked, "trajectory-combine", &profiles).unwrap();
    let mut capabilities = vec![conduit_std_host::flow_activation::value_scan_offer(
        &contract, &contract, &initial, 16,
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
        (conduit_core::PortDirection::Input, "queries"),
        (conduit_core::PortDirection::Output, "observations"),
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
    let outputs = parent_kernel::run(plan, &expanded.front, registry, &items, gap, hold);
    for (index, bytes) in outputs.iter().enumerate() {
        let value = conduit_core::StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
        let conduit_core::StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            panic!("stage")
        };
        assert_eq!(tag, "observation");
        let original = field(payload, "original");
        let input = conduit_core::StructuredInfoValue::from_canonical_bytes(&items[index]).unwrap();
        let conduit_core::StructuredInfoValueShape::Variant {
            payload: expected, ..
        } = input.shape()
        else {
            panic!("request")
        };
        assert_eq!(&original, expected);
        let result = field(payload, "result");
        let n = number(&result, "numerator");
        let d = number(&result, "denominator");
        let (rn, rd) = [
            (100, 3),
            ((3 * 100 * 7) + (200 * 3), 4 * 3 * 7),
            ((100 * 7) + (200 * 3), 2 * 3 * 7),
            ((100 * 7) + (3 * 200 * 3), 4 * 3 * 7),
            (200, 7),
        ][index];
        assert_eq!(u128::from(n) * rd, rn * u128::from(d));
        if index == 0 {
            assert_eq!((n, d), (100, 3));
        }
        if index == 4 {
            assert_eq!((n, d), (200, 7));
        }
    }
    outputs
}
fn field(
    value: &conduit_core::StructuredInfoValue,
    name: &str,
) -> conduit_core::StructuredInfoValue {
    let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .clone()
}
#[path = "common_acoustic_reference/parent_kernel.rs"]
mod parent_kernel;

use conduit_audio::*;
use conduit_core::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
fn number(value: &StructuredInfoValue, name: &str) -> u64 {
    let value = field(value, name);
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("integer")
    };
    u64::from_le_bytes(bytes.try_into().unwrap())
}
fn requests(stage: &StructuredInfoType) -> Vec<Vec<u8>> {
    let StructuredInfoTypeShape::Variant { cases, .. } = stage.shape() else {
        panic!("stage")
    };
    let request_type = cases
        .iter()
        .find(|case| case.tag() == "request")
        .unwrap()
        .payload_type()
        .clone();
    let mut items = Vec::new();
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let policy = AudioTrajectoryInterpolation::Linear;
    {
        let trajectory = AudioQuantityTrajectory::new(
            anchor.clone(),
            AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
            AudioTrajectoryOutside::Refuse,
            AudioTrajectoryProvenance::new(
                AudioTrajectoryProvenanceKind::Authored,
                "independent rational reference".into(),
                Some("v1".into()),
            )
            .unwrap(),
            BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
                AudioExactTimeOffset::new(1, 1).unwrap(),
                policy,
                AudioTrajectoryQuantity::frequency(3, 100).unwrap(),
                AudioTrajectoryQuantity::frequency(7, 200).unwrap(),
                AudioExactTimeOffset::new(1, 0).unwrap(),
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap();
        let admitted =
            PreparedAudioQuantityTrajectory::new(&trajectory.clone().encode().unwrap()).unwrap();
        for (qn, qd) in [(0, 1), (1, 4), (1, 2), (3, 4), (1, 1)] {
            let query = AudioTrajectoryQuery::new(
                anchor.clone(),
                AudioExactTimeOffset::new(qd, qn).unwrap(),
            )
            .unwrap()
            .encode()
            .unwrap();
            let admission = admitted.evaluate(&query).unwrap();
            assert_eq!(admission.original(), &trajectory);
            assert_eq!(admission.query_canonical(), query);
            let execution = admission.executions().last().unwrap();
            let carrier =
                StructuredInfoValue::from_canonical_bytes(execution.input_canonical()).unwrap();
            let input = if policy == AudioTrajectoryInterpolation::Linear {
                field(&field(&carrier, "request"), "input")
            } else {
                field(&carrier, "input")
            };
            let request = StructuredInfoValue::record(
                request_type.clone(),
                vec![StructuredFieldValue::new("input", input).unwrap()],
            )
            .unwrap();
            items.push(
                StructuredInfoValue::variant(stage.clone(), "request", request)
                    .unwrap()
                    .canonical_bytes()
                    .unwrap(),
            );
        }
    }
    items
}
