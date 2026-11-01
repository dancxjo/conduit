//! Ordinary callable matching retains exact historical Front equality.
use conduit_core as c;
use conduit_plot::CheckedGear;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn fixture() -> (CheckedGear, c::CapabilityOffer) {
    let offer = c::BackOfferBuilder::new(
        c::Kind {
            startup_parameters: vec![],
            shorthand: None,
            kind_id: "test/front".into(),
            kind_contract_revision: "contract".into(),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
            semantic_laws: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        },
        c::Back {
            capability_id: "back".into(),
            execution_profile_id: "profile".into(),
            implementation_id: "implementation".into(),
            artifact_id: "artifact".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let gear = conduit_plot::checked_gear_from_parts! {
        gear_id: "gear".into(),
        kind_id: offer.kind_id.clone(),
        kind_contract_revision: offer.kind_contract_revision.clone(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![],
        outputs: vec![],
        semantic_contract: c::KindSemanticContract::default(),
        terminal_transductions: vec![],
        resource_ports: vec![],
        configuration: vec![],
        pool_references: vec![],
    };
    (gear, offer)
}
fn ports(variant: u16, direction: c::PortDirection) -> Vec<c::PortDescriptor> {
    let mut ports = vec![c::PortDescriptor {
        port_id: "z".into(),
        value_kind: if variant & 1 == 0 {
            "value/text"
        } else {
            "value/bytes"
        }
        .into(),
        direction,
        temporal: match variant & 6 {
            0 => c::PortTemporal::Value,
            2 => c::PortTemporal::Current,
            4 => c::PortTemporal::Flow { closes: false },
            _ => c::PortTemporal::Flow { closes: true },
        },
        abnormal_kind: (variant & 8 != 0).then(|| "value/text".into()),
    }];
    if variant & 16 != 0 {
        let mut other = ports[0].clone();
        other.port_id = if variant & 32 == 0 { "a" } else { "z" }.into();
        other.value_kind = "value/bytes".into();
        ports.push(other);
    }
    if variant & 64 != 0 {
        ports.reverse();
    }
    ports
}
#[test]
fn full_port_fields_stable_duplicate_order_and_shorthand_match_historical_fronts() {
    for source in 0..128 {
        for mutation in 0..12 {
            let (mut gear, mut offer) = fixture();
            gear.inputs = ports(source, c::PortDirection::Input);
            gear.outputs = ports(source ^ 17, c::PortDirection::Output);
            offer.inputs = gear.inputs.clone();
            offer.outputs = gear.outputs.clone();
            gear.shorthand = Some((
                gear.inputs[0].port_id.clone(),
                gear.outputs[0].port_id.clone(),
            ));
            offer.shorthand = gear.shorthand.clone();
            match mutation {
                0 => {}
                1 => offer.inputs.reverse(),
                2 => offer.outputs.reverse(),
                3 => offer.inputs[0].value_kind = "different".into(),
                4 => offer.inputs[0].abnormal_kind = Some("different".into()),
                5 => offer.inputs[0].direction = c::PortDirection::Output,
                6 => offer.outputs[0].temporal = c::PortTemporal::Value,
                7 => offer.shorthand = None,
                8 => gear.shorthand = None,
                9 => offer.shorthand = Some(("missing".into(), "missing".into())),
                10 => {
                    offer.inputs.clear();
                }
                _ => offer.outputs.push(offer.outputs[0].clone()),
            }
            let historical = gear.checked_front() == offer.checked_front();
            let (actual, cost) = allocation_probe::observe(|| gear.accepts_callable_front(&offer));
            assert_eq!(actual, historical, "source {source}, mutation {mutation}");
            assert_eq!((cost.allocations, cost.reallocations), (0, 0));
        }
    }
}
#[test]
fn startup_and_explicit_contracts_retain_complete_historical_comparison() {
    let (mut gear, mut offer) = fixture();
    gear.inputs = ports(0, c::PortDirection::Input);
    offer.inputs = gear.inputs.clone();
    for has_default in [false, true] {
        gear.startup_parameters = vec![c::FrontStartupParameter {
            name: "text".into(),
            value_type: "value/text".into(),
            has_default,
        }];
        offer.startup_parameters = gear.startup_parameters.clone();
        assert_eq!(
            gear.accepts_callable_front(&offer),
            gear.checked_front() == offer.checked_front()
        );
        offer.startup_parameters[0].has_default = !has_default;
        assert_eq!(
            gear.accepts_callable_front(&offer),
            gear.checked_front() == offer.checked_front()
        );
    }
    gear.startup_parameters.clear();
    offer.startup_parameters.clear();
    for bound in [1, 256, 65536] {
        let contract = c::FrontValueContract {
            location: c::FrontValueLocation::Input("z".into()),
            contract: c::CheckedValueContract::new("value/text".into(), bound, vec![]).unwrap(),
        };
        gear.semantic_contract.laws =
            vec![c::KindSemanticLaw::ValueContracts(vec![contract.clone()])];
        offer.semantic_contract.laws = vec![c::KindSemanticLaw::ValueContracts(vec![contract])];
        assert_eq!(
            gear.accepts_callable_front(&offer),
            gear.checked_front() == offer.checked_front()
        );
        offer.semantic_contract.laws.clear();
        assert_eq!(
            gear.accepts_callable_front(&offer),
            gear.checked_front() == offer.checked_front()
        );
    }
}
