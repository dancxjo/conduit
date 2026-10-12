use super::*;

#[test]
fn external_seed_uses_the_exact_bounded_singleton_contract() {
    let prepared = source();
    let expanded = prepared.expand("cdc-ecm-frame-service").unwrap();
    let binding = expanded
        .input_bindings
        .iter()
        .find(|binding| binding.front_port_id.as_str() == "start")
        .unwrap();
    let gear = expanded
        .expanded
        .gears
        .iter()
        .find(|gear| gear.gear_id == binding.gear_id)
        .unwrap();
    assert_eq!(gear.kind_id.as_str(), "flow/singleton");
    let external = expanded
        .front
        .value_contract(&FrontValueLocation::Input(binding.front_port_id.clone()))
        .unwrap();
    let native = gear.checked_front();
    assert_eq!(
        Some(external),
        native.value_contract(&FrontValueLocation::Input(binding.gear_port_id.clone()))
    );
    assert_eq!(external.maximum_bytes, 4096);
    assert_eq!(native.inputs()[0].temporal, PortTemporal::Value);
}

#[test]
fn every_external_contract_matches_its_checked_native_back() {
    let source = source();
    let expanded = source.expand("cdc-ecm-frame-service").unwrap();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/ecm".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    source.publish_pure_backs(&expanded, &mut host).unwrap();
    for (bindings, input) in [
        (&expanded.input_bindings, true),
        (&expanded.output_bindings, false),
    ] {
        for binding in bindings {
            let gear = expanded
                .expanded
                .gears
                .iter()
                .find(|gear| gear.gear_id == binding.gear_id)
                .unwrap();
            let offer = host
                .capabilities
                .iter()
                .find(|offer| offer.kind_id == gear.kind_id)
                .unwrap();
            let location = |port: PortId| {
                if input {
                    FrontValueLocation::Input(port)
                } else {
                    FrontValueLocation::Output(port)
                }
            };
            let native = offer.checked_front();
            assert_eq!(
                expanded
                    .front
                    .value_contract(&location(binding.front_port_id.clone())),
                native.value_contract(&location(binding.gear_port_id.clone())),
                "external {}",
                binding.front_port_id.as_str()
            );
        }
    }
}

#[test]
fn all_terminal_outcomes_fit_the_checked_native_envelope() {
    let source = source();
    let program = program(&source, "cdc-ecm-outcome");
    let StructuredInfoTypeShape::Variant { cases, .. } = program.output_type.shape() else {
        panic!("outcome variant")
    };
    let mut maximum = 0;
    for case in cases {
        let value = unit(&program.output_type, case.tag());
        let bytes = value.canonical_bytes().unwrap();
        maximum = maximum.max(bytes.len());
        assert!(
            bytes.len() <= 512,
            "{} needs {} bytes",
            case.tag(),
            bytes.len()
        );
    }
    eprintln!("ECM terminal canonical maximum={maximum}");
}

#[test]
fn every_pure_gear_has_an_exact_retained_native_realization() {
    let source = source();
    let expanded = source.expand("cdc-ecm-frame-service").unwrap();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/ecm".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/native@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    source.publish_pure_backs(&expanded, &mut host).unwrap();
    for gear in &expanded.expanded.gears {
        if gear.kind_id.as_str().starts_with("machine/usb/")
            || gear.kind_id.as_str() == "network/ethernet-frame"
        {
            continue;
        }
        assert!(
            host.capabilities
                .iter()
                .any(|offer| gear.accepts_realization(offer)),
            "missing exact native Back for {}",
            gear.kind_id.as_str()
        );
    }
}
