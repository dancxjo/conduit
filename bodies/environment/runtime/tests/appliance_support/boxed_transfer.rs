//! Boxed transfer is preparation only. Both retained owners survive refusal.
use super::*;
use conduit_environment_runtime::appliance::assigned::{
    PreparedResidentSoftware, ValidatedAssignedAppliance,
};
fn admitted(
    projection: &assignment::NativeProjection,
    recipe: &Recipe,
) -> Box<ValidatedAssignedAppliance> {
    Box::new(
        ValidatedAssignedAppliance::admit(
            &projection.bytes,
            assignment::NativeProjection::maxima(),
            projection.required(),
            &recipe.resident(projection),
        )
        .unwrap(),
    )
}
#[test]
fn boxed_transfer_preserves_both_slots_then_allocates_only_the_destination() {
    let (projection, recipe) = transfer_fixture(1);
    let programs = recipe.programs();
    let software = PreparedResidentSoftware::prepare(
        &recipe.resident(&projection),
        &programs,
        &conduit_time::TickConfiguration {
            count: 1,
            period_ms: 0,
        },
        conduit_environment_runtime::appliance::prepare_value_storage().unwrap(),
    )
    .unwrap();
    let mut software = Some(Box::new(software));
    let (other, other_recipe) = transfer_fixture(2);
    let mut selected = Some(admitted(&other, &other_recipe));
    let original_software = (&**software.as_ref().unwrap()) as *const _;
    let original_assignment = (&**selected.as_ref().unwrap()) as *const _;
    let (result, memory) = allocation_probe::observe(|| {
        PreparedResidentSoftware::bind_boxed_retained(&mut software, &mut selected)
    });
    assert!(result.is_err());
    assert_eq!(
        (
            memory.allocations,
            memory.reallocations,
            memory.deallocations
        ),
        (0, 0, 0)
    );
    assert_eq!(
        (&**software.as_ref().unwrap()) as *const _,
        original_software
    );
    assert_eq!(
        (&**selected.as_ref().unwrap()) as *const _,
        original_assignment
    );
    // Missing slots also refuse before allocation or destruction of the peer.
    let saved = software.take();
    let (result, memory) = allocation_probe::observe(|| {
        PreparedResidentSoftware::bind_boxed_retained(&mut software, &mut selected)
    });
    assert!(result.is_err() && selected.is_some());
    assert_eq!(
        (
            memory.allocations,
            memory.reallocations,
            memory.deallocations
        ),
        (0, 0, 0)
    );
    software = saved;
    selected = None;
    let (result, memory) = allocation_probe::observe(|| {
        PreparedResidentSoftware::bind_boxed_retained(&mut software, &mut selected)
    });
    assert!(result.is_err() && software.is_some());
    assert_eq!(
        (
            memory.allocations,
            memory.reallocations,
            memory.deallocations
        ),
        (0, 0, 0)
    );
    selected = Some(admitted(&projection, &recipe));
    let (result, memory) = allocation_probe::observe(|| {
        PreparedResidentSoftware::bind_boxed_retained(&mut software, &mut selected)
    });
    let prepared = result.unwrap();
    assert!(software.is_none() && selected.is_none());
    assert_eq!((memory.allocations, memory.reallocations), (1, 0));
    assert_eq!(memory.requested_bytes, core::mem::size_of_val(&*prepared));
    assert_eq!(prepared.count, 1);
    assert_eq!(
        prepared.tables.identity.plan.0.as_slice(),
        &projection.bytes[16..32]
    );
}
