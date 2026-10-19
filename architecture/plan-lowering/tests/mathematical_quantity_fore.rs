//! Sealed lowering fixture, not authored Source or installed Back execution.
use conduit_core::*;
use conduit_plan_lowering::lowering::{lower_plan_fragment, ForeValueRefusal, LoweringError};
#[path = "../../core/tests/common/sealed_state.rs"]
mod common;
fn fragment(profile: QuantityValueProfile) -> PlanFragment {
    let mut f = common::fragment();
    f.states.clear();
    f.sign_storage_budget = mandatory_sign_storage_requirement(&f.expected_sign).unwrap();
    let placement = &mut f.placements[0];
    for port in placement
        .inputs
        .iter_mut()
        .chain(placement.outputs.iter_mut())
    {
        port.value_kind = kind_id(DISTANCE_INFO_ID);
    }
    placement.limits.max_queue_bytes = profile.maximum_encoded_bytes() as u32;
    placement.realization_properties = vec![QuantityWireContract::new(
        vec![("next".into(), profile)],
        vec![("current".into(), profile)],
    )
    .unwrap()
    .to_property()
    .unwrap()];
    f.fore_ports = [
        ("input", "next", PortDirection::Input),
        ("output", "current", PortDirection::Output),
    ]
    .into_iter()
    .map(|(front, gear, direction)| PlannedForePort {
        front_port_id: port_id(front),
        direction,
        placement_id: PlacementId::from("placement"),
        gear_port_id: port_id(gear),
        value_kind: kind_id(DISTANCE_INFO_ID),
        value_contract: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: profile.maximum_encoded_bytes() as u32,
        selected_line: None,
    })
    .collect();
    f
}
fn wire(symbolic: bool) -> Vec<u8> {
    let mut work = ExactNumericWork::new(200_000_000);
    let value = if symbolic {
        MathematicalValue::parse_literal("pi", &mut work).unwrap()
    } else {
        MathematicalValue::rational(ExactRational::ratio(1, 3, &mut work).unwrap())
    };
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut coordinate).unwrap();
    let view = MathematicalValueView::decode(&coordinate[..n], &mut work).unwrap();
    let mut out = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &view,
        Unit::Meter,
        QuantityRole::Linear,
        &mut out,
        &mut work,
    )
    .unwrap();
    out.truncate(n);
    out
}
#[test]
fn selected_mathematical_fore_retains_sealed_profile_and_rejects_undersized_output() {
    let f = fragment(QuantityValueProfile::Mathematical);
    let plan = common::seal(f.clone());
    assert!(verify_plan(&plan));
    let lowered = lower_plan_fragment(&plan.fragments[0]).unwrap();
    for port in &lowered.fore_ports {
        assert_eq!(port.quantity_profile, QuantityValueProfile::Mathematical);
        assert_eq!(port.value_kind, kind_id(DISTANCE_INFO_ID));
        assert!(port.validate_value(&wire(true)).is_ok());
    }
    let mut narrow = f;
    narrow.fore_ports[1].byte_capacity = QUANTITY_VALUE_MAX_ENCODED_BYTES as u32;
    let plan = common::seal(narrow);
    assert!(matches!(
        lower_plan_fragment(&plan.fragments[0]),
        Err(LoweringError::ConnectionContractMismatch(_))
    ));
}
#[test]
fn bounded_fore_uses_selected_profile_and_preserves_physical_range_refusals() {
    let mut f = fragment(QuantityValueProfile::Mathematical);
    f.fore_ports[0].byte_capacity = 1024;
    f.fore_ports[0].value_contract = Some(
        CheckedValueContract::new(
            kind_id(DISTANCE_INFO_ID),
            1024,
            vec![ValueConstraint::QuantityRange {
                minimum: Some(Box::new(Quantity::parse_plot_literal("25cm").unwrap())),
                maximum: Some(Box::new(Quantity::parse_plot_literal("50cm").unwrap())),
                minimum_endpoint: IntervalEndpoint::Inclusive,
                maximum_endpoint: IntervalEndpoint::Exclusive,
            }],
        )
        .unwrap(),
    );
    f.placements[0]
        .semantic_contract
        .laws
        .push(KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Input(port_id("next")),
            contract: f.fore_ports[0].value_contract.clone().unwrap(),
        }]));
    let plan = common::seal(f);
    let lowered = lower_plan_fragment(&plan.fragments[0]).unwrap();
    let port = &lowered.fore_ports[0];
    assert!(port.validate_value(&wire(false)).is_ok());
    assert!(matches!(
        port.validate_value(&wire(true)),
        Err(ForeValueRefusal::ProfileConstraint(
            QuantityProfileContractRefusal::Contract(ValueConstraintRefusal::QuantityRange)
        ))
    ));
    assert_eq!(
        port.validate_value(&vec![0; 1025]),
        Err(ForeValueRefusal::CordCapacity)
    );
    for profile in [QuantityValueProfile::Decimal, QuantityValueProfile::Exact] {
        let mut old = port.clone();
        old.quantity_profile = profile;
        assert!(matches!(
            old.validate_value(&wire(false)),
            Err(ForeValueRefusal::ProfileConstraint(
                QuantityProfileContractRefusal::Quantity(
                    QuantityValueContractRefusal::UnsupportedRepresentation { version: 3, .. }
                )
            ))
        ));
    }
    assert!(port.validate_value(&wire(false)).is_ok());
}

