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
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    startup.insert_structured_type("Cell", ty.clone()).unwrap();
    install_state_value_kind("Cell", &ty, &initial, &mut startup, &mut profile).unwrap();
    let form = conduit_form::parse_with_startup(
        "form retained {\n cell: state/value(initial = true)\n}\n",
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
        capabilities: vec![conduit_std_offers::state_value_std_offer("Cell", &ty).unwrap()],
    }];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan(&form, &hosts, &placements, &[]).unwrap();
    let state = derive_state_boundary(&form, &GearId::from("retained/cell"), 64).unwrap();
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
    assert!(initial_io.test_canonical_output().is_some());
    let next = initial.canonical_bytes().unwrap();
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
