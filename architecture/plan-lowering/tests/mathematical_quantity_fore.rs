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