#[path = "../../core/tests/common/numerical_allocation.rs"]
mod allocation_probe;
#[derive(Clone, Copy, Default)]
struct Carry {
    cancelled: bool,
}
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
impl conduit_kernel::scheduler::StepBack<PORTS> for Carry {
    fn step(
        &mut self,
        io: &mut conduit_kernel::scheduler::StepIo<PORTS>,
        _: &conduit_kernel::scheduler::StepInputBytes<'_, PORTS>,
    ) -> conduit_kernel::scheduler::StepOutcome {
        use conduit_kernel::{scheduler::StepOutcome, PortId};
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.consume(PortId(0)).unwrap();
        io.send(PortId(0), value).unwrap();
        StepOutcome::Progress
    }
    fn cancel(&mut self) {
        self.cancelled = true;
    }
}
#[test]
fn negotiated_frame_crosses_production_kernel_pressure_delivery_and_cancellation_without_growth() {
    use conduit_kernel::{
        scheduler::{FixedScheduler, RemoteIngressOutcome, SchedulerError},
        FixedRoutes, FixedSignLog, FixedValueStore, ValueStorage,
    };
    let plan = common::seal(fragment(QuantityValueProfile::Mathematical));
    let lowered = lower_plan_fragment(&plan.fragments[0]).unwrap();
    let input = lowered.fore_ports[0].clone();
    let output = lowered.fore_ports[1].clone();
    let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::<_, _, _, 1, 2, PORTS, 2, 1, 1>::new(
        lowered.node_specs.try_into().unwrap(),
        [lowered.cords[0].spec, lowered.cords[1].spec],
        routes,
        [Carry::default()],
        FixedValueStore::<3, MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES>::new(
            (3 * MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES) as u32,
        )
        .unwrap(),
        FixedSignLog::<128>::new_with_remote_storage(
            (128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
            128,
            conduit_kernel::remote_sign_storage_bytes(128).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let source = wire(true);
    let mut payload = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let mut converter = PreparedMathematicalQuantityConversion::new(200000, 200_000_000).unwrap();
    let receipt = converter
        .execute(
            &source,
            Unit::Centimeter,
            &mut payload,
            &mut ExactNumericWork::new(200_000_000),
        )
        .unwrap();
    payload.truncate(receipt.encoded_bytes());
    let mut evaluator = PreparedMathematicalQuantityEnclosure::new(
        MathematicalEnclosureRequirements {
            maximum_width: ExactRational::ratio(
                1,
                100_000_000_000_000,
                &mut ExactNumericWork::new(200_000_000),
            )
            .unwrap(),
            maximum_leaf_width: ExactRational::ratio(
                1,
                100_000_000_000_000_000,
                &mut ExactNumericWork::new(200_000_000),
            )
            .unwrap(),
            maximum_constant_terms: 32,
            maximum_root_refinements: 64,
            maximum_scratch_payload_bytes: 131072,
            maximum_retained_payload_bytes: 131072,
        },
        200_000_000,
    )
    .unwrap();
    let retained = evaluator.retained_payload_bytes();
    let (_, allocations) = allocation_probe::observe(|| {
        input.validate_value(&payload).unwrap();
        assert_eq!(
            scheduler
                .admit_remote_input(input.endpoint, input.cord, 0, &payload)
                .unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        );
        scheduler.step().unwrap();
        let first = scheduler
            .remote_egress_offer(output.endpoint, output.cord)
            .unwrap()
            .unwrap();
        assert_eq!(
            scheduler
                .remote_egress_offer(output.endpoint, output.cord)
                .unwrap()
                .unwrap(),
            first
        );
        assert_eq!(
            scheduler
                .admit_remote_input(input.endpoint, input.cord, 1, &payload)
                .unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 1 }
        );
        scheduler.step().unwrap();
        assert_eq!(
            scheduler.cord_usage(input.cord).unwrap(),
            (1, payload.len() as u32)
        );
        assert_eq!(
            scheduler
                .admit_remote_input(input.endpoint, input.cord, 2, &payload)
                .unwrap(),
            RemoteIngressOutcome::Full { sequence: 2 }
        );
        assert_eq!(scheduler.values().used_items(), 2);
        for sequence in 0..2 {
            let offer = scheduler
                .remote_egress_offer(output.endpoint, output.cord)
                .unwrap()
                .unwrap();
            assert_eq!(offer.sequence, sequence);
            let bytes = scheduler.values().get(offer.value).unwrap();
            output.validate_value(bytes).unwrap();
            assert_eq!(bytes, payload);
            let physical =
                MathematicalQuantityView::decode(bytes, &mut ExactNumericWork::new(200_000_000))
                    .unwrap();
            assert_eq!(physical.unit(), Unit::Centimeter);
            assert_eq!(physical.family(), Unit::Meter.family());
            assert_eq!(physical.role(), QuantityRole::Linear);
            let numerical = evaluator
                .execute(bytes, &mut ExactNumericWork::new(200_000_000))
                .unwrap();
            assert_eq!(numerical.source().encoded(), payload);
            assert_eq!(numerical.lower().unit(), Unit::Centimeter);
            assert_ne!(numerical.width_coordinate(), ExactRational::ZERO);
            scheduler
                .remote_egress_accept(output.endpoint, output.cord, offer.sequence)
                .unwrap();
            scheduler
                .remote_egress_delivered(output.endpoint, output.cord, offer.sequence)
                .unwrap();
            if sequence == 0 {
                scheduler.step().unwrap();
            }
        }
        assert_eq!(scheduler.values().used_items(), 0);
        assert_eq!(
            scheduler
                .admit_remote_input(input.endpoint, input.cord, 2, &payload)
                .unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 2 }
        );
        scheduler.cancel().unwrap();
        assert!(scheduler.drivers()[0].cancelled);
        assert_eq!(scheduler.values().used_items(), 0);
        assert_eq!(
            scheduler.admit_remote_input(input.endpoint, input.cord, 3, &payload),
            Err(SchedulerError::Cancelled)
        );
        assert_eq!(evaluator.retained_payload_bytes(), retained);
    });
    assert_eq!(allocations, 0);
}
