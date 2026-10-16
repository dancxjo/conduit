//! Cancellation consumes parent authority and retains its terminal Sign.
use conduit_core::*;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common_acoustic_timed/boundary.rs"]
mod boundary;
#[path = "common_acoustic_timed/parent_plan.rs"]
mod parent_plan;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn cancellation_during_active_and_held_output_refuses_late_control_without_growth() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            for hold_output in [false, true] {
                let prepared = parent_plan::prepare();
                let mut kernel =
                    boundary::prepare(prepared.plan, &prepared.front, &prepared.registry);
                let input = &prepared.front.inputs()[0];
                let output = &prepared.front.outputs()[0];
                let item = ValuePayload {
                    value_kind: input.value_kind.clone(),
                    encoded: prepared.items[0].clone(),
                };
                let mut captured = ValuePayload {
                    value_kind: output.value_kind.clone(),
                    encoded: Vec::with_capacity(32768),
                };
                let (_, allocation) = allocation_probe::observe(|| {
                    kernel.start().unwrap();
                    kernel.admit_input(&input.port_id, 0, &item).unwrap();
                    if hold_output {
                        let mut ready = false;
                        for _ in 0..5000 {
                            kernel.step().unwrap();
                            if kernel
                                .output_into(&output.port_id, &mut captured)
                                .unwrap()
                                .is_some()
                            {
                                ready = true;
                                break;
                            }
                        }
                        assert!(ready);
                    } else {
                        kernel.step().unwrap();
                    }
                    kernel.cancel().unwrap();
                    assert_eq!(
                        kernel.step().unwrap(),
                        conduit_composite::KernelCompositeStatus::Cancelled
                    );
                    assert!(kernel.cancellation_failures().is_empty());
                    assert!(matches!(
                        kernel.admit_input(&input.port_id, 1, &item),
                        Err(conduit_composite::KernelCompositeError::InvalidLifecycle)
                    ));
                    assert!(matches!(
                        kernel.complete_output(&output.port_id, 0),
                        Err(conduit_composite::KernelCompositeError::InvalidLifecycle)
                    ));
                    assert!(kernel.output_into(&output.port_id, &mut captured).is_err());
                });
                assert_eq!(
                    (allocation.allocations, allocation.reallocations),
                    (0, 0),
                    "{allocation:?}"
                );
                let signs = kernel.signs();
                assert!(!signs.is_empty());
                for events in signs.values() {
                    assert_eq!(
                        events
                            .iter()
                            .filter(
                                |event| event.kind == conduit_kernel::KernelEventKind::RunCancelled
                            )
                            .count(),
                        1
                    );
                }
                println!(
                    "timed-cancel held={hold_output} plays={:?} storage={allocation:?}",
                    kernel.active_plays()
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
