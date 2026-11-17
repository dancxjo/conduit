use super::*;

#[test]
fn large_valid_tick_plan_tiny_heap_refuses_before_owned_preparation() {
    let (mut host, wake, plan, mut clock) = fixture(128, 1024);
    let control = crate::RunControl::default();
    let mut output = Output::new(128);
    let mut text = Vec::new();
    let sequence = host.next_kernel_play_sequence;
    let counter = host.kernel_resources.image_memory_observer();
    let baseline = counter.load(Ordering::Acquire);
    let mut started = false;
    let allocations = crate::allocation_probe::begin();
    let result = host.run_selected_image_body_plan_with_output_to_with_start(
        request(&wake, &plan, &control),
        &mut text,
        &mut clock,
        Some(&mut output),
        |_, _| {
            started = true;
            Ok(())
        },
    );
    let bytes = allocations.finish_bytes();
    assert!(result.err().unwrap().contains("managed backing capacity"));
    assert!(
        bytes < 16 * 1024,
        "refusal allocated {bytes} bytes before quota admission"
    );
    assert!(!started);
    assert_eq!(clock.sampled, 0);
    assert!(text.is_empty());
    assert!(output.retained.is_empty());
    assert_eq!(host.next_kernel_play_sequence, sequence);
    assert_eq!(counter.load(Ordering::Acquire), baseline);
    assert!(host.kernel_resources.is_idle());
}

#[test]
fn exact_requested_footprint_admits_and_one_byte_short_refuses() {
    let (host, wake, plan, clock) = fixture(4, 64 * 1024 * 1024);
    let metadata = host
        .kernel_resources
        .image_memory_observer()
        .load(Ordering::Acquire);
    let required = footprint(&host, &wake, &plan, &clock) + metadata;
    for (heap, accepted) in [(required, true), (required - 1, false)] {
        let (mut current, wake, plan, mut clock) = fixture(4, heap as u64);
        let control = crate::RunControl::default();
        let mut output = Output::new(4);
        let mut text = Vec::new();
        let mut started = false;
        let result = current.run_selected_image_body_plan_with_output_to_with_start(
            request(&wake, &plan, &control),
            &mut text,
            &mut clock,
            Some(&mut output),
            |_, _| {
                started = true;
                Ok(())
            },
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "{heap}: {:?}",
            result.as_ref().err()
        );
        assert_eq!(started, accepted);
        assert!(current.kernel_resources.is_idle());
    }
}

#[test]
fn a_foreign_original_counter_cannot_admit_a_body_receipt() {
    let (left, _, _, _) = fixture(4, 64 * 1024 * 1024);
    let (right, _, _, _) = fixture(4, 64 * 1024 * 1024);
    let lease = left.kernel_resources.fund_tick_preparation(4096).unwrap();
    assert!(left
        .kernel_resources
        .require_input_memory_owner(&lease)
        .is_ok());
    assert!(right
        .kernel_resources
        .require_input_memory_owner(&lease)
        .unwrap_err()
        .contains("different original counter"));
}
