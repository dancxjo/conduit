//! Producer block partitions retain identical waveform/state and drain after close.
#[path = "common_acoustic_timed/assertions.rs"]
mod assertions;
#[path = "common_acoustic_timed/parent_plan.rs"]
mod parent_plan;
#[path = "common_acoustic_timed/partition_kernel.rs"]
mod partition_kernel;
#[test]
fn one_block_and_partitioned_controls_have_identical_state_with_pressure_and_tail_drain() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut reference = None;
            let mut source = None;
            for (partitions, gap, hold) in [(&[7][..], 0, 1), (&[1, 2, 1, 3][..], 7, 4)] {
                let prepared = parent_plan::prepare();
                if let Some(previous) = &source {
                    assert_eq!(&prepared.plan.source_document_id, previous);
                }
                source = Some(prepared.plan.source_document_id.clone());
                let states = partition_kernel::run(
                    prepared.plan,
                    &prepared.front,
                    &prepared.registry,
                    &prepared.items,
                    partitions,
                    gap,
                    hold,
                );
                assertions::check_states(&states);
                if let Some(previous) = &reference {
                    assert_eq!(&states, previous);
                }
                reference = Some(states);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
