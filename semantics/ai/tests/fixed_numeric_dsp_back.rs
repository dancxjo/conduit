#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_dsp_back::FixedDspBack, fixed_numeric_dsp_catalog::*,
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
                conduit_plot::maximum_prepared_transport_value_bytes(ty).unwrap(),
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
    let input_name = if operation == IntegerConversion::U16 {
        "U16"
    } else {
        "NumericI16Vector160"
    };
    let output_name = format!("NumericF32Vector{}", operation.width());
    let mut offers = vec![];
    for (name, width, source) in [
        ("dsp-fixture/left", input_name, true),
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
fn selected_closing_flow_sqrt_stages_three_frames_and_refuses_domain_without_progress() {
    let plan = plan(FixedDspOperation::Sqrt1, true);
    let mut back =
        FixedDspBack::<1, 1>::prepare_planned::<4>(gear(&plan), 3, FixedDspOperation::Sqrt1, true)
            .unwrap();
    let ty = fixed_numeric_type("NumericF32Vector1").unwrap();
    let mut codec = FixedF32VectorCodec::<1>::prepare(&ty).unwrap();
    let decoder = FixedF32VectorCodec::<1>::prepare(&ty).unwrap();
    for (i, value) in [4., 9., 16.].into_iter().enumerate() {
        let bytes = codec.encode(&[value]).unwrap();
        let inputs = StepInputBytes::test_frame([Some(bytes), None, None, None], None);
        let mut blocked = frame(bytes, false);
        assert_eq!(back.step(&mut blocked, &inputs), StepOutcome::Await);
        assert!(!blocked.test_consumed(PortId(0)));
        assert_eq!(back.committed_frames(), i as u64);
        let mut io = frame(bytes, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        assert_eq!(back.committed_frames(), i as u64);
        let mut output = [0.];
        decoder
            .decode(
                <FixedDspBack<1, 1> as StepBack<4>>::prepared_output(&back, PortId(0)).unwrap(),
                &mut output,
            )
            .unwrap();
        assert_eq!(output, [i as f32 + 2.]);
        <FixedDspBack<1, 1> as StepBack<4>>::step_committed(&mut back);
        assert_eq!(back.committed_frames(), i as u64 + 1);
    }
    let bytes = codec.encode(&[-1.]).unwrap();
    let mut io = frame(bytes, true);
    assert!(matches!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(bytes), None, None, None], None)
        ),
        StepOutcome::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::InvalidInput,
            detail: 2813
        })
    ));
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!(back.committed_frames(), 3);
    <FixedDspBack<1, 1> as StepBack<4>>::cancel(&mut back);
    let mut io = frame(bytes, true);
    assert!(matches!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(bytes), None, None, None], None)
        ),
        StepOutcome::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::Cancelled,
            ..
        })
    ));
    assert_eq!(back.committed_frames(), 3);
    let mut foreign = gear(&plan).clone();
    foreign.implementation_id = ImplementationId::from("foreign/dsp");
    assert!(FixedDspBack::<1, 1>::prepare_planned::<4>(
        &foreign,
        3,
        FixedDspOperation::Sqrt1,
        true
    )
    .is_err());
    assert!(FixedDspBack::<1, 1>::prepare_planned::<4>(
        gear(&plan),
        3,
        FixedDspOperation::Sqrt1,
        false
    )
    .is_err());
}
#[test]
fn ordinary_std_factory_prepares_exact_dft_and_dot_value_operations() {
    use conduit_composite::KernelOperationFactory;
    for operation in [FixedDspOperation::RealDft320, FixedDspOperation::Dot160] {
        let plan = plan(operation, false);
        let factory = conduit_std_host::fixed_numeric::FixedNumericOperationFactory::for_plan(
            &plan,
            &std::collections::BTreeMap::new(),
        )
        .unwrap()
        .into_iter()
        .find(|factory| factory.implementation_id().as_str() == DSP_IMPLEMENTATION)
        .unwrap();
        let mut store = conduit_kernel::HostedValueStore::new(4, 16384, 65536).unwrap();
        let mut back = factory.prepare(gear(&plan), &mut store).unwrap();
        let mut input = if operation == FixedDspOperation::RealDft320 {
            let mut codec = FixedF32VectorCodec::<320>::prepare(
                &fixed_numeric_type("NumericF32Vector320").unwrap(),
            )
            .unwrap();
            let mut impulse = [0.; 320];
            impulse[0] = 1.;
            codec.encode(&impulse).unwrap().to_vec()
        } else {
            let mut codec = FixedF32VectorCodec::<160>::prepare(
                &fixed_numeric_type("NumericF32Vector160").unwrap(),
            )
            .unwrap();
            codec.encode(&[0.5; 160]).unwrap().to_vec()
        };
        let right = if operation == FixedDspOperation::Dot160 {
            let mut codec = FixedF32VectorCodec::<160>::prepare(
                &fixed_numeric_type("NumericF32Vector160").unwrap(),
            )
            .unwrap();
            Some(codec.encode(&[0.25; 160]).unwrap().to_vec())
        } else {
            None
        };
        let mut refs = [None; 16];
        refs[0] = Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: input.len() as u32,
        });
        if let Some(right) = &right {
            refs[1] = Some(ValueRef {
                slot: 1,
                generation: 1,
                byte_len: right.len() as u32,
            });
        }
        let mut io = StepIo::test_frame(
            refs,
            [false; 16],
            core::array::from_fn(|i| if i == 0 { Some(16384) } else { None }),
            None,
            16,
        );
        assert_eq!(
            back.step(
                &mut io,
                &StepInputBytes::test_frame(
                    core::array::from_fn(|i| match i {
                        0 => Some(input.as_slice()),
                        1 => right.as_deref(),
                        _ => None,
                    }),
                    None
                )
            ),
            StepOutcome::Progress
        );
        let bytes = back.prepared_output(PortId(0)).unwrap();
        if operation == FixedDspOperation::RealDft320 {
            let codec = FixedF32VectorCodec::<322>::prepare(
                &fixed_numeric_type("NumericF32Vector322").unwrap(),
            )
            .unwrap();
            let mut output = [0.; 322];
            codec.decode(bytes, &mut output).unwrap();
            for bin in output.as_chunks::<2>().0 {
                assert_eq!(bin, &[1., 0.]);
            }
        } else {
            let codec = FixedF32VectorCodec::<1>::prepare(
                &fixed_numeric_type("NumericF32Vector1").unwrap(),
            )
            .unwrap();
            let mut output = [0.];
            codec.decode(bytes, &mut output).unwrap();
            assert_eq!(output, [20.]);
        }
        back.step_committed();
        input.clear();
        let mut io = StepIo::test_frame(
            [None; 16],
            [false; 16],
            core::array::from_fn(|i| if i == 0 { Some(16384) } else { None }),
            None,
            16,
        );
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 16], None)),
            StepOutcome::Complete
        );
    }
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
