#![cfg(feature = "display-output")]
use conduit_environment_runtime::display_output::DisplayValueBudget as Budget;
use conduit_kernel::ValueStorage;
#[test]
fn finite_counts_size_the_actual_store_and_larger_work_keeps_the_existing_profile() {
    for (frames, slots) in [(1, 5), (2, 9), (3, 12), (960, 12), (u32::MAX, 12)] {
        let budget = Budget::derive(frames, 651).unwrap();
        let store = budget.prepare().unwrap();
        assert_eq!(budget.items(), slots);
        assert_eq!(budget.bytes(), u32::from(slots) * 651);
        assert_eq!(store.item_capacity(), slots);
        assert_eq!(
            store.reserved_storage_bytes(),
            budget.storage_bytes().unwrap()
        );
    }
    for (frames, bytes) in [(0, 651), (1, 0), (1, 1025)] {
        assert!(Budget::derive(frames, bytes).is_err());
    }
}
