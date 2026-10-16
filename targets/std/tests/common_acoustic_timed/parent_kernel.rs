//! Delayed producer and held-output checks over the prepared production parent.
use conduit_composite::KernelOperationRegistry;
use conduit_core::*;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "boundary.rs"]
mod boundary;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
pub fn run(
    plan: Plan,
    front: &CheckedFront,
    registry: KernelOperationRegistry,
    items: &[Vec<u8>],
    admission_gap: usize,
    output_hold: usize,
) -> Vec<Vec<u8>> {
    let mut kernel = boundary::prepare(plan, front, &registry);
    let inputs = items
        .iter()
        .map(|encoded| ValuePayload {
            value_kind: front.inputs()[0].value_kind.clone(),
            encoded: encoded.clone(),
        })
        .collect::<Vec<_>>();
    let mut results = items
        .iter()
        .map(|_| Vec::with_capacity(32768))
        .collect::<Vec<_>>();
    let mut output = ValuePayload {
        value_kind: front.outputs()[0].value_kind.clone(),
        encoded: Vec::with_capacity(32768),
    };
    let input_port = &front.inputs()[0].port_id;
    let output_port = &front.outputs()[0].port_id;
    let (_, storage) = allocation_probe::observe(|| {
        kernel.start().unwrap();
        for (index, item) in inputs.iter().enumerate() {
            for _ in 0..admission_gap {
                kernel.step().unwrap();
            }
            kernel.admit_input(input_port, index as u64, item).unwrap();
            let mut delivered = false;
            let mut holds = 0;
            let mut digest = None;
            for _ in 0..5000 {
                kernel.step().unwrap();
                assert!(kernel.next_host_request().is_none());
                if kernel
                    .output_into(output_port, &mut output)
                    .unwrap()
                    .is_some()
                {
                    let current = semantic_digest("fixture/timed-held-output", &output.encoded);
                    if let Some(expected) = digest {
                        assert_eq!(current, expected);
                    }
                    digest = Some(current);
                    holds += 1;
                    if holds == output_hold {
                        results[index].extend_from_slice(&output.encoded);
                        kernel.complete_output(output_port, index as u64).unwrap();
                        delivered = true;
                        break;
                    }
                }
            }
            assert!(delivered);
        }
        kernel.close_input(input_port).unwrap();
        let mut complete = false;
        for _ in 0..64 {
            complete |=
                kernel.step().unwrap() == conduit_composite::KernelCompositeStatus::Complete;
        }
        assert!(complete);
        assert_eq!(
            kernel
                .output_terminal_into(output_port, &mut output)
                .unwrap(),
            Some(conduit_composite::KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(
        (storage.allocations, storage.reallocations),
        (0, 0),
        "{storage:?}"
    );
    println!(
        "timed-parent-plays={:?}; storage={storage:?}",
        kernel.active_plays()
    );
    results
}
