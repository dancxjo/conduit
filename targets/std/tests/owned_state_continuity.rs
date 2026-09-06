use conduit_core::*;
use conduit_kernel::{Operation, OperationAction, PortId, ValueRef};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment_for_profile, LoweredState, FIXED_KERNEL_STORAGE_PROFILE,
};
use conduit_planner::state_delay::continuity::{seal_state_continuity, StateContinuityApproval};
use conduit_semantic_catalog::state_value::*;
use conduit_std_host::state_value::{RetainedTypedState, TypedStateOperation};

fn planned() -> (Plan, Vec<u8>) {
    let ty = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let value = StructuredInfoValue::leaf(ty.clone(), b"false".to_vec()).unwrap();
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    startup.insert_structured_type("Cell", ty.clone()).unwrap();
    install_state_value_kind("Cell", &ty, &value, &mut startup, &mut profile).unwrap();
    let form = conduit_form::parse_with_startup(
        "form retained {\n cell: state/value(initial = true)\n}\n",
        &startup,
        &profile,
    )
    .unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "state-host".into(),
        boot_id: "state-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "state-test@1".into(),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![conduit_std_offers::state_value_std_offer("Cell", &ty).unwrap()],
    }];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    let ordinary = conduit_planner::plan(&form, &hosts, &placements, &[]).unwrap();
    let state = derive_state_boundary(&form, &GearId::from("retained/cell"), 60).unwrap();
    let mut fragments = ordinary.fragments;
    fragments[0].states.push(state);
    fragments[0].sign_storage_budget.item_capacity += 2;
    fragments[0].sign_storage_budget.byte_capacity += 64;
    // A standalone typed-operation fixture. Full graph execution is proved by
    // installed_std's conformance test; this test owns the consuming handoff.
    (
        seal_plan(form.identity(), fragments),
        value.canonical_bytes().unwrap(),
    )
}

fn lower(plan: &Plan) -> LoweredState {
    lower_plan_fragment_for_profile(
        &plan.fragments[0],
        FIXED_KERNEL_STORAGE_PROFILE
            .with_state_storage(1, 64)
            .unwrap()
            .with_owned_state_continuity(),
    )
    .unwrap()
    .states
    .remove(0)
}

fn play(plan: &Plan, sequence: u64) -> ActivePlayIdentity {
    bind_active_play(
        &plan.plan_id,
        &plan.fragments[0].host_id,
        &plan.fragments[0].boot_id,
        sequence,
    )
}

fn retained(plan: &Plan, next: &[u8]) -> RetainedTypedState {
    let mut operation =
        TypedStateOperation::prepare_for_play(&plan.fragments[0], &lower(plan), &play(plan, 1))
            .unwrap();
    assert!(matches!(
        operation.start(),
        OperationAction::EmitCanonical { .. }
    ));
    let refused = operation.try_retire().err().unwrap();
    let mut operation = refused.source;
    assert_eq!(operation.generation(), 0);
    assert!(matches!(
        operation.resume_value(
            PortId(0),
            ValueRef {
                slot: 0,
                generation: 0,
                byte_len: next.len() as u32,
            },
            next
        ),
        OperationAction::EmitCanonical { .. }
    ));
    operation.step_committed();
    assert_eq!(operation.generation(), 1);
    operation.cancel();
    operation
        .try_retire()
        .unwrap_or_else(|failure| panic!("{}", failure.reason))
}

fn destination(source: &Plan, owned: &RetainedTypedState) -> Plan {
    let mut fragments = source.fragments.clone();
    fragments[0].boot_id = "replacement-boot".into();
    fragments[0].placements[0].boot_id = fragments[0].boot_id.clone();
    fragments[0].states[0].maximum_value_bytes = 64;
    let candidate = seal_plan(owned.provenance().source_form.clone(), fragments);
    seal_state_continuity(
        source,
        &candidate,
        owned.provenance().clone(),
        &StateContinuityApproval {
            source_plan: source.plan_id.clone(),
            destination_plan: candidate.plan_id.clone(),
            state: owned.provenance().source_state.clone(),
            maximum_value_bytes: 64,
        },
    )
    .unwrap()
}

#[test]
fn owned_state_moves_to_new_boot_and_larger_capacity_without_resetting_generation() {
    let (source, next) = planned();
    let owned = retained(&source, &next);
    assert_eq!(owned.provenance().source_play, play(&source, 1));
    let destination = destination(&source, &owned);
    let mut continued = TypedStateOperation::prepare_continued(
        &destination.fragments[0],
        &lower(&destination),
        &play(&destination, 2),
        owned,
    )
    .unwrap_or_else(|failure| panic!("{}", failure.reason));
    assert_eq!(continued.current(), next);
    assert_eq!(continued.generation(), 1);
    match continued.start() {
        OperationAction::EmitCanonical { value, .. } => assert_eq!(value.as_slice(), next),
        other => panic!("replacement must publish retained current: {other:?}"),
    }
    continued.cancel();
    let second = continued
        .try_retire()
        .unwrap_or_else(|failure| panic!("{}", failure.reason));
    assert_eq!(second.provenance().source_play, play(&destination, 2));
    assert_eq!(second.provenance().generation, 1);
}

#[test]
fn forged_snapshot_refuses_and_returns_the_original_owned_cell() {
    let (source, next) = planned();
    let owned = retained(&source, &next);
    let destination = destination(&source, &owned);
    let mut altered = destination.fragments.clone();
    altered[0].states[0].retained.as_mut().unwrap().generation += 1;
    let forged = seal_plan(owned.provenance().source_form.clone(), altered);
    assert!(verify_plan(&forged)); // Structurally valid metadata is insufficient.
    let refused = TypedStateOperation::prepare_continued(
        &forged.fragments[0],
        &lower(&forged),
        &play(&forged, 2),
        owned,
    )
    .err()
    .unwrap();
    assert_eq!(refused.source.provenance().current_value, next);
    assert_eq!(refused.source.provenance().generation, 1);
    let continued = TypedStateOperation::prepare_continued(
        &destination.fragments[0],
        &lower(&destination),
        &play(&destination, 2),
        refused.source,
    )
    .unwrap_or_else(|failure| panic!("{}", failure.reason));
    assert_eq!(continued.generation(), 1);
}

#[test]
fn fresh_constructor_cannot_reset_a_retained_contract() {
    let (source, next) = planned();
    let owned = retained(&source, &next);
    let destination = destination(&source, &owned);
    assert!(TypedStateOperation::prepare_for_play(
        &destination.fragments[0],
        &lower(&destination),
        &play(&destination, 2)
    )
    .is_err());
    let mut wrong_play = play(&source, 1);
    wrong_play.boot_id = "stale".into();
    assert!(TypedStateOperation::prepare_for_play(
        &source.fragments[0],
        &lower(&source),
        &wrong_play
    )
    .is_err());
}
