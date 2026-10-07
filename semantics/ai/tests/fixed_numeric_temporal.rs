#![cfg(feature = "kernel-step")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_codec::*, fixed_numeric_index_back::*,
    fixed_numeric_operations_back::*, fixed_numeric_signal_back::*, fixed_numeric_temporal::*,
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

fn selected_plan(identity: &str, implementation: &str) -> Plan {
    let kind = closing_numeric_contract(identity, implementation).unwrap();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profile).unwrap();
    install_closing_numeric_catalogs(&mut startup, &mut profile).unwrap();
    let mut offers = vec![closing_numeric_offer(identity, implementation).unwrap()];
    let mut source = format!(
        "plot temporal-proof {{\n operation: {}\n",
        kind.kind_id.as_str()
    );
    for (i, port) in kind.inputs.iter().chain(&kind.outputs).enumerate() {
        let output = port.direction == PortDirection::Output;
        let ty = fixed_numeric_types()
            .unwrap()
            .into_iter()
            .find(|item| item.value_type.profile().unwrap().value_kind() == &port.value_kind)
            .map(|item| item.value_type)
            .unwrap_or_else(|| StructuredInfoType::leaf(port.value_kind.clone()).unwrap());
        let name = format!("temporal-fixture/port{i}");
        let mut fixture = fixture(&name, &ty, !output, true);
        if port.value_kind.as_str() == "value/u16" {
            if let KindSemanticLaw::ValueContracts(contracts) = &mut fixture.semantic_laws[0] {
                contracts[0].contract =
                    CheckedValueContract::new(port.value_kind.clone(), 2, vec![]).unwrap();
            }
        }
        startup
            .insert(KindSignature {
                kind: name.clone(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert_kind(fixture.clone()).unwrap();
        offers.push(fixture_offer(fixture));
        source += &format!(" port{i}: {name}\n");
        source += &if output {
            format!(" operation.{} >> port{i}.value\n", port.port_id.as_str())
        } else {
            format!(" port{i}.value >> operation.{}\n", port.port_id.as_str())
        };
    }
    source += "}\n";
    let plot = conduit_plot::parse_with_startup(&source, &startup, &profile).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("temporal-fixture"),
        boot_id: BootId::from("boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("fixture@1"),
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
fn selected(identity: &str, implementation: &str) -> PlannedGear {
    selected_plan(identity, implementation).fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str().starts_with("numeric/"))
        .unwrap()
        .clone()
}

fn frame(inputs: &[&[u8]], ready: bool) -> StepIo<4> {
    let refs = core::array::from_fn(|i| {
        inputs.get(i).map(|bytes| ValueRef {
            slot: i as u16,
            generation: 1,
            byte_len: bytes.len() as u32,
        })
    });
    StepIo::test_frame(
        refs,
        [false; 4],
        [ready.then_some(16384), None, None, None],
        None,
        16,
    )
}
#[test]
fn explicit_flow_elementwise_reuses_owner_without_one_shot_rearming() {
    let gear = selected("numeric/multiply160", FLOW_ELEMENTWISE_IMPLEMENTATION);
    let mut back = FixedElementwiseBack::<160>::prepare_flow_planned::<4>(
        &gear,
        3,
        FixedElementwiseOperation::Multiply,
    )
    .unwrap();
    assert!(FixedElementwiseBack::<160>::prepare_planned::<4>(
        &gear,
        3,
        FixedElementwiseOperation::Multiply
    )
    .is_err());
    let mut codec =
        FixedF32VectorCodec::<160>::prepare(&fixed_numeric_type("NumericF32Vector160").unwrap())
            .unwrap();
    for i in 1..=3 {
        let a = codec.encode(&[i as f32; 160]).unwrap().to_vec();
        let b = codec.encode(&[2.; 160]).unwrap().to_vec();
        let inputs =
            StepInputBytes::test_frame([Some(a.as_slice()), Some(b.as_slice()), None, None], None);
        let mut io = frame(&[&a, &b], false);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
        assert!(!io.test_consumed(PortId(0)));
        assert_eq!(back.committed_frames(), i - 1);
        let mut io = frame(&[&a, &b], true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        let mut out = [0.; 160];
        codec
            .decode(
                <FixedElementwiseBack<160> as StepBack<4>>::prepared_output(&back, PortId(0))
                    .unwrap(),
                &mut out,
            )
            .unwrap();
        assert_eq!(out, [2. * i as f32; 160]);
        assert_eq!(back.committed_frames(), i - 1);
        <FixedElementwiseBack<160> as StepBack<4>>::step_committed(&mut back);
        assert_eq!(back.committed_frames(), i);
    }
    let mut closed = StepIo::test_frame([None; 4], [true, true, false, false], [None; 4], None, 16);
    assert_eq!(
        back.step(&mut closed, &StepInputBytes::test_frame([None; 4], None)),
        StepOutcome::Complete
    );
}
#[test]
fn explicit_flow_index_tanh_and_concat_have_selected_fore_identity() {
    let gear = selected("numeric/gather256x44", FLOW_INDEX_IMPLEMENTATION);
    let _back =
        FixedIndexBack::<256, 44>::prepare_flow_planned::<4>(&gear, 3, FixedIndexOperation::Gather)
            .unwrap();
    let gear = selected("numeric/tanh128", FLOW_OPERATION_IMPLEMENTATION);
    let _back = FixedTanhBack::<128>::prepare_flow_planned::<4>(&gear, 2).unwrap();
    let gear = selected("numeric/concatenate20x12", FLOW_OPERATION_IMPLEMENTATION);
    let _back = FixedConcatenateBack::<20, 12, 32>::prepare_flow_planned::<4>(&gear, 3).unwrap();
}

#[test]
fn closing_gather_reuses_explicit_indices_and_cancellation_is_terminal() {
    use conduit_ai::fixed_numeric_index_codec::FixedU16IndexCodec;
    let gear = selected("numeric/gather256x44", FLOW_INDEX_IMPLEMENTATION);
    let mut back =
        FixedIndexBack::<256, 44>::prepare_flow_planned::<4>(&gear, 3, FixedIndexOperation::Gather)
            .unwrap();
    let mut input =
        FixedF32VectorCodec::<256>::prepare(&fixed_numeric_type("NumericF32Vector256").unwrap())
            .unwrap();
    let mut indices =
        FixedU16IndexCodec::<44>::prepare(&fixed_numeric_type("NumericU16Indices44").unwrap())
            .unwrap();
    let output =
        FixedF32VectorCodec::<44>::prepare(&fixed_numeric_type("NumericF32Vector44").unwrap())
            .unwrap();
    for frame_index in 0..3 {
        let values = core::array::from_fn(|i| i as f32 + frame_index as f32);
        let positions = core::array::from_fn(|i| (255 - i) as u16);
        let a = input.encode(&values).unwrap();
        let b = indices.encode(&positions);
        let inputs = StepInputBytes::test_frame([Some(a), Some(b), None, None], None);
        let mut io = frame(&[a, b], false);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
        assert_eq!(back.committed_frames(), frame_index);
        let mut io = frame(&[a, b], true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        let mut result = [0.; 44];
        output
            .decode(
                <FixedIndexBack<256, 44> as StepBack<4>>::prepared_output(&back, PortId(0))
                    .unwrap(),
                &mut result,
            )
            .unwrap();
        assert_eq!(
            result,
            core::array::from_fn(|i| values[usize::from(positions[i])])
        );
        <FixedIndexBack<256, 44> as StepBack<4>>::step_committed(&mut back);
    }
    <FixedIndexBack<256, 44> as StepBack<4>>::cancel(&mut back);
    let mut io = frame(&[], true);
    assert!(matches!(
        back.step(&mut io, &StepInputBytes::test_frame([None; 4], None)),
        StepOutcome::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::Cancelled,
            ..
        })
    ));
    assert_eq!(back.committed_frames(), 3);
    assert!(<FixedIndexBack<256, 44> as StepBack<4>>::prepared_output(&back, PortId(0)).is_none());
}

#[test]
fn hosted_factory_prepares_the_distinct_reusable_numeric_owners() {
    use conduit_composite::KernelOperationFactory;
    for (identity, implementation) in [
        ("numeric/multiply160", FLOW_ELEMENTWISE_IMPLEMENTATION),
        ("numeric/gather256x44", FLOW_INDEX_IMPLEMENTATION),
        ("numeric/tanh128", FLOW_OPERATION_IMPLEMENTATION),
        ("numeric/concatenate20x12", FLOW_OPERATION_IMPLEMENTATION),
        (
            "numeric/history2x64",
            conduit_ai::fixed_numeric_window_back::FLOW_WINDOW_IMPLEMENTATION,
        ),
        (
            "numeric/one-pole40",
            conduit_ai::fixed_numeric_scan_back::FLOW_SCAN_IMPLEMENTATION,
        ),
        ("numeric/gather640x160", FLOW_INDEX_IMPLEMENTATION),
        ("numeric/concatenate1x1", FLOW_OPERATION_IMPLEMENTATION),
        ("numeric/concatenate2x1", FLOW_OPERATION_IMPLEMENTATION),
    ] {
        let plan = selected_plan(identity, implementation);
        let gear = plan.fragments[0]
            .placements
            .iter()
            .find(|gear| gear.kind_id.as_str().starts_with("numeric/"))
            .unwrap();
        let factories = conduit_std_host::fixed_numeric::FixedNumericOperationFactory::for_plan(
            &plan,
            &std::collections::BTreeMap::new(),
        )
        .unwrap();
        let factory = factories
            .iter()
            .find(|factory| factory.implementation_id().as_str() == implementation)
            .unwrap();
        let mut values = conduit_kernel::HostedValueStore::new(4, 16384, 65536).unwrap();
        let _back = factory.prepare(gear, &mut values).unwrap();
    }
}

#[test]
fn flow_window_keeps_next_history_and_window_in_one_committed_result() {
    use conduit_ai::fixed_numeric_window_back::*;
    let gear = selected("numeric/history2x64", FLOW_WINDOW_IMPLEMENTATION);
    let mut back = FixedWindowBack::prepare_flow_planned::<4>(&gear, 3).unwrap();
    let mut value_codec =
        FixedF32VectorCodec::<64>::prepare(&fixed_numeric_type("NumericF32Vector64").unwrap())
            .unwrap();
    let mut history_codec = FixedF32VectorCodec::<128>::prepare_history().unwrap();
    let result_codec = FixedF32VectorCodec::<320>::prepare_window_result().unwrap();
    let mut history = [0.; 128];
    for epoch in 1..=3 {
        let value = [epoch as f32; 64];
        let value_bytes = value_codec.encode(&value).unwrap();
        let history_bytes = history_codec.encode(&history).unwrap();
        let mut ports = [None; 4];
        for (i, port) in gear.inputs.iter().enumerate() {
            ports[i] = Some(if port.port_id.as_str() == "value" {
                value_bytes
            } else {
                history_bytes
            });
        }
        let bytes: Vec<_> = ports.iter().flatten().copied().collect();
        let inputs = StepInputBytes::test_frame(ports, None);
        let mut io = frame(&bytes, false);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
        assert_eq!(back.committed_frames(), epoch - 1);
        assert!(<FixedWindowBack as StepBack<4>>::prepared_output(&back, PortId(0)).is_none());
        let mut io = frame(&bytes, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        let mut result = [0.; 320];
        result_codec
            .decode(
                <FixedWindowBack as StepBack<4>>::prepared_output(&back, PortId(0)).unwrap(),
                &mut result,
            )
            .unwrap();
        assert_eq!(&result[..64], &history[64..]);
        assert_eq!(&result[64..128], &value);
        assert_eq!(&result[128..256], &history);
        assert_eq!(&result[256..], &value);
        assert_eq!(back.committed_frames(), epoch - 1);
        <FixedWindowBack as StepBack<4>>::step_committed(&mut back);
        assert_eq!(back.committed_frames(), epoch);
        history.copy_from_slice(&result[..128]);
    }
}
#[test]
fn flow_one_pole_proposes_exact_value_and_returned_state_without_private_recurrence() {
    use conduit_ai::fixed_numeric_scan_back::*;
    let gear = selected("numeric/one-pole40", FLOW_SCAN_IMPLEMENTATION);
    let mut back = FixedOnePoleBack::prepare_flow_planned::<4>(&gear, 4).unwrap();
    let mut values =
        FixedF32VectorCodec::<40>::prepare(&fixed_numeric_type("NumericF32Vector40").unwrap())
            .unwrap();
    let mut scalar =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap();
    let decoder = FixedF32VectorCodec::<41>::prepare_one_pole_result().unwrap();
    let mut prior = 0.25f32;
    for epoch in 1..=3 {
        let input = [epoch as f32; 40];
        let a = values.encode(&input).unwrap();
        let b = scalar.encode(&[0.5]).unwrap().to_vec();
        let c = scalar.encode(&[prior]).unwrap().to_vec();
        let inputs = StepInputBytes::test_frame(
            [Some(a), Some(b.as_slice()), Some(c.as_slice()), None],
            None,
        );
        let mut io = frame(&[a, &b, &c], false);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
        assert!(!io.test_consumed(PortId(2)));
        let mut io = frame(&[a, &b, &c], true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        let mut result = [0.; 41];
        decoder
            .decode(
                <FixedOnePoleBack as StepBack<4>>::prepared_output(&back, PortId(0)).unwrap(),
                &mut result,
            )
            .unwrap();
        let mut reference = f64::from(prior);
        for (actual, value) in result[1..].iter().zip(input) {
            reference = f64::from(value) + 0.5 * reference;
            assert!((f64::from(*actual) - reference).abs() < 1e-5);
        }
        assert_eq!(result[0], result[40]);
        assert_eq!(back.committed_frames(), epoch - 1);
        <FixedOnePoleBack as StepBack<4>>::step_committed(&mut back);
        prior = result[0];
    }
    <FixedOnePoleBack as StepBack<4>>::cancel(&mut back);
    let mut io = frame(&[], true);
    assert!(matches!(
        back.step(&mut io, &StepInputBytes::test_frame([None; 4], None)),
        StepOutcome::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::Cancelled,
            ..
        })
    ));
    assert_eq!(back.committed_frames(), 3);
}
