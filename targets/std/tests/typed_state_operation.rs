use conduit_core::*;
use conduit_kernel::{Operation, OperationAction, PortId, ValueRef};
use conduit_semantic_catalog::state_value::*;

#[test]
fn malformed_input_preserves_committed_state_and_is_not_completion() {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let initial = StructuredInfoValue::leaf(ty.clone(), b"false".to_vec()).unwrap();
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
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![conduit_std_offers::state_value_std_offer("Cell", &ty).unwrap()],
    }];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan(&form, &hosts, &placements, &[]).unwrap();
    let state = derive_state_boundary(&form, &GearId::from("retained/cell"), 64).unwrap();
    let mut operation = conduit_std_host::state_value::TypedStateOperation::prepare(
        &plan.fragments[0].placements[0],
        &state,
        0,
        PortId(0),
        PortId(0),
    )
    .unwrap();
    assert!(matches!(
        operation.start(),
        OperationAction::EmitCanonical { .. }
    ));
    let next = initial.canonical_bytes().unwrap();
    let reference = ValueRef {
        slot: 0,
        generation: 0,
        byte_len: next.len() as u32,
    };
    assert!(matches!(
        operation.resume_value(PortId(0), reference, &next),
        OperationAction::EmitCanonical { .. }
    ));
    assert_eq!(operation.current(), next);
    assert_eq!(operation.generation(), 1);
    assert!(matches!(operation.advance(), OperationAction::Await));
    let before = operation.current().to_vec();
    let invalid = [255_u8];
    let reference = ValueRef {
        slot: 0,
        generation: 0,
        byte_len: 1,
    };
    assert!(matches!(
        operation.resume_value(PortId(0), reference, &invalid),
        OperationAction::Fail(_)
    ));
    assert_eq!(operation.current(), before);
    assert_eq!(operation.generation(), 1);
}
