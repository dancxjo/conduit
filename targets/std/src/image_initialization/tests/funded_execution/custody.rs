use super::*;

#[test]
fn escaped_fore_clone_retains_original_charge_after_report_and_pressures_next_play() {
    let (host, wake, plan, clock) = fixture(4, 64 * 1024 * 1024);
    let metadata = host
        .kernel_resources
        .image_memory_observer()
        .load(Ordering::Acquire);
    let limit = footprint(&host, &wake, &plan, &clock) + metadata;
    let (mut current, wake, plan, mut clock) = fixture(4, limit as u64);
    let control = crate::RunControl::default();
    let mut output = Output::new(4);
    let mut text = Vec::new();
    let counter = current.kernel_resources.image_memory_observer();
    let report = current
        .run_selected_image_body_plan_with_output_to_with_start(
            request(&wake, &plan, &control),
            &mut text,
            &mut clock,
            Some(&mut output),
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(output.retained.len(), 4); // Clones escaped from the actual adapter before return.
    let retained = counter.load(Ordering::Acquire);
    assert!(retained > metadata);
    drop(report);
    assert_eq!(counter.load(Ordering::Acquire), retained);
    let sequence = current.next_kernel_play_sequence;
    let mut started = false;
    let refused = current.run_selected_image_body_plan_with_output_to_with_start(
        request(&wake, &plan, &control),
        &mut text,
        &mut clock,
        Some(&mut output),
        |_, _| {
            started = true;
            Ok(())
        },
    );
    assert!(refused.err().unwrap().contains("managed backing capacity"));
    assert!(!started);
    assert_eq!(current.next_kernel_play_sequence, sequence);
    output.retained.clear();
    assert_eq!(counter.load(Ordering::Acquire), metadata);
    assert!(current.kernel_resources.is_idle());
}
