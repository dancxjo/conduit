//! Source-derived pair/expression planning for the reference transport proof.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::flow_activation::PreparedValuePairFactory;
use std::collections::BTreeMap;

pub fn prepare(
    fixture: &str,
    stage_name: &str,
    entry: &str,
) -> (Plan, CheckedFront, PreparedValuePairFactory) {
    prepare_with_catalogues(
        fixture,
        stage_name,
        entry,
        StartupCatalog::new(),
        ProfileCatalog::new(),
    )
}

pub fn prepare_with_catalogues(
    fixture: &str,
    stage_name: &str,
    entry: &str,
    mut startup: StartupCatalog,
    mut profiles: ProfileCatalog,
) -> (Plan, CheckedFront, PreparedValuePairFactory) {
    let types = [
        include_str!("../../../../semantics/audio/types.conduit"),
        include_str!("../../../../semantics/audio/trajectory_types.conduit"),
        include_str!("../../../../semantics/audio/decibel_types.conduit"),
    ]
    .join("\n");
    let declarations = fixture
        .lines()
        .take_while(|line| !line.starts_with("plot "))
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let owner = check_syntax_document(
        &parse_syntax_document(&format!("{types}\n{declarations}")),
        &startup,
    )
    .unwrap();
    let stage = owner
        .native_types
        .iter()
        .find(|ty| ty.name == stage_name)
        .unwrap()
        .value_type
        .clone();
    let contract =
        CheckedValueContract::new(stage.profile().unwrap().value_kind().clone(), 32768, vec![])
            .unwrap();
    conduit_semantic_catalog::install_value_pair_kind(
        &contract,
        &stage,
        &contract,
        &stage,
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let units = [
        types.as_str(),
        include_str!("../../../../semantics/audio/acoustic_quantities.conduit"),
        include_str!("../../../../semantics/audio/trajectory.conduit"),
        include_str!("../../../../semantics/audio/decibel_projection.conduit"),
        fixture,
    ];
    let mut imports = vec![];
    let mut body = vec![];
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
    let source = imports.join("\n") + "\n";
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, entry, &profiles).unwrap();
    let pair = PreparedValuePairFactory::new(&contract, &stage, &contract, &stage).unwrap();
    let mut offers = vec![pair.offer().clone()];
    for gear in &authored.expanded.gears {
        if gear.kind_id.as_str() == conduit_semantic_catalog::VALUE_PAIR_KIND {
            continue;
        }
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("Source program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer = conduit_std_offers::pure_expression_local_offer(&program).unwrap();
        assert!(gear.accepts_realization(&offer));
        assert!(offer.host_calls.is_empty());
        if !offers
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            offers.push(offer);
        }
    }
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    host.host_id = "fixture/acoustic-reference-child".into();
    host.capabilities = offers;
    let hosts = [host];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
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
    let capacity = authored
        .expanded
        .gears
        .iter()
        .flat_map(|gear| gear.semantic_contract.value_contracts())
        .map(|contract| contract.contract.maximum_bytes)
        .max()
        .unwrap();
    assert!(capacity as usize <= MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(
        plan.source_document_id,
        authored.expanded.source_document_id
    );
    assert_eq!(plan.checked_plot_id, authored.expanded.checked_plot_id);
    assert_eq!(plan.expanded_plot_id, authored.expanded.expanded_plot_id);
    (plan, authored.front, pair)
}

pub fn field(value: &StructuredInfoValue, name: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .clone()
}
