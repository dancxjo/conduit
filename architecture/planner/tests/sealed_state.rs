use conduit_core::{
    GearId, PlannedStateBoundary, StateContinuation, StateId, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};
use conduit_planner::state_delay::{plan::seal_state_plan, StateGraphError};
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
mod common;

#[test]
fn checked_state_is_sealed_into_a_fresh_plan_with_exact_evidence_capacity() {
    let mut profile = conduit_semantic_catalog::standard_profile_catalog();
    let mut source = conduit_semantic_catalog::scalar_literal_contract();
    source.kind_id = conduit_core::kind_id("fixture/scalar-flow");
    source.plain_name = "Planning-only scalar flow".into();
    source.summary = "Typed source fixture for structural Plan admission".into();
    source.example = "source: fixture/scalar-flow".into();
    source.configuration.clear();
    source.limits = conduit_semantic_catalog::state_latest_scalar_contract().limits;
    source.outputs[0].temporal = conduit_core::PortTemporal::Flow { closes: true };
    source.terminal_behavior =
        conduit_semantic_catalog::TerminalBehavior::HostInputEndsOrFailsSource;
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: source.kind_id.clone(),
            kind_contract_revision: conduit_core::KindContractRevision::from(
                "fixture/scalar-flow@1",
            ),
            inputs: source.inputs.clone(),
            outputs: source.outputs.clone(),
            configuration: vec![],
        })
        .unwrap();
    let startup = profile.startup_catalog().unwrap();
    let form = conduit_form::parse_with_startup(
        "form retained {\n source: fixture/scalar-flow\n cell: state/latest\n source.value > cell.in\n}\n",
        &startup,
        &profile,
    ).unwrap();
    let mut host = common::standard_planning_fixture("host", "boot");
    // This source is a planning-only fixture, not an installed runtime claim.
    host.capabilities
        .push(conduit_semantic_catalog::realization_offer(
            source,
            "fixture/scalar-flow@1",
            conduit_semantic_catalog::RealizationOfferIdentity {
                capability: "fixture/scalar-flow",
                execution_profile: "fixture/scalar-flow@1",
                implementation: "fixture/scalar-flow@1",
                artifact: "fixture/planning-only@1",
            },
            vec![],
            vec![],
            vec![],
        ));
    let hosts = [host];
    let placements = default_placements(&form, &hosts).unwrap();
    let ordinary = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            conduit_core::LOCAL_BASE_IMPLEMENTATION_ID,
        )],
        PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let state = PlannedStateBoundary {
        state_id: StateId::from("retained-state"),
        gear_id: GearId::from("retained/cell"),
        value_kind: conduit_core::KindId::from(SCALAR_INFO_ID),
        initial_value: vec![0; SCALAR_ENCODED_LEN],
        retained: None,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
        continuation: StateContinuation::ExternallyBounded,
    };
    let sealed = seal_state_plan(&form, &ordinary, vec![state.clone()]).unwrap();
    assert!(conduit_core::verify_plan(&sealed));
    assert_ne!(sealed.plan_id, ordinary.plan_id);
    assert_eq!(sealed.checked_form_id, ordinary.checked_form_id);
    assert_eq!(sealed.fragments[0].states, vec![state.clone()]);
    assert_eq!(
        sealed.fragments[0].sign_storage_budget.item_capacity,
        ordinary.fragments[0].sign_storage_budget.item_capacity + 2
    );
    assert!(ordinary.fragments[0].states.is_empty());
    assert_eq!(
        seal_state_plan(&form, &sealed, vec![state]),
        Err(StateGraphError::StateAlreadySealed)
    );
    assert!(matches!(
        conduit_plan_lowering::lowering::lower_plan_fragment(&sealed.fragments[0]),
        Err(conduit_plan_lowering::lowering::LoweringError::UnsupportedState(_))
    ));
}
