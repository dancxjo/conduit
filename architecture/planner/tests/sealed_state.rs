use conduit_core::{
    GearId, PlannedStateBoundary, StateContinuation, StateId, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};
use conduit_planner::state_delay::{plan::seal_state_plan, StateGraphError};
use conduit_planner::{default_placements, plan};
mod common;

#[test]
fn checked_state_is_sealed_into_a_fresh_plan_with_exact_evidence_capacity() {
    let mut profile = conduit_semantic_catalog::standard_profile_catalog();
    let mut startup = profile.startup_catalog().unwrap();
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profile).unwrap();
    let form = conduit_form::parse_with_startup(
        "form retained {\n source: scalar/literal(value = 7)\n cell: state/latest\n source.value > cell.in\n}\n",
        &startup,
        &profile,
    ).unwrap();
    let mut host = common::standard_planning_fixture("host", "boot");
    // Scalar literals are a portable contract, but not an installed nucleus
    // source. Supply an honestly named planning-only fixture realization.
    host.capabilities
        .push(conduit_semantic_catalog::realization_offer(
            conduit_semantic_catalog::scalar_literal_contract(),
            conduit_semantic_catalog::VALUE_PRIMITIVE_CONTRACT_REVISION,
            conduit_semantic_catalog::RealizationOfferIdentity {
                capability: "fixture/scalar-literal",
                execution_profile: "fixture/scalar-literal@1",
                implementation: "fixture/scalar-literal@1",
                artifact: "fixture/planning-only@1",
            },
            vec![],
            vec![],
            vec![],
        ));
    let hosts = [host];
    let placements = default_placements(&form, &hosts).unwrap();
    let ordinary = plan(&form, &hosts, &placements, &[]).unwrap();
    let state = PlannedStateBoundary {
        state_id: StateId::from("retained-state"),
        gear_id: GearId::from("retained/cell"),
        value_kind: conduit_core::KindId::from(SCALAR_INFO_ID),
        initial_value: vec![0; SCALAR_ENCODED_LEN],
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
