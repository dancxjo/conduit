use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_semantic_catalog::state_value::*;

#[test]
fn malformed_input_preserves_committed_state_and_is_not_completion() {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let initial =
        StructuredInfoValue::leaf(ty.clone(), InfoBool::new(false).encode().to_vec()).unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    startup.insert_structured_type("Cell", ty.clone()).unwrap();
    install_state_value_kind("Cell", &ty, &initial, &mut startup, &mut profile).unwrap();
    let plot = conduit_plot::parse_with_startup(
        "plot retained {\n cell: state/value(initial = true)\n}\n",
        &startup,
        &profile,
    )
    .unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("state-host"),
        boot_id: BootId::from("state-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("state-test@1"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![
            conduit_std_offers::state_value_std_offer("Cell", &ty, &initial).unwrap(),
        ],
    }];
    let placements = conduit_planner::default_placements(&plot, &hosts).unwrap();
    let plan = conduit_planner::plan(&plot, &hosts, &placements, &[]).unwrap();
    let state = derive_state_boundary(&plot, &GearId::from("retained/cell"), 64).unwrap();
    let mut back = conduit_std_host::state_value::TypedStateBack::prepare(
        &plan.fragments[0].placements[0],
        &state,
        0,
        PortId(0),
        PortId(0),
    )
    .unwrap();
    let mut initial_io = StepIo::test_frame([None], [false], [Some(64)], None, 4);
    assert_eq!(
        back.step(&mut initial_io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(initial_io.test_prepared_output(), Some((PortId(0), 1)));
    assert_eq!(
        StepBack::<1>::prepared_output(&back, PortId(0)),
        Some(InfoBool::new(true).encode().as_slice())
    );
    // A leaf State carries the exact primitive payload Kind on its runtime
    // cord. The structured envelope remains admission/configuration truth.
    let next = InfoBool::new(false).encode().to_vec();
    let reference = ValueRef {
        slot: 0,
        generation: 0,
        byte_len: next.len() as u32,
    };
    let mut next_io = StepIo::test_frame([Some(reference)], [false], [Some(64)], None, 4);
    assert_eq!(
        back.step(
            &mut next_io,
            &StepInputBytes::test_frame([Some(&next)], None),
        ),
        StepOutcome::Progress
    );
    assert_eq!(
        back.generation(),
        0,
        "proposing output does not publish State"
    );
    StepBack::<1>::step_committed(&mut back);
    assert_eq!(back.current(), next);
    assert_eq!(back.generation(), 1);
    let before = back.current().to_vec();
    let invalid = [255_u8];
    let reference = ValueRef {
        slot: 0,
        generation: 0,
        byte_len: 1,
    };
    let mut invalid_io = StepIo::test_frame([Some(reference)], [false], [Some(64)], None, 4);
    assert!(matches!(
        back.step(
            &mut invalid_io,
            &StepInputBytes::test_frame([Some(&invalid)], None),
        ),
        StepOutcome::Fail(_)
    ));
    assert_eq!(back.current(), before);
    assert_eq!(back.generation(), 1);
}
