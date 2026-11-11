//! Separate finite State profiles over source-authored native structured values.
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::state_value::{LargeTypedStateBack, TypedStateBack};

fn authored(
    maximum: u32,
) -> (
    conduit_plot::CheckedPlot,
    StructuredInfoType,
    StructuredInfoValue,
) {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let source = format!("type Cell = {{\n payload: Text <= 6000B\n}}\nplot retained {{\n cell: keep Cell({{payload: \"\"}}) <= {maximum}B for this play\n}}\n");
    let expanded =
        conduit_plot::parse_with_startup(&source, &startup, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Structured(initial) = &expanded.gears[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "initial")
        .unwrap()
        .value
    else {
        panic!("native initialized State")
    };
    let initial = StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).unwrap();
    (expanded, initial.value_type().clone(), initial)
}
fn host(offer: CapabilityOffer) -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/large-state-proof".into(),
        boot_id: "boot/large-state-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: "large-state-proof@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![offer],
    }
}
fn planned(maximum: u32, large: bool) -> (Plan, StructuredInfoType) {
    let (plot, ty, initial) = authored(maximum);
    let offer = if large {
        conduit_std_offers::state_value_large_std_offer("Cell", &ty, &initial)
    } else {
        conduit_std_offers::state_value_std_offer("Cell", &ty, &initial)
    }
    .unwrap();
    let hosts = [host(offer)];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    (
        conduit_planner::plan(&plot, &hosts, &placements, &[]).unwrap(),
        ty,
    )
}

fn text_value(ty: &StructuredInfoType, text: &str) -> StructuredInfoValue {
    // Construct values through the same native expression compiler as authored code.
    let source = format!("type Cell = {{\n payload: Text <= 6000B\n}}\nplot value (\n >> unused: Boolean\n result: Cell >>\n) = ({{payload: \"{text}\"}})\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "value",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(program) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("expression program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(
        &program.evaluate(&InfoBool::FALSE.encode()).unwrap(),
    )
    .unwrap();
    assert_eq!(value.value_type(), ty);
    value
}
#[test]
fn separate_large_profile_plans_and_commits_native_value_above_four_kib() {
    let (plan, ty) = planned(65_536, true);
    let fragment = &plan.fragments[0];
    let state = &fragment.states[0];
    let gear = fragment
        .placements
        .iter()
        .find(|gear| gear.gear_id == state.gear_id)
        .unwrap();
    assert_eq!(state.maximum_value_bytes, 65_536);
    assert_eq!(
        gear.execution_profile_id.as_str(),
        conduit_std_offers::STATE_VALUE_LARGE_STD_PROFILE
    );
    assert_eq!(
        gear.implementation_id.as_str(),
        conduit_std_offers::STATE_VALUE_LARGE_STD_IMPLEMENTATION
    );
    assert!(TypedStateBack::prepare(gear, state, 0, PortId(0), PortId(0)).is_err());
    let mut back = LargeTypedStateBack::prepare(gear, state, 0, PortId(0), PortId(0)).unwrap();
    let mut initial = StepIo::test_frame([None], [false], [Some(65_536)], None, 4);
    assert_eq!(
        back.step(&mut initial, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    StepBack::<1>::step_committed(&mut back);
    let value = text_value(&ty, &"x".repeat(5000))
        .canonical_bytes()
        .unwrap();
    assert!(value.len() > 4096 && value.len() <= 65_536);
    let reference = ValueRef {
        slot: 0,
        generation: 0,
        byte_len: value.len() as u32,
    };
    let mut io = StepIo::test_frame([Some(reference)], [false], [Some(65_536)], None, 4);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&value)], None)),
        StepOutcome::Progress
    );
    assert_eq!(back.generation(), 0);
    assert_eq!(
        StepBack::<1>::prepared_output(&back, PortId(0)).unwrap(),
        value
    );
    StepBack::<1>::step_committed(&mut back);
    assert_eq!(back.current(), value);
    assert_eq!(back.generation(), 1);
}
#[test]
fn original_profile_refuses_the_larger_authored_capacity() {
    let (plot, ty, initial) = authored(65_536);
    let hosts = [host(
        conduit_std_offers::state_value_std_offer("Cell", &ty, &initial).unwrap(),
    )];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    assert!(matches!(
        conduit_planner::plan(&plot, &hosts, &placements, &[]),
        Err(conduit_planner::PlannerError::InvalidStateContract(_))
    ));
    let (small_plan, _) = planned(4096, false);
    let state = &small_plan.fragments[0].states[0];
    let gear = &small_plan.fragments[0].placements[0];
    assert!(TypedStateBack::prepare(gear, state, 0, PortId(0), PortId(0)).is_ok());
    assert!(LargeTypedStateBack::prepare(gear, state, 0, PortId(0), PortId(0)).is_err());
}
#[test]
fn large_profile_refuses_over_bound_input_without_committing() {
    let (plan, _) = planned(65_536, true);
    let state = &plan.fragments[0].states[0];
    let gear = &plan.fragments[0].placements[0];
    let mut back = LargeTypedStateBack::prepare(gear, state, 0, PortId(0), PortId(0)).unwrap();
    let original = back.current().to_vec();
    let bytes = vec![0_u8; 65_537];
    let reference = ValueRef {
        slot: 0,
        generation: 0,
        byte_len: bytes.len() as u32,
    };
    let mut io = StepIo::test_frame([Some(reference)], [false], [Some(65_536)], None, 4);
    assert!(matches!(
        back.step(&mut io, &StepInputBytes::test_frame([Some(&bytes)], None)),
        StepOutcome::Fail(_)
    ));
    assert_eq!(back.current(), original);
    assert_eq!(back.generation(), 0);
}
