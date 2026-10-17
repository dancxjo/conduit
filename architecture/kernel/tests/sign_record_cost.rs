//! Manual native observations of the production mandatory Sign sink.
//! Timings include the loop and black-box barriers; no timer subtraction,
//! scheduler CPU attribution, compiler-copy count or pass threshold is implied.
use conduit_kernel::{
    FixedSignLog, KernelEvent, KernelEventKind, NodeId, PortId, SignQuery, SignSink,
};
use std::{hint::black_box, mem::size_of, time::Instant};

fn record<const N: usize>(log: &mut FixedSignLog<N>) -> KernelEvent {
    log.record(
        black_box(NodeId(0)),
        black_box(Some(PortId(0))),
        None,
        black_box(KernelEventKind::ValueConsumed),
    )
    .unwrap()
}

fn sample<const N: usize>(trial: usize, print: bool) {
    let budget = u32::try_from(N * size_of::<KernelEvent>()).unwrap();
    let prepare = Instant::now();
    let mut log = FixedSignLog::<N>::new(budget).unwrap();
    black_box(&mut log);
    let prepare_ns = prepare.elapsed().as_nanos();
    let started = Instant::now();
    for _ in 0..N {
        black_box(record(&mut log));
    }
    let append_ns = started.elapsed().as_nanos();
    assert_eq!(usize::from(log.len()), N);
    assert_eq!(log.used_bytes(), budget);
    assert_eq!(log.retention_gap(), None);
    assert!(log
        .events()
        .enumerate()
        .all(|(i, e)| { e.sequence == i as u32 && e.kind == KernelEventKind::ValueConsumed }));

    // This is the real full-log path: transient eviction, entry rotation,
    // gap extension and append. It is not the normal spare-capacity path.
    let started = Instant::now();
    for _ in 0..N {
        black_box(record(&mut log));
    }
    let retention_ns = started.elapsed().as_nanos();
    assert_eq!(usize::from(log.len()), N);
    assert_eq!(log.used_bytes(), budget);
    let gap = log
        .retention_gap()
        .expect("full log retains an explicit gap");
    assert_eq!(gap.first_sequence, 0);
    assert_eq!(gap.last_sequence, (N - 1) as u32);
    assert_eq!(gap.entries, N as u32);
    assert!(log.events().enumerate().all(|(i, e)| {
        e.sequence == (N + i) as u32 && e.kind == KernelEventKind::ValueConsumed
    }));
    if print {
        println!(
            "SIGN_RECORD_COST_ROW {{\"capacity\":{N},\"records_per_phase\":{N},\"trial\":{trial},\"prepare_ns\":{prepare_ns},\"append_ns\":{append_ns},\"full_retention_ns\":{retention_ns},\"retained_event_bytes\":{budget},\"event_size\":{},\"sink_object_bytes\":{}}}",
            size_of::<KernelEvent>(), size_of::<FixedSignLog<N>>()
        );
    }
}

#[test]
fn probe_preserves_exact_events_and_full_log_retention() {
    sample::<1>(0, false);
    sample::<8>(0, false);
    sample::<32>(0, false);
}

#[test]
#[ignore = "manual optimized native Sign cost observations; no timing threshold"]
fn observe_production_sign_record_cost() {
    for trial in 0..32 {
        sample::<1>(trial, true);
        sample::<8>(trial, true);
        sample::<32>(trial, true);
        sample::<128>(trial, true);
        sample::<1024>(trial, true);
    }
}
