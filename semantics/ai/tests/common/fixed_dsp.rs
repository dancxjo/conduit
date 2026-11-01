//! Shared exact Source/Plan fixtures; individual targets use different helpers.
#![allow(dead_code)]
use conduit_ai::{fixed_numeric_catalog::*, fixed_numeric_dsp_catalog::*};
use conduit_core::*;
use conduit_kernel::{scheduler::StepIo, ValueRef};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};
fn fixture(name: &str, ty: &StructuredInfoType, source: bool, flow: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: ty.profile().unwrap().value_kind().clone(),
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: if flow {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(name),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: if source { vec![] } else { vec![port.clone()] },
        outputs: if source { vec![port.clone()] } else { vec![] },
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: if source {
                FrontValueLocation::Output(port.port_id)
            } else {
                FrontValueLocation::Input(port.port_id)
            },
            contract: CheckedValueContract::new(
                port.value_kind,
                conduit_plot::maximum_prepared_canonical_value_bytes(ty).unwrap(),
                vec![],
            )
            .unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16384,
        },
    }
}
fn fixture_offer(kind: Kind) -> CapabilityOffer {
    let id = String::from(kind.kind_id.as_str());
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(id.clone()),
            execution_profile_id: ExecutionProfileId::from("dsp-fixture@1"),
            implementation_id: ImplementationId::from(id.clone()),
            artifact_id: ArtifactId::from(id),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
pub fn plan(operation: FixedDspOperation, flow: bool) -> Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    if flow {
        install_fixed_dsp_flow_catalogs(&mut startup, &mut profile).unwrap();
    }
    let (input, output) = operation.shape();
    let mut offers = vec![];
    for (name, width, source) in [
        ("dsp-fixture/left", input, true),
        ("dsp-fixture/right", input, true),
        ("dsp-fixture/sink", output, false),
    ] {
        let ty = fixed_numeric_type(&format!("NumericF32Vector{width}")).unwrap();
        let kind = fixture(name, &ty, source, flow);
        startup
            .insert(KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(kind.clone()).unwrap();
        offers.push(fixture_offer(kind));
    }
    offers.push(fixed_dsp_offer(operation, flow).unwrap());
    let identity = if flow {
        format!("numeric/flow-{}", operation.name())
    } else {
        format!("numeric/{}", operation.name())
    };
    let cords = if operation == FixedDspOperation::Dot160 {
        " right: dsp-fixture/right\n left.value >> operation.left\n right.value >> operation.right"
    } else {
        " left.value >> operation.value"
    };
    let plot=conduit_plot::parse_with_startup(&format!("plot dsp-proof {{\n left: dsp-fixture/left\n operation: {identity}\n sink: dsp-fixture/sink\n{cords}\n operation.result >> sink.value\n}}\n"),&startup,&profile).unwrap();
    let hosts = [HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("dsp-fixture"),
        boot_id: BootId::from("dsp-fixture-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("dsp-fixture@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        16384,
    )
    .unwrap()
}
pub fn conversion_plan(operation: IntegerConversion, flow: bool) -> Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    if flow {
        install_fixed_dsp_flow_catalogs(&mut startup, &mut profile).unwrap();
    }
    let input_name = match operation {
        IntegerConversion::U16 => String::from("U16"),
        IntegerConversion::I16Vector160 => String::from("NumericI16Vector160"),
        IntegerConversion::I16Vector80 => String::from("NumericI16Vector80"),
        IntegerConversion::FiniteVector(n) => format!("NumericRawF32Vector{n}"),
    };
    let output_name = format!("NumericF32Vector{}", operation.width());
    let mut offers = vec![];
    for (name, width, source) in [
        ("dsp-fixture/left", input_name.as_str(), true),
        ("dsp-fixture/sink", output_name.as_str(), false),
    ] {
        let ty = fixed_numeric_type(if width == "U16" {
            "NumericF32Vector1"
        } else {
            width
        })
        .unwrap();
        let mut kind = fixture(name, &ty, source, flow);
        if width == "U16" {
            kind.outputs[0].value_kind = kind_id("value/u16");
            if let KindSemanticLaw::ValueContracts(contracts) = &mut kind.semantic_laws[0] {
                contracts[0].contract =
                    CheckedValueContract::new(kind_id("value/u16"), 2, vec![]).unwrap();
            }
        }
        startup
            .insert(KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(kind.clone()).unwrap();
        offers.push(fixture_offer(kind));
    }
    offers.push(fixed_integer_conversion_offer(operation, flow).unwrap());
    let identity = if flow {
        format!("numeric/flow-{}", operation.name())
    } else {
        format!("numeric/{}", operation.name())
    };
    let cords = " left.value >> operation.value";
    let plot=conduit_plot::parse_with_startup(&format!("plot dsp-proof {{\n left: dsp-fixture/left\n operation: {identity}\n sink: dsp-fixture/sink\n{cords}\n operation.result >> sink.value\n}}\n"),&startup,&profile).unwrap();
    let hosts = [HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("dsp-fixture"),
        boot_id: BootId::from("dsp-fixture-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("dsp-fixture@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    conduit_planner::plan_with_connection_limits(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        16384,
    )
    .unwrap()
}
pub fn gear(plan: &Plan) -> &PlannedGear {
    plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str().starts_with("numeric/"))
        .unwrap()
}
pub fn frame(bytes: &[u8], ready: bool) -> StepIo<4> {
    StepIo::test_frame(
        [
            Some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: bytes.len() as u32,
            }),
            None,
            None,
            None,
        ],
        [false; 4],
        [if ready { Some(16384) } else { None }, None, None, None],
        None,
        16,
    )
}
