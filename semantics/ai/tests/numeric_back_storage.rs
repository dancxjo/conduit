#![cfg(feature = "fixed-numeric-owners")]
extern crate alloc;
// Test the same private boxing boundary, retaining original planned Back guards.
#[path = "../src/numeric_allocation_probe.rs"]
mod allocation_probe;
#[path = "../src/operation_owners/prepared_numeric_back.rs"]
mod prepared_numeric_back;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
#[ignore = "requires exact archived FARGAN artifact root"]
fn actual_original_back_box_inventory_and_legacy_projection() {
    use conduit_ai::fixed_numeric_float_integer::{
        FLOAT_INTEGER_IMPLEMENTATION, FixedFloatIntegerBack,
    };
    use prepared_numeric_back::PreparedNumericBack;
    let project = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_FARGAN_PREPARATION_ARTIFACT_ROOT").unwrap(),
    );
    let plan: conduit_core::Plan = serde_json::from_slice(
        &std::fs::read(
            project.join("outputs/committed-common-fargan-greeting/sealed-epoch-plan.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let gear = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|g| g.implementation_id.as_str() == FLOAT_INTEGER_IMPLEMENTATION)
        .unwrap();
    let (back, observed) = allocation_probe::observe(|| {
        let back = FixedFloatIntegerBack::prepare_planned::<16>(gear, 4, true).unwrap();
        let local = back.local_accounted_heap_bytes();
        PreparedNumericBack::new(back, local)
    });
    let (storage, getter) = allocation_probe::observe(|| back.storage());
    assert_eq!(
        storage.concrete_root_bytes(),
        std::mem::size_of::<FixedFloatIntegerBack>()
    );
    assert_eq!(storage.local_accounted_heap_bytes(), 6392);
    assert_eq!(
        storage.accounted_retained_bytes(),
        Some(observed.live_bytes)
    );
    assert_eq!((getter.allocations, getter.reallocations), (0, 0));
    let (mut legacy, projection) = allocation_probe::observe(|| back.into_back());
    assert_eq!((projection.allocations, projection.reallocations), (0, 0));
    assert!(legacy.prepared_output(conduit_kernel::PortId(0)).is_none());
    legacy.cancel();
    assert!(legacy.prepared_output(conduit_kernel::PortId(0)).is_none());
    println!(
        "actual original Back retained={} root={} local={}",
        observed.live_bytes,
        storage.concrete_root_bytes(),
        storage.local_accounted_heap_bytes()
    );
}
