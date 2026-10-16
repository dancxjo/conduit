//! Preparation of the actual timed Source child; no DSP advancement here.
use conduit_core::*;
use conduit_plot::*;
#[path = "../common_acoustic_value/source.rs"]
mod source;
pub struct Fixture {
    pub plan: Plan,
    pub front: CheckedFront,
    pub checked: CheckedSyntaxDocument,
    pub profiles: ProfileCatalog,
    pub stage: StructuredInfoType,
    pub contract: CheckedValueContract,
}
impl Fixture {
    pub fn pair(&self) -> conduit_std_host::flow_activation::PreparedValuePairFactory {
        conduit_std_host::flow_activation::PreparedValuePairFactory::new(
            &self.contract,
            &self.stage,
            &self.contract,
            &self.stage,
        )
        .unwrap()
    }
}
pub fn prepare() -> Fixture {
    let source =
        source::composed_source() + include_str!("../fixtures/acoustic-timed-frame.conduit");
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
        expand_canonical_plot_for_authoring(&checked, "acoustic-timed-frame", &profiles).unwrap();
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
    assert_eq!(pair_count, 4);
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
    Fixture {
        plan,
        front: child.front,
        checked,
        profiles,
        stage,
        contract,
    }
}
