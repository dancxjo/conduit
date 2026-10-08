use super::*;
#[test]
fn immutable_fragment_batch_preserves_exact_binding_and_owner_refusals() {
    let (fragment, lowered, active, placement) = selected();
    let batch = PreparedExpressionFragment::prepare(&fragment, &lowered, &active).unwrap();
    let mut owner = batch.owner(&placement).unwrap();
    assert_eq!(
        owner
            .invoke(owner.node, HostCallId(0), RequestId(0), &[41])
            .unwrap(),
        &[42]
    );
    assert!(matches!(
        batch.owner(&PlacementId::from("foreign")),
        Err(ExpressionCallRefusal::WrongBinding)
    ));
    let mut foreign = active.clone();
    foreign.boot_id = "foreign".into();
    assert!(matches!(
        PreparedExpressionFragment::prepare(&fragment, &lowered, &foreign),
        Err(ExpressionCallRefusal::WrongBinding)
    ));
    let mut drift = lowered.clone();
    drift.node_specs[0].maximum_step_fuel += 1;
    assert!(matches!(
        PreparedExpressionFragment::prepare(&fragment, &drift, &active),
        Err(ExpressionCallRefusal::WrongBinding)
    ));
    let mut unsealed = fragment.clone();
    unsealed.placements[0].configuration.clear();
    assert!(matches!(
        PreparedExpressionFragment::prepare(&unsealed, &lowered, &active),
        Err(ExpressionCallRefusal::WrongBinding)
    ));
}
