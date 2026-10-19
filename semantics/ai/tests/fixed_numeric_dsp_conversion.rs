#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_dsp_catalog::*,
};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
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
fn plan(operation: FixedDspOperation, flow: bool) -> Plan {
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
fn conversion_plan(operation: IntegerConversion, flow: bool) -> Plan {
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
fn gear(plan: &Plan) -> &PlannedGear {
    plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str().starts_with("numeric/"))
        .unwrap()
}
fn frame(bytes: &[u8], ready: bool) -> StepIo<4> {
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
#[test]
fn selected_integer_converters_preserve_signed_scale_and_stage_only_on_commit() {
    use conduit_ai::{
        fixed_numeric_i16_codec::FixedI16VectorCodec,
        fixed_numeric_integer_conversion::FixedIntegerConversionBack,
    };
    let plan = conversion_plan(IntegerConversion::U16, true);
    let mut back = FixedIntegerConversionBack::<1>::prepare_planned::<4>(
        gear(&plan),
        3,
        IntegerConversion::U16,
        true,
    )
    .unwrap();
    let decoder =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap();
    for value in [0u16, 32, 255, u16::MAX] {
        let bytes = value.to_le_bytes();
        let inputs = StepInputBytes::test_frame([Some(&bytes), None, None, None], None);
        let mut blocked = frame(&bytes, false);
        assert_eq!(back.step(&mut blocked, &inputs), StepOutcome::Await);
        assert!(!blocked.test_consumed(PortId(0)));
        let before = back.committed_frames();
        let mut io = frame(&bytes, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        assert_eq!(back.committed_frames(), before);
        let mut out = [0.];
        decoder
            .decode(
                <FixedIntegerConversionBack<1> as StepBack<4>>::prepared_output(&back, PortId(0))
                    .unwrap(),
                &mut out,
            )
            .unwrap();
        assert_eq!(out, [f32::from(value)]);
        <FixedIntegerConversionBack<1> as StepBack<4>>::step_committed(&mut back);
        assert_eq!(back.committed_frames(), before + 1);
    }
    let plan = conversion_plan(IntegerConversion::I16Vector160, false);
    let mut back = FixedIntegerConversionBack::<160>::prepare_planned::<4>(
        gear(&plan),
        3,
        IntegerConversion::I16Vector160,
        false,
    )
    .unwrap();
    let mut codec =
        FixedI16VectorCodec::<160>::prepare(&fixed_numeric_type("NumericI16Vector160").unwrap())
            .unwrap();
    let signed = core::array::from_fn(|i| [i16::MIN, -1, 0, i16::MAX][i % 4]);
    let bytes = codec.encode(&signed);
    let inputs = StepInputBytes::test_frame([Some(bytes), None, None, None], None);
    let mut io = frame(bytes, true);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
    let mut out = [0.; 160];
    FixedF32VectorCodec::<160>::prepare(&fixed_numeric_type("NumericF32Vector160").unwrap())
        .unwrap()
        .decode(
            <FixedIntegerConversionBack<160> as StepBack<4>>::prepared_output(&back, PortId(0))
                .unwrap(),
            &mut out,
        )
        .unwrap();
    assert_eq!(out, signed.map(f32::from));
    <FixedIntegerConversionBack<160> as StepBack<4>>::step_committed(&mut back);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Complete);
}

#[test]
fn finite_admission_checks_exact_raw_envelope_and_nonfinite_before_consumption() {
    use conduit_ai::fixed_numeric_integer_conversion::FixedIntegerConversionBack;
    let plan = conversion_plan(IntegerConversion::FiniteVector(1), false);
    let mut back = FixedIntegerConversionBack::<1>::prepare_planned::<4>(
        gear(&plan),
        3,
        IntegerConversion::FiniteVector(1),
        false,
    )
    .unwrap();
    let raw_type = fixed_numeric_type("NumericRawF32Vector1").unwrap();
    assert!(FixedF32VectorCodec::<1>::prepare(&raw_type).is_err());
    let mut raw = FixedF32VectorCodec::<1>::prepare_raw(&raw_type).unwrap();
    let bytes = raw.encode(&[f32::from_bits(0x3f010203)]).unwrap().to_vec();
    let mut invalid = bytes.clone();
    let offset = invalid.windows(4).position(|w| w == [3, 2, 1, 63]).unwrap();
    invalid[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    let mut io = frame(&invalid, true);
    let inputs = StepInputBytes::test_frame([Some(&invalid), None, None, None], None);
    assert!(matches!(back.step(&mut io, &inputs), StepOutcome::Fail(_)));
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!(back.committed_frames(), 0);
    assert!(
        <FixedIntegerConversionBack<1> as StepBack<4>>::prepared_output(&back, PortId(0)).is_none()
    );
    let mut io = frame(&bytes, true);
    let inputs = StepInputBytes::test_frame([Some(&bytes), None, None, None], None);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
    let finite =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap();
    let mut out = [0.];
    finite
        .decode(
            <FixedIntegerConversionBack<1> as StepBack<4>>::prepared_output(&back, PortId(0))
                .unwrap(),
            &mut out,
        )
        .unwrap();
    assert_eq!(out, [f32::from_bits(0x3f010203)]);
}

#[test]
fn hosted_factory_selects_explicit_flow_dsp_and_finite_admission_offers() {
    use conduit_composite::KernelOperationFactory;
    for plan in [
        plan(FixedDspOperation::Sqrt1, true),
        conversion_plan(IntegerConversion::FiniteVector(20), true),
        conversion_plan(IntegerConversion::FiniteVector(256), false),
        conversion_plan(IntegerConversion::I16Vector80, true),
    ] {
        let factory =
            conduit_ai::operation_owners::fixed_numeric::FixedNumericOperationFactory::for_plan(
                &plan,
                &std::collections::BTreeMap::new(),
            )
            .unwrap()
            .into_iter()
            .find(|factory| factory.implementation_id().as_str() == DSP_IMPLEMENTATION)
            .unwrap();
        let mut store = conduit_kernel::HostedValueStore::new(4, 16384, 65536).unwrap();
        let _back = factory.prepare(gear(&plan), &mut store).unwrap();
    }
}

#[test]
fn finite256_refuses_malformed_and_nonfinite_without_partial_output() {
    use conduit_ai::fixed_numeric_integer_conversion::FixedIntegerConversionBack;
    let operation = IntegerConversion::FiniteVector(256);
    let plan = conversion_plan(operation, false);
    let mut back =
        FixedIntegerConversionBack::<256>::prepare_planned::<4>(gear(&plan), 3, operation, false)
            .unwrap();
    let ty = fixed_numeric_type("NumericRawF32Vector256").unwrap();
    let mut raw = FixedF32VectorCodec::<256>::prepare_raw(&ty).unwrap();
    let values = core::array::from_fn(|i| {
        if i == 255 {
            f32::from_bits(0x3f010203)
        } else {
            i as f32
        }
    });
    let bytes = raw.encode(&values).unwrap().to_vec();
    let mut nonfinite = bytes.clone();
    let offset = nonfinite
        .windows(4)
        .rposition(|w| w == [3, 2, 1, 63])
        .unwrap();
    nonfinite[offset..offset + 4].copy_from_slice(&f32::INFINITY.to_le_bytes());
    let mut malformed = bytes.clone();
    malformed[0] ^= 255;
    for invalid in [&nonfinite[..], &malformed[..], &bytes[..bytes.len() - 1]] {
        let mut decoded = [9.; 256];
        assert!(raw.decode(invalid, &mut decoded).is_err());
        assert_eq!(decoded, [9.; 256]);
        let mut io = frame(invalid, true);
        let inputs = StepInputBytes::test_frame([Some(invalid), None, None, None], None);
        assert!(matches!(back.step(&mut io, &inputs), StepOutcome::Fail(_)));
        assert!(!io.test_consumed(PortId(0)));
        assert_eq!(back.committed_frames(), 0);
        assert!(
            <FixedIntegerConversionBack<256> as StepBack<4>>::prepared_output(&back, PortId(0))
                .is_none()
        );
    }
    let mut io = frame(&bytes, false);
    let inputs = StepInputBytes::test_frame([Some(&bytes), None, None, None], None);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
    assert!(!io.test_consumed(PortId(0)));
    let mut io = frame(&bytes, true);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
    let mut output = [0.; 256];
    FixedF32VectorCodec::<256>::prepare(&fixed_numeric_type("NumericF32Vector256").unwrap())
        .unwrap()
        .decode(
            <FixedIntegerConversionBack<256> as StepBack<4>>::prepared_output(&back, PortId(0))
                .unwrap(),
            &mut output,
        )
        .unwrap();
    assert_eq!(output, values);
    assert_eq!(back.committed_frames(), 0);
    <FixedIntegerConversionBack<256> as StepBack<4>>::step_committed(&mut back);
    assert_eq!(back.committed_frames(), 1);
}

#[path = "fixed_numeric_dsp_conversion/flow_closure.rs"]
mod flow_closure;
