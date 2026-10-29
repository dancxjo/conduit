use super::*;

#[test]
fn original_terminal_value_backing_is_emptied_without_resetting_generations_or_signs() {
    let mut f = fixture();
    let execution = f.execution.as_mut().unwrap();
    assert_eq!(run(execution).exit, native_worker::Exit::Drained);
    let retired = DeviceExecution::retire_with_material_retained(&mut f.execution).unwrap();
    let mut parts = retired.material.into_parts();
    let original = parts.execution.values.store(&[91]).unwrap();
    let expected = parts.execution.signs.events().collect::<Vec<_>>();
    let capacity = parts.execution.values.item_capacity();
    let ((mut values, signs), memory) = allocation_probe::observe(|| parts.into_reusable_backing());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(values.item_capacity(), capacity);
    assert_eq!((values.used_items(), values.used_bytes()), (0, 0));
    assert_eq!(signs.events().collect::<Vec<_>>(), expected);
    assert!(values.get(original).is_err());
    let mut reused_original_slot = false;
    for _ in 0..capacity {
        let fresh = values.store(&[92]).unwrap();
        if fresh.slot == original.slot {
            assert!(fresh.generation > original.generation);
            reused_original_slot = true;
            break;
        }
    }
    assert!(reused_original_slot);
    assert!(values.get(original).is_err());
}

#[test]
fn direct_retired_material_returns_original_backing_and_complete_signs_without_driver_return() {
    let mut f = fixture();
    assert_eq!(
        run(f.execution.as_mut().unwrap()).exit,
        native_worker::Exit::Drained
    );
    let retired = DeviceExecution::retire_with_material_retained(&mut f.execution).unwrap();
    let expected = retired
        .material
        .kernel()
        .signs()
        .events()
        .collect::<Vec<_>>();
    let capacity = retired.material.kernel().values().item_capacity();
    let ((mut values, signs), memory) =
        allocation_probe::observe(|| retired.material.into_reusable_backing());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(values.item_capacity(), capacity);
    assert_eq!((values.used_items(), values.used_bytes()), (0, 0));
    assert_eq!(signs.events().collect::<Vec<_>>(), expected);
    let fresh = values.store(&[93]).unwrap();
    assert!(fresh.generation > 0);
}
