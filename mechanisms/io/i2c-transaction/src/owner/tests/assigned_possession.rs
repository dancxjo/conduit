//! Original opaque grants must remain usable before controller custody moves.
use super::*;

pub(super) fn reject_unusable_grants(
    slot: &mut Option<ReadyI2cBase<Provider>>,
    scope: &BaseCapabilityScope,
    issue: &impl Fn() -> (
        BaseCapabilityTable,
        BaseCapabilityHandle,
        BaseOperationClaim,
    ),
    mut bind: impl FnMut(
        &mut Option<ReadyI2cBase<Provider>>,
        BaseCapabilityTable,
        BaseCapabilityHandle,
        BaseOperationClaim,
    ) -> Result<I2cHostCall<Provider>, I2cCallRefusal>,
) {
    let original = slot.as_ref().unwrap().native_identity().clone();
    let attachment = slot.as_ref().unwrap().attachment();
    let (table, _, claim) = issue();
    let (_, foreign_handle, _) = fixture::authorization_for_play_seed(
        scope.clone(),
        &claim.plan_id,
        &claim.active_play_id,
        [8; 32],
    );
    assert_eq!(
        bind(slot, table, foreign_handle, claim).err(),
        Some(I2cCallRefusal::Capability(
            BaseCapabilityRefusal::UnknownCapability
        ))
    );
    assert_eq!(slot.as_ref().unwrap().native_identity(), &original);
    assert_eq!(slot.as_ref().unwrap().attachment(), attachment);

    let (mut table, handle, claim) = issue();
    table.revoke(&handle).unwrap();
    assert_eq!(
        table.inspections().next().unwrap().lifecycle,
        CapabilityLifecycle::Revoked
    );
    assert!(bind(slot, table, handle, claim).is_err());
    assert_eq!(slot.as_ref().unwrap().native_identity(), &original);

    // One completed local fixture lease makes this exact grant Exhausted.
    // No hardware transaction or replenished budget is manufactured here.
    let (_, _, basis) = issue();
    let mut finite = scope.clone();
    finite.maximum_operations = 1;
    let (mut table, mut handle, claim) = fixture::authorization_for_play_seed(
        finite,
        &basis.plan_id,
        &basis.active_play_id,
        [7; 32],
    );
    let lease = table.authorize(&handle, &claim).unwrap();
    table.complete(&mut handle, lease, 0).unwrap();
    assert_eq!(
        table.inspections().next().unwrap().lifecycle,
        CapabilityLifecycle::Exhausted
    );
    assert!(bind(slot, table, handle, claim).is_err());
    assert_eq!(slot.as_ref().unwrap().native_identity(), &original);
    assert_eq!(slot.as_ref().unwrap().attachment(), attachment);
}
