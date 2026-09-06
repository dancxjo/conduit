use conduit_core::*;
use conduit_semantic_catalog::state_value::*;
mod common;

#[test]
fn authored_state_reaches_an_exact_plan_and_rejects_silent_initialization_or_capacity_changes() {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let seed = StructuredInfoValue::leaf(ty.clone(), b"false".to_vec()).unwrap();
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    startup.insert_structured_type("Cell", ty.clone()).unwrap();
    install_state_value_kind("Cell", &ty, &seed, &mut startup, &mut profile).unwrap();
    // Planning-only external Flow; runtime production is outside this proof.
    let mut source = conduit_std_offers::state_value_std_offer("Cell", &ty).unwrap();
    source.kind_id = kind_id("fixture/typed-flow");
    source.kind_contract_revision = KindContractRevision::from("fixture/typed-flow@1");
    source.capability_id = CapabilityId::from("fixture/typed-flow");
    source.implementation.execution_profile_id = ExecutionProfileId::from("fixture/typed-flow@1");
    source.implementation.implementation_id = ImplementationId::from("fixture/typed-flow@1");
    source.implementation.artifact_id = ArtifactId::from("fixture/planning-only@1");
    source.startup_parameters.clear();
    source.inputs.clear();
    source.shorthand = None;
    source.outputs[0].temporal = PortTemporal::Flow { closes: true };
    startup
        .insert(conduit_form::KindSignature {
            kind: "fixture/typed-flow".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: source.kind_id.clone(),
            kind_contract_revision: source.kind_contract_revision.clone(),
            inputs: vec![],
            outputs: source.outputs.clone(),
            configuration: vec![],
        })
        .unwrap();
    let form = conduit_form::parse_with_startup(
        "form retained {\n cell: state/value(initial = true)\n source: fixture/typed-flow\n source.current > cell.next\n}\n",
        &startup,
        &profile,
    )
    .unwrap();
    let mut host = common::standard_planning_fixture("state-host", "state-boot");
    host.capabilities = vec![
        conduit_std_offers::state_value_std_offer("Cell", &ty).unwrap(),
        source,
    ];
    let hosts = [host];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        1,
        64,
    )
    .unwrap();
    let state = derive_state_boundary(&form, &GearId::from("retained/cell"), 64).unwrap();
    validate_state_placement(&plan.fragments[0].placements[0], &state).unwrap();
    let sealed =
        conduit_planner::state_delay::plan::seal_state_plan(&form, &plan, vec![state.clone()])
            .unwrap();
    assert!(verify_plan(&sealed));
    assert_eq!(sealed.checked_form_id, form.checked_form_id);
    assert_ne!(sealed.plan_id, plan.plan_id);
    let placement = &sealed.fragments[0].placements[0];
    let mut altered = state.clone();
    altered.initial_value = seed.canonical_bytes().unwrap();
    assert_eq!(
        validate_state_placement(placement, &altered),
        Err(StateValueAdmissionError::InvalidInitialization)
    );
    altered = state.clone();
    altered.maximum_value_bytes = 65;
    assert_eq!(
        validate_state_placement(placement, &altered),
        Err(StateValueAdmissionError::InvalidCapacity)
    );
    altered = state;
    altered.continuation = StateContinuation::MaximumTransitions(1);
    assert_eq!(
        validate_state_placement(placement, &altered),
        Err(StateValueAdmissionError::WrongContract)
    );
}
