use super::*;
use crate::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
};

fn graph() -> PreparedTimerGraph {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    let prepared = crate::tour_timer_plan::prepare(&identities, &offer, "build").unwrap();
    let fragment = &prepared.plan.fragments[0];
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    TourTimerKernel::prepare_graph(fragment, &lowered).unwrap()
}

#[test]
fn tour_timer_wire_fits_one_window_and_refuses_every_truncated_prefix() {
    let graph = graph();
    let mut encoded = [0; 256];
    let length = graph.encode(&mut encoded).unwrap();
    assert_eq!(
        PreparedTimerGraph::decode(&encoded[..length]).unwrap(),
        graph
    );
    for prefix in 0..length {
        assert!(
            PreparedTimerGraph::decode(&encoded[..prefix]).is_err(),
            "prefix {prefix}"
        );
    }
    assert!(PreparedTimerGraph::decode(&encoded[..length + 1]).is_err());
    let mut short = [0x55; 255];
    assert!(graph.encode(&mut short[..length - 1]).is_err());
    assert_eq!(short[length - 1], 0x55);
    let mut changed = encoded;
    changed[2] = 2;
    assert!(PreparedTimerGraph::decode(&changed[..length]).is_err());
    // The ordinary graph has two bindings followed by one absent binding.
    changed = encoded;
    changed[length - 1] = 2;
    assert!(PreparedTimerGraph::decode(&changed[..length]).is_err());
}

#[test]
fn tour_timer_wire_reconstructs_causal_count_progress_without_allocating() {
    let graph = graph();
    let allocations = crate::test_allocations::allocations(|| {
        let mut encoded = [0; 256];
        let length = graph.encode(&mut encoded).unwrap();
        let graph = PreparedTimerGraph::decode(&encoded[..length]).unwrap();
        let mut runtime = TourTimerKernel::from_prepared_graph(graph).unwrap();
        let mut counts = [None; 2];
        let mut count_len = 0;
        let mut waiting = None;
        let mut timer_requests = 0;
        for _ in 0..192 {
            while let Some(request) = runtime.next_host_request() {
                if runtime.is_timer(&request) {
                    assert!(waiting.replace(request).is_none());
                    timer_requests += 1;
                } else {
                    assert!(runtime.is_presentation(&request));
                    let bytes: [u8; 8] = runtime
                        .host_value(request.input.value)
                        .unwrap()
                        .try_into()
                        .unwrap();
                    counts[count_len] = Some(u64::from_le_bytes(bytes));
                    count_len += 1;
                    runtime.complete_presentation(request).unwrap();
                }
            }
            if count_len == 2 && waiting.is_some() && runtime.pending_host_calls() == 1 {
                assert_eq!(counts, [Some(0), Some(1)]);
                assert_eq!(timer_requests, 2);
                assert_eq!(runtime.pending_host_calls(), 1);
                runtime.cancel().unwrap();
                assert_eq!(runtime.step().unwrap(), SchedulerStatus::Cancelled);
                return;
            }
            if runtime.step().unwrap() == SchedulerStatus::Idle {
                let request = waiting
                    .take()
                    .expect("only an exact pending timer can wake progress");
                runtime
                    .complete_request(request.node, request.request)
                    .unwrap();
            }
        }
        panic!("standing timer did not reach bounded Stop");
    });
    assert_eq!(allocations, 0);
}

#[test]
fn tour_timer_numeric_graph_refuses_duplicate_roles_and_invalid_route_capacity() {
    let graph = graph();
    let mut changed = graph;
    changed.count = changed.timer;
    assert!(TourTimerKernel::from_prepared_graph(changed).is_err());
    changed = graph;
    changed.nodes[0].maximum_step_fuel = 0;
    assert!(TourTimerKernel::from_prepared_graph(changed).is_err());
    changed = graph;
    changed.cords[0].item_capacity = u16::MAX;
    assert!(TourTimerKernel::from_prepared_graph(changed).is_err());
}
